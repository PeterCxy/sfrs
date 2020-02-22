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
struct SyncConflict {
    #[serde(rename(serialize = "type"))]
    conf_type: String,
    server_item: Option<item::SyncItem>,
    unsaved_item: Option<item::SyncItem>
}

impl SyncConflict {
    fn uuid<'a>(&self) -> String {
        if let Some(ref item) = self.server_item {
            item.uuid.clone()
        } else if let Some(ref item) = self.unsaved_item {
            item.uuid.clone()
        } else {
            panic!("SyncConflict should have either server_item or unsaved_item");
        }
    }
}

#[derive(Serialize)]
struct SyncResp {
    retrieved_items: Vec<item::SyncItem>,
    saved_items: Vec<item::SyncItem>,
    conflicts: Vec<SyncConflict>,
    sync_token: Option<String>, // for convenience, we will actually always return this
    cursor_token: Option<String>
}

#[post("/items/sync", format = "json", data = "<params>")]
fn items_sync(db: DbConn, u: user::User, params: Json<SyncParams>) -> Custom<JsonResp<SyncResp>> {
    let mut resp = SyncResp {
        retrieved_items: vec![],
        saved_items: vec![],
        conflicts: vec![],
        sync_token: None,
        cursor_token: None
    };

    let inner_params = params.into_inner();

    let mut from_id: Option<i64> = None;
    let mut max_id: Option<i64> = None;
    let mut had_cursor = false;
    // mark if we have a larger sync_token than cursor_token
    let mut sync_token_ahead = false;

    if let Some(cursor_token) = inner_params.cursor_token {
        // If the client provides cursor_token,
        // then, we return all records
        // until sync_token (the head of the last sync)
        from_id = cursor_token.parse().ok();
        had_cursor = true;
    }
    
    if let Some(sync_token) = inner_params.sync_token.clone() {
        if !had_cursor {
            // If there is no cursor_token, then we are doing
            // a normal sync, so just return all records from sync_token
            from_id = sync_token.parse().ok();
        } else {
            // When we have both a cursor_token and a sync_token,
            // we need to always make sure we don't go *beyond* sync_token
            max_id = sync_token.parse().ok()
                    .and_then(|x| {
                        if x < from_id.unwrap() {
                            // If sync_token is smaller than cursor_token
                            // we don't set a max_id
                            // we will synchronize the two later after
                            // items are retrieved.
                            // We don't need to worry about the case
                            // where sync_token = cursor_token, because
                            // in that case we will get empty result,
                            // and sync_token will get updated anyway
                            None
                        } else {
                            // Tell our program logic later to not update
                            // sync_token (because it's already ahead)
                            sync_token_ahead = true;
                            Some(x)
                        }
                    });
        }
    }

    // First, retrieve what the client needs
    let result = item::SyncItem::items_of_user(&db, &u,
        from_id, max_id, inner_params.limit);

    match result {
        Err(item::ItemOpError(e)) => {
            return error_resp(Status::InternalServerError, vec![e])
        },
        Ok(items) => {
            if !items.is_empty() {
                // If we fetched something, and the length is right at limit
                // we may have more to fetch. In this case, we need to
                // inform the client to continue fetching, until there is
                // nothing more to fetch
                // (i.e. until cursor_token is equal to sync_token)
                let next_from = items.last().unwrap().id;
                if let Some(limit) = inner_params.limit {
                    if items.len() as i64 == limit {
                        // We may still have something to fetch
                        resp.cursor_token = Some(next_from.to_string());
                    }
                }
                
                if sync_token_ahead {
                    // Always keep sync_token unchanged in this case
                    // (this may change later when we save items)
                    resp.sync_token = inner_params.sync_token;
                } else {
                    // If sync_token is not ahead of cursor_token
                    // (or cursor_token is simply null)
                    // update it to latest
                    // Since it's sync_token, we don't need to worry
                    // about whether we *actually* have anything to fetch
                    resp.sync_token = Some(next_from.to_string());
                }
            } else {
                if had_cursor {
                    // If we already have no item to give, but the client still holds a cursor
                    // Revoke that cursor, and make it the sync_token
                    // (this may change later when we save items)
                    resp.sync_token = resp.cursor_token.clone();
                    resp.cursor_token = None;
                } else {
                    // Pass the same sync_token back
                    // (this may change later when we save items)
                    resp.sync_token = inner_params.sync_token;
                }
            }
            resp.retrieved_items = items.into_iter().map(|x| x.into()).collect();
        }
    }

    // Then, update all items sent by client
    let mut last_id: i64 = -1;
    for mut it in inner_params.items.into_iter() {
        // Handle conflicts 
        // Anything that we just retrieved but need to save immediately
        // is potentially a conflict
        // TODO: how do we handle this when the sync needs multiple requests
        //   to finish?
        let mut conflicted = false;
        for y in resp.retrieved_items.iter() {
            if it.uuid == y.uuid {
                conflicted = true;
                // We assume enc_item_key identifies an "item"
                if it.enc_item_key == y.enc_item_key {
                    // A sync conflict
                    resp.conflicts.push(SyncConflict {
                        conf_type: "sync_conflict".to_string(),
                        server_item: Some(y.clone()),
                        unsaved_item: None
                    });
                } else {
                    // A UUID conflict (unlikely)
                    resp.conflicts.push(SyncConflict {
                        conf_type: "uuid_conflict".to_string(),
                        server_item: None,
                        unsaved_item: Some(it.clone())
                    })
                }
            }
        }

        // do not save conflicted items
        if conflicted {
            continue;
        }

        // Always update updated_at for all items on server
        it.updated_at = 
            Some(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true));

        match item::SyncItem::items_insert(&db, &u, &it) {
            Err(item::ItemOpError(e)) => {
                return error_resp(Status::InternalServerError, vec![e]);
            },
            Ok(id) => {
                last_id = id;
                resp.saved_items.push(it);
            }
        }
    }

    if last_id > -1 {
        // Update sync_token to the latest one of our saved items
        // This is ALWAYS the case. `sync_token` indicates the
        // LATEST known state of the system by the client,
        // but it MAY still need to fill in a bit of history
        // (that's where `cursor_token` comes into play)
        resp.sync_token = Some(last_id.to_string());
    }

    // Remove conflicted items from retrieved items
    let mut new_retrieved = vec![];
    for x in resp.retrieved_items.into_iter() {
        let mut is_conflict = false;
        for y in resp.conflicts.iter() {
            if x.uuid == y.uuid() {
                is_conflict = true;
            }
        }

        if !is_conflict {
            new_retrieved.push(x);
        }
    }
    resp.retrieved_items = new_retrieved;

    Custom(Status::Ok, Json(Response::Success(resp)))
}