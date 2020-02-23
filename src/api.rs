use crate::DbConn;
use crate::user;
use crate::item;
use crate::lock::UserLock;
use itertools::{Itertools, Either};
use rocket::State;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket_contrib::json::Json;
use serde::{Serialize, Deserialize};
use std::vec::Vec;

lazy_static! {
    static ref EMAIL_RE: regex::Regex =
        regex::Regex::new(r"^([a-z0-9_+]([a-z0-9_+.]*[a-z0-9_+])?)@([a-z0-9]+([\-\.]{1}[a-z0-9]+)*\.[a-z]{2,6})")
                .unwrap();
}

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
    if !EMAIL_RE.is_match(&new_user.email) {
        return error_resp(Status::BadRequest, vec!["Invalid email address".into()]);
    }

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
    let res = user::User::find_user_by_email(&db.0, mail)
                .and_then(|u| u.create_token(&db.0, passwd)
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
    match user::User::find_user_by_email(&db.0, &email) {
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
    let res = user::User::find_user_by_email(&db.0, &params.email)
                .and_then(|u|
                    u.change_pw(&db.0, &params.current_password, &params.password));
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
fn items_sync(
    db: DbConn, lock: State<UserLock>,
    u: user::User, params: Json<SyncParams>
) -> Custom<JsonResp<SyncResp>> {
    // Only allow one sync per user at the same time
    // Operations below are far from atomic (neither are they in Ruby or Go impl)
    // so allowing multiple synchronize sessions each time can cause
    // some confusing behavior, e.g. another sync session might insert
    // something new into the database after this one gets the current_max_id
    // but before this one returns. It can also mess things up during
    // insertions into the database.
    // In short, do not let the same user synchronize from two clients
    // at the same time. All code below assumes that this lock works
    // and at any given point in time, up to one sync process is running
    // for each user.
    let mutex = lock.get_mutex(u.id);
    let _lock = mutex.lock().unwrap();

    // sync_token should always be set to the maximum ID currently available
    // (for this user, of course)
    // Remember that we have a mutex at the beginning of this function,
    // so all that can change the current_max_id for the current user
    // is operations later in this function.
    let new_sync_token = match item::SyncItem::get_current_max_id(&db.0, &u) {
        Ok(Some(id)) => Some(id.to_string()),
        Ok(None) => None,
        Err(item::ItemOpError(e)) =>
            return error_resp(Status::InternalServerError, vec![e])
    };

    let mut resp = SyncResp {
        retrieved_items: vec![],
        saved_items: vec![],
        conflicts: vec![],
        sync_token: new_sync_token,
        cursor_token: None
    };

    let inner_params = params.into_inner();

    let from_id: Option<i64> = if let Some(cursor_token) = inner_params.cursor_token {
        // If the client provides cursor_token,
        // then, we return all records
        // until sync_token (the head of the last sync)
        cursor_token.parse().ok()
    } else if let Some(sync_token) = inner_params.sync_token {
        // If there is no cursor_token, then we are doing
        // a normal sync, so just return all records from sync_token
        sync_token.parse().ok()
    } else {
        None
    };

    // First, retrieve what the client needs
    let result = item::SyncItem::items_of_user(&db.0, &u,
        from_id, None, inner_params.limit);

    match result {
        Err(item::ItemOpError(e)) => {
            return error_resp(Status::InternalServerError, vec![e])
        },
        Ok(items) => {
            if !items.is_empty() {
                // If we fetched something, and the length is right at limit
                // we may have more to fetch. In this case, we need to
                // inform the client to continue fetching
                let next_from = items.last().unwrap().id;
                if let Some(limit) = inner_params.limit {
                    if items.len() as i64 == limit {
                        // We may still have something to fetch
                        resp.cursor_token = Some(next_from.to_string());
                    }
                }
            }

            resp.retrieved_items = items.into_iter().map(|x| x.into()).collect();
        }
    }

    // Detect conflicts between client items and server items
    let (items_conflicted, items_to_save): (Vec<_>, Vec<_>) =
        inner_params.items.into_iter().partition_map(|client_item| {
            let conflict: Vec<_> = resp.retrieved_items.iter()
                .filter(|server_item| client_item.uuid == server_item.uuid)
                .collect();
            if !conflict.is_empty() {
                Either::Left((client_item, conflict[0].clone()))
            } else {
                Either::Right(client_item)
            }
        });

    // Convert conflicts into the format our client wants
    resp.conflicts = items_conflicted.into_iter().map(|(_client_item, server_item)| {
        // Our implementation never produces `uuid_conflict`
        // because the primary key of the `items` table is an internal ID
        // and we retrieve content based on (user, uuid) tuple, not just uuid.
        // The whole point of having `uuid_conflict` in their official impl
        // is because they use `uuid` as the primary key, so two items
        // on the same server cannot share the same uuid
        SyncConflict {
            conf_type: "sync_conflict".to_string(),
            server_item: Some(server_item),
            unsaved_item: None
        }
    }).collect();

    // Then, update all items sent by client
    let mut last_id: i64 = -1;
    for mut it in items_to_save.into_iter() {
        // Always update updated_at for all items on server
        it.updated_at = 
            Some(chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true));

        match item::SyncItem::items_insert(&db.0, &u, &it) {
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
        // Since we have added more items to the database,
        // the sync_token we had no longer points to the latest item
        // Update sync_token to the latest one of our saved items
        // This is ALWAYS the case. `sync_token` indicates the
        // LATEST known state of the system by the client,
        // but it MAY still need to fill in a bit of history
        // (that's where `cursor_token` comes into play)
        resp.sync_token = Some(last_id.to_string());
    }

    // Remove conflicted items from retrieved items
    let conflicts = &resp.conflicts;
    resp.retrieved_items = resp.retrieved_items.into_iter().filter(|x| {
        !conflicts.iter()
            .map(|y| x.uuid == y.uuid())
            .fold(false, |x, y| x || y)
    }).collect();

    success_resp(resp)
}