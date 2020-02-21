use crate::schema::users;
use crate::schema::users::dsl::*;
use crate::{lock_db_write, lock_db_read};
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use rocket::request;
use rocket::http::Status;
use serde::Deserialize;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

lazy_static! {
    static ref JWT_SECRET: String = env::var("SFRS_JWT_SECRET")
        .expect("Please have SFRS_JWT_SECRET set");
}

#[derive(Debug)]
pub struct UserOpError(pub String);

impl UserOpError {
    fn new(s: impl Into<String>) -> UserOpError {
        UserOpError(s.into())
    }
}

impl Into<UserOpError> for &str {
    fn into(self) -> UserOpError {
        UserOpError::new(self)
    }
}

// Password should ALWAYS be hashed
#[derive(Debug)]
pub struct Password(String);

impl Password {
    fn new(passwd: &str) -> Password {
        let params = scrypt::ScryptParams::new(11, 8, 1).unwrap();
        Password(scrypt::scrypt_simple(passwd, &params).unwrap())
    }
}

impl PartialEq<&str> for Password {
    fn eq(&self, other: &&str) -> bool {
        scrypt::scrypt_check(*other, &self.0).is_ok()
    }
}

impl Into<Password> for String {
    fn into(self) -> Password {
        Password::new(&self)
    }
}

// Convert itself to a hash String for db operations
impl Into<String> for Password {
    fn into(self) -> String {
        self.0
    }
}

// A raw User returned from database
// we need to wrap the password in the Password type
#[derive(Queryable)]
struct UserQuery {
    pub id: i32,
    pub email: String,
    pub password: String,
    pub pw_cost: i32,
    pub pw_nonce: String,
    pub version: String
}

impl Into<User> for UserQuery {
    fn into(self) -> User {
        User {
            id: self.id,
            email: self.email,
            // We can directly construct Password here
            // because it's already the hashed value from db
            password: Password(self.password),
            pw_cost: self.pw_cost,
            pw_nonce: self.pw_nonce,
            version: self.version
        }
    }
}

#[derive(Debug)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub password: Password,
    pub pw_cost: i32,
    pub pw_nonce: String,
    pub version: String
}

#[derive(Insertable, Deserialize)]
#[table_name="users"]
pub struct NewUser {
    pub email: String,
    pub password: String,
    pub pw_cost: i32,
    pub pw_nonce: String,
    pub version: String
}

impl User {
    pub fn create(db: &SqliteConnection, new_user: &NewUser) -> Result<(), UserOpError> {
        let user_hashed = NewUser {
            email: new_user.email.clone(),
            password: Password::new(&new_user.password).into(),
            pw_cost: new_user.pw_cost.clone(),
            pw_nonce: new_user.pw_nonce.clone(),
            version: new_user.version.clone()
        };

        match Self::find_user_by_email(db, &new_user.email) {
            Ok(_) => Err(UserOpError::new("User already registered")),
            Err(_) => lock_db_write!()
                        .and_then(|_| diesel::insert_into(users::table)
                            .values(user_hashed)
                            .execute(db)
                            .map(|_| ())
                            .map_err(|_| UserOpError::new("Database error")))
        }
    }

    pub fn find_user_by_email(db: &SqliteConnection, user_email: &str) -> Result<User, UserOpError> {
        let mut results = lock_db_read!()
            .and_then(|_| users.filter(email.eq(user_email))
                .limit(1)
                .load::<UserQuery>(db)
                .map_err(|_| UserOpError::new("Database error")))?;
        if results.is_empty() {
            Result::Err(UserOpError::new("No matching user found"))
        } else {
            Result::Ok(results.remove(0).into()) // Take ownership, kill the stupid Vec
        }
    }

    pub fn find_user_by_token(db: &SqliteConnection, token: &str) -> Result<User, UserOpError> {
        let parsed: jwt::Token<jwt::Claims, jwt::Registered> =
            jwt::Token::parse(token).map_err(|_| "Invalid JWT Token".into())?;
        if !parsed.verify(JWT_SECRET.as_bytes(), crypto::sha2::Sha256::new()) {
            Err("JWT Signature Verification Error".into())
        } else {
            parsed.claims.sub
                .ok_or("Malformed Token".into())
                .and_then(|mail| Self::find_user_by_email(db, &mail))
        }
    }

    // Create a JWT token for the current user if password matches
    pub fn create_token(&self, passwd: &str) -> Result<String, UserOpError> {
        if self.password != passwd {
            Err(UserOpError::new("Password mismatch"))
        } else {
            jwt::Token::new(
                jwt::Header::default(),
                jwt::Claims::new(jwt::Registered {
                    iss: None,
                    sub: Some(self.email.clone()),
                    exp: None,
                    aud: None,
                    iat: Some(SystemTime::now().duration_since(UNIX_EPOCH)
                            .expect("wtf????").as_secs()),
                    nbf: None,
                    jti: None
                })
            ).signed(JWT_SECRET.as_bytes(), crypto::sha2::Sha256::new())
             .map_err(|_| UserOpError::new("Failed to generate token"))
        }
    }

    // Change the password in database, if old password is provided
    // The current instance of User model will not be mutated
    pub fn change_pw(&self, db: &SqliteConnection, passwd: &str, new_passwd: &str) -> Result<(), UserOpError> {
        if self.password != passwd {
            Err(UserOpError::new("Password mismatch"))
        } else {
            // Update database
            // TODO: Maybe we should revoke all JWTs somehow?
            //      maybe we can record when the user last changed?
            lock_db_write!()
                .and_then(|_| diesel::update(users.find(self.id))
                    .set(password.eq::<String>(Password::new(new_passwd).into()))
                    .execute(db)
                    .map(|_| ())
                    .map_err(|_| UserOpError::new("Database error")))
        }
    }
}

// Implement request guard for User type
// This is intended for protecting authorized endpoints
impl<'a, 'r> request::FromRequest<'a, 'r> for User {
    type Error = UserOpError;

    fn from_request(request: &'a request::Request<'r>) -> request::Outcome<Self, Self::Error> {
        let token = request.headers().get_one("authorization");
        match token {
            None => request::Outcome::Failure((Status::Unauthorized, "Token missing".into())),
            Some(token) => {
                if !token.starts_with("Bearer ") {
                    return request::Outcome::Failure((Status::Unauthorized, "Malformed Token".into()));
                }

                let result = Self::find_user_by_token(
                    &request.guard::<crate::DbConn>().unwrap(), &token[7..]);
                match result {
                    Ok(u) => request::Outcome::Success(u),
                    Err(err) => request::Outcome::Failure((Status::Unauthorized, err))
                }
            }
        }
    }
}