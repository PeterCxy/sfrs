use crate::DbConn;
use crate::user;
use rocket::http::Status;
use rocket::response::status::Custom;
use rocket_contrib::json::Json;
use serde::Serialize;
use std::vec::Vec;

pub fn routes() -> impl Into<Vec<rocket::Route>> {
    routes![
        auth,
        auth_params
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
struct AuthResult {
    token: String
}

#[post("/auth", format = "json", data = "<new_user>")]
fn auth(db: DbConn, new_user: Json<user::NewUser>) -> Custom<JsonResp<AuthResult>> {
    match user::User::create(&db.0, &new_user) {
        Ok(_) => success_resp(AuthResult {
            token: "aaaa".to_string()
        }),
        Err(user::UserOpError(e)) =>
            error_resp(Status::InternalServerError, vec![e])
    }
}

#[derive(Serialize)]
struct AuthParams {
    pw_cost: String,
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