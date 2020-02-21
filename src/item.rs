use crate::schema::items;
use crate::schema::items::dsl::*;
use crate::{lock_db_write, lock_db_read};
use crate::user;
use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use serde::{Serialize, Deserialize};
use std::vec::Vec;

#[derive(Debug)]
pub struct ItemOpError(pub String);

impl ItemOpError {
    fn new(s: impl Into<String>) -> ItemOpError {
        ItemOpError(s.into())
    }
}

impl Into<ItemOpError> for &str {
    fn into(self) -> ItemOpError {
        ItemOpError::new(self)
    }
}

#[derive(Queryable)]
pub struct Item {
    pub id: i64,
    pub owner: i32,
    pub uuid: String,
    pub content: Option<String>,
    pub content_type: String,
    pub enc_item_key: Option<String>,
    pub deleted: bool,
    pub created_at: String,
    pub updated_at: Option<String>
}

#[derive(Insertable)]
#[table_name = "items"]
struct InsertItem {
    owner: i32,
    uuid: String,
    content: Option<String>,
    content_type: String,
    enc_item_key: Option<String>,
    deleted: bool,
    created_at: String,
    updated_at: Option<String>
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SyncItem {
    pub uuid: String,
    pub content: Option<String>,
    pub content_type: String,
    pub enc_item_key: Option<String>,
    #[serde(default)]
    pub deleted: bool,
    pub created_at: String,
    pub updated_at: Option<String>
}

impl Into<SyncItem> for Item {
    fn into(self) -> SyncItem {
        SyncItem {
            uuid: self.uuid,
            content: self.content,
            content_type: self.content_type,
            enc_item_key: self.enc_item_key,
            deleted: self.deleted,
            created_at: self.created_at,
            updated_at: self.updated_at
        }
    }
}

impl SyncItem {
    pub fn items_of_user(
        db: &SqliteConnection, u: &user::User,
        since_id: Option<i64>, max_id: Option<i64>,
        limit: Option<i64>
    ) -> Result<Vec<Item>, ItemOpError> {
        lock_db_read!()
            .and_then(|_| {
                let mut stmt = items.filter(owner.eq(u.id)).into_boxed();
                if let Some(limit) = limit {
                    stmt = stmt.limit(limit);
                }

                if let Some(since_id) = since_id {
                    stmt = stmt.filter(id.gt(since_id));
                }

                if let Some(max_id) = max_id {
                    stmt = stmt.filter(id.le(max_id));
                }

                stmt.order(id.asc())
                    .load::<Item>(db)
                    .map_err(|_| "Database error".into())
            })
    }

    pub fn find_item_by_uuid(db: &SqliteConnection, u: &user::User, i: &str) -> Result<Item, ItemOpError> {
        lock_db_read!()
            .and_then(|_| {
                items.filter(owner.eq(u.id).and(uuid.eq(i)))
                    .first::<Item>(db)
                    .map_err(|_| "Database error".into())
            })
    }

    pub fn items_insert(db: &SqliteConnection, u: &user::User, it: &SyncItem) -> Result<i64, ItemOpError> {
        // First, try to find the original item, if any, delete it, and insert a new one with the same UUID
        // This way, the ID is updated each time an item is updated
        // This method acts both as insertion and update
        let orig = lock_db_read!()
            .and_then(|_| {
                items.filter(uuid.eq(&it.uuid).and(owner.eq(u.id)))
                    .load::<Item>(db)
                    .map_err(|_| "Database error".into())
            })?;
        // TODO: Detect sync conflict? similar to the Go version.

        let _lock = lock_db_write!()?;
        if !orig.is_empty() {
            diesel::delete(items.filter(uuid.eq(&it.uuid).and(owner.eq(u.id))))
                .execute(db)
                .map(|_| ())
                .map_err(|_| "Database error".into())?;
        }

        diesel::insert_into(items::table)
            .values(InsertItem {
                owner: u.id,
                uuid: it.uuid.clone(),
                content: if it.deleted { None } else { it.content.clone() },
                content_type: it.content_type.clone(),
                enc_item_key: if it.deleted { None } else { it.enc_item_key.clone() },
                deleted: it.deleted,
                created_at: it.created_at.clone(),
                updated_at: it.updated_at.clone()
            })
            .execute(db)
            .map_err(|_| "Database error".into())?;
        std::mem::drop(_lock);

        Self::find_item_by_uuid(db, u, &it.uuid)
            .map(|i| i.id)
    }
}