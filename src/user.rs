use crate::schema::users;
use crate::schema::users::dsl::*;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use serde::Deserialize;

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
}