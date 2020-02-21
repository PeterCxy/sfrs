use crate::schema::tokens;
use crate::schema::tokens::dsl::*;
use crate::{lock_db_write, lock_db_read};
use chrono::NaiveDateTime;
use diesel::sqlite::SqliteConnection;
use diesel::prelude::*;
use std::sync::{RwLockReadGuard, RwLockWriteGuard};
use uuid::Uuid;

#[derive(Queryable, Insertable)]
#[table_name = "tokens"]
pub struct Token {
    id: String,
    uid: i32,
    timestamp: Option<NaiveDateTime>
}

impl Token {
    // Return user id if any
    pub fn find_token_by_id(db: &SqliteConnection, tid: &str) -> Option<i32> {
        (lock_db_read!() as Result<RwLockReadGuard<()>, String>).ok()
            .and_then(|_| {
                tokens.filter(id.eq(tid))
                    .load::<Token>(db)
                    .ok()
                    .and_then(|mut v| {
                        if !v.is_empty() {
                            Some(v.remove(0).uid)
                        } else {
                            None
                        }
                    })
            })
    }

    // Create a new token for a user
    pub fn create_token(db: &SqliteConnection, user: i32) -> Option<String> {
        let tid = Uuid::new_v4().to_hyphenated().to_string();
        (lock_db_write!() as Result<RwLockWriteGuard<()>, String>).ok()
            .and_then(|_| {
                diesel::insert_into(tokens::table)
                    .values(Token {
                        id: tid.clone(),
                        uid: user,
                        timestamp: None // There's default value from SQLite
                    })
                    .execute(db)
                    .ok()
                    .map(|_| tid)
            })
    }
}