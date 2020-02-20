use crate::schema::users;
use crate::schema::users::dsl::*;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use serde::Deserialize;
use std::env;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug)]
pub struct UserOpError(pub String);

impl UserOpError {
    fn new(s: impl Into<String>) -> UserOpError {
        UserOpError(s.into())
    }
}

#[derive(Queryable, Debug)]
pub struct User {
    pub id: i32,
    pub email: String,
    pub password: String,
    pub pw_cost: String,
    pub pw_nonce: String,
    pub version: String
}

#[derive(Insertable, Deserialize)]
#[table_name="users"]
pub struct NewUser {
    pub email: String,
    pub password: String,
    pub pw_cost: String,
    pub pw_nonce: String,
    pub version: String
}

impl User {
    pub fn create(db: &SqliteConnection, new_user: &NewUser) -> Result<(), UserOpError> {
        match Self::find_user_by_email(db, &new_user.email) {
            Ok(_) => Err(UserOpError::new("User already registered")),
            Err(_) => diesel::insert_into(users::table)
                        .values(new_user)
                        .execute(db)
                        .map(|_| ())
                        .map_err(|_| UserOpError::new("Database error"))
        }
    }

    pub fn find_user_by_email(db: &SqliteConnection, user_email: &str) -> Result<User, UserOpError> {
        let mut results = users.filter(email.eq(user_email))
            .limit(1)
            .load::<User>(db)
            .map_err(|_| UserOpError::new("Database error"))?;
        if results.is_empty() {
            Result::Err(UserOpError::new("No matching user found"))
        } else {
            Result::Ok(results.remove(0)) // Take ownership, kill the stupid Vec
        }
    }

    // Create a JWT token for the current user if password matches
    pub fn create_token(&self, passwd: &str) -> Result<String, UserOpError> {
        if passwd != self.password {
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
            ).signed(env::var("SFRS_JWT_SECRET")
                .expect("Please have SFRS_JWT_SECRET set")
                .as_bytes(), crypto::sha2::Sha256::new())
             .map_err(|_| UserOpError::new("Failed to generate token"))
        }
    }

    // Change the password in database, if old password is provided
    // The current instance of User model will not be mutated
    pub fn change_pw(&self, db: &SqliteConnection, passwd: &str, new_passwd: &str) -> Result<(), UserOpError> {
        if passwd != self.password {
            Err(UserOpError::new("Password mismatch"))
        } else {
            // Update database
            // TODO: Maybe we should revoke all JWTs somehow?
            //      maybe we can record when the user last changed?
            diesel::update(users.find(self.id))
                .set(password.eq(new_passwd))
                .execute(db)
                .map(|_| ())
                .map_err(|_| UserOpError::new("Database error"))
        }
    }
}