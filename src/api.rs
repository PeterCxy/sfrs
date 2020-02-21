use crate::DbConn;
use crate::user;
use crate::item;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket_contrib::json::Json;
use serde::{Serialize, Deserialize};
use std::vec::Vec;

pub fn routes() -> impl Into<Vec<rocket::Route>> {
    routes![
        auth,
        auth_change_pw,
        auth_sign_in,
        auth_params,
        auth_ping,
        items_sync
    ]
}

#[derive(Serialize)]
#[serde(untagged)]
enum Response<T: Serialize> {
    Error {
        errors: Vec<String>
    },
    Success(T)
}

// Some shorthands
type JsonResp<T> = Json<Response<T>>;

fn success_resp<T: Serialize>(resp: T) -> Custom<JsonResp<T>> {
    Custom(Status::Ok, Json(Response::Success(resp)))
}

fn error_resp<T: Serialize>(status: Status, errors: Vec<String>) -> Custom<JsonResp<T>> {
    Custom(status, Json(Response::Error {
        errors
    }))
}

#[derive(Serialize)]
struct AuthResultUser {
    email: String,
    uuid: String
}

#[derive(Serialize)]
struct AuthResult {
    user: AuthResultUser,
    token: String
}

#[post("/auth", format = "json", data = "<new_user>")]
fn auth(db: DbConn, new_user: Json<user::NewUser>) -> Custom<JsonResp<AuthResult>> {
    match user::User::create(&db.0, &new_user) {
        Ok(_) => _sign_in(db, &new_user.email, &new_user.password),
        Err(user::UserOpError(e)) =>
            error_resp(Status::InternalServerError, vec![e])
    }
}

#[derive(Deserialize)]
struct SignInParams {
    email: String,
    password: String
}

#[post("/auth/sign_in", format = "json", data = "<params>")]
fn auth_sign_in(db: DbConn, params: Json<SignInParams>) -> Custom<JsonResp<AuthResult>> {
    _sign_in(db, &params.email, &params.password)
}

// Shared logic for all interfaces that needs to do an automatic sign-in
fn _sign_in(db: DbConn, mail: &str, passwd: &str) -> Custom<JsonResp<AuthResult>> {
    // Try to find the user first
    let res = user::User::find_user_by_email(&db, mail)
                .and_then(|u| u.create_token(&db, passwd)
                                .map(|x| (u.uuid, u.email, x)));
    match res {
        Ok((uuid, email, token)) => success_resp(AuthResult {
            user: AuthResultUser {
                uuid,
                email
            },
            token
        }),
        Err(user::UserOpError(e)) =>
            error_resp(Status::InternalServerError, vec![e])
    }
}

#[derive(Serialize)]
struct AuthParams {
    pw_cost: i32,
    pw_nonce: String,
    version: String
}

impl Into<AuthParams> for user::User {
    fn into(self) -> AuthParams {
        AuthParams {
            pw_cost: self.pw_cost,
            pw_nonce: self.pw_nonce,
            version: self.version
        }
    }
}

#[get("/auth/params?<email>")]
fn auth_params(db: DbConn, email: String) -> Custom<JsonResp<AuthParams>> {
    match user::User::find_user_by_email(&db, &email) {
        Ok(u) => success_resp(u.into()),
        Err(user::UserOpError(e)) =>
            error_resp(Status::InternalServerError, vec![e])
    }
}

#[derive(Deserialize)]
struct ChangePwParams {
    email: String,
    password: String,
    current_password: String
}

#[post("/auth/change_pw", format = "json", data = "<params>")]
fn auth_change_pw(db: DbConn, params: Json<ChangePwParams>) -> Custom<JsonResp<()>> {
    let res = user::User::find_user_by_email(&db, &params.email)
                .and_then(|u|
                    u.change_pw(&db, &params.current_password, &params.password));
    match res {
        Ok(_) => Custom(Status::NoContent, Json(Response::Success(()))),
        Err(user::UserOpError(e)) =>
            error_resp(Status::InternalServerError, vec![e])
    }
}

// For testing the User request guard
#[get("/auth/ping")]
fn auth_ping(_db: DbConn, u: user::User) -> Custom<JsonResp<String>> {
    Custom(Status::Ok, Json(Response::Success(u.email)))
}

#[derive(Deserialize)]
struct SyncParams {
    items: Vec<item::SyncItem>,
    sync_token: Option<String>,
    cursor_token: Option<String>,
    limit: Option<i64>
}

#[derive(Serialize)]
struct SyncResp {
    retrieved_items: Vec<item::SyncItem>,
    saved_items: Vec<item::SyncItem>,
    unsaved: Vec<item::SyncItem>,
    sync_token: Option<String>, // for convenience, we will actually always return this
    cursor_token: Option<String>
}

#[post("/items/sync", format = "json", data = "<params>")]
fn items_sync(db: DbConn, u: user::User, params: Json<SyncParams>) -> Custom<JsonResp<SyncResp>> {
    let mut resp = SyncResp {
        retrieved_items: vec![],
        saved_items: vec![],
        unsaved: vec![],
        sync_token: None,
        cursor_token: None
    };

    let inner_params = params.into_inner();

    // First, update all items sent by client
    for mut it in inner_params.items.into_iter() {
        let old_updated_at = it.updated_at.clone();
        // Always update updated_at for all items on server
        it.updated_at = 
            Some(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true));

        if let Err(item::ItemOpError(_)) = item::SyncItem::items_insert(&db, &u, &it) {
            // Well, we should try twice...
            // TODO: make this more elegant (also handle differneces between db error and conflict)
            // (if we were ever to implement a conflict feature)
            if let Err(item::ItemOpError(_)) = item::SyncItem::items_insert(&db, &u, &it) {
                // Let's not fail just because one of them...
                // At least the client will know there's an error
                // (maybe mistakes it for conflict)
                it.updated_at = old_updated_at;
                resp.unsaved.push(it);
            }
        } else {
            resp.saved_items.push(it);
        }
    }

    let mut from_id: Option<i64> = None;
    let mut had_cursor = false;

    if let Some(cursor_token) = inner_params.cursor_token {
        // If the client provides cursor_token,
        // then, we return all records
        // until sync_token (the head of the last sync)
        from_id = cursor_token.parse().ok();
        had_cursor = true;
    } else if let Some(sync_token) = inner_params.sync_token.clone() {
        // If there is no cursor_token, then we are doing
        // a normal sync, so just return all records from sync_token
        from_id = sync_token.parse().ok();
    }

    // Then, retrieve what the client needs
    let result = item::SyncItem::items_of_user(&db, &u,
        from_id, None, inner_params.limit);

    match result {
        Err(item::ItemOpError(e)) => {
            error_resp(Status::InternalServerError, vec![e])
        },
        Ok(items) => {
            if !items.is_empty() {
                // If we fetched something
                // we should record the current last id to *some* token
                // this may be `cursor_token` or `sync_token`
                // if we have more to fetch, we set `cursor_token` to this id
                //    so that the client knows to continue
                // and we set `sync_token` to this id regardless, for the
                //    client to remember where we were even if
                //    it doesn't need to continue
                let next_from = items.last().unwrap().id;
                if let Some(limit) = inner_params.limit {
                    if items.len() as i64 == limit {
                        // We may still have something to fetch
                        resp.cursor_token = Some(next_from.to_string());
                    }
                }
                
                // Always set sync_token to equal cursor_token in this case
                resp.sync_token = Some(next_from.to_string());
            } else {
                if had_cursor {
                    // If we already have no item to give, but the client still holds a cursor
                    // Revoke that cursor, and make it the sync_token
                    resp.sync_token = resp.cursor_token.clone();
                    resp.cursor_token = None;
                } else {
                    // Pass the same sync_token back
                    resp.sync_token = inner_params.sync_token.clone();
                }
            }
            resp.retrieved_items = items.into_iter().map(|x| x.into()).collect();
            Custom(Status::Ok, Json(Response::Success(resp)))
        }
    }
}