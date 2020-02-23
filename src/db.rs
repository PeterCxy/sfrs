use diesel::connection::{SimpleConnection, Connection};
use diesel::deserialize::{Queryable, QueryableByName};
use diesel::query_builder::{AsQuery, QueryFragment, QueryId};
use diesel::result::{ConnectionResult, QueryResult};
use diesel::sqlite::{Sqlite, SqliteConnection};
use diesel::sql_types::*;
use rocket_contrib::databases::{r2d2, DatabaseConfig, Poolable};
use std::sync::RwLock;

// We need a global RwLock for SQLite
// This is unfortunate when we still use SQLite
// but should be mostly fine for our purpose
// (however, due to disk sync delays, the RwLock alone
//  may still produce some SQLITE_BUSY errors randomly.
//  We implemented a wrapper later in this module to enable busy_timeout
//  to avoid this.)
lazy_static! {
    pub static ref DB_LOCK: RwLock<()> = RwLock::new(());
}

#[macro_export]
macro_rules! lock_db_write {
    () => {
        crate::DB_LOCK.write()
            .map_err(|_| "Cannot lock database for writing".into())
    };
}

#[macro_export]
macro_rules! lock_db_read {
    () => {
        crate::DB_LOCK.read()
            .map_err(|_| "Cannot lock database for reading".into())
    };
}

pub trait SqliteLike = Connection<Backend = Sqlite>;

pub struct BusyWaitSqliteConnection(SqliteConnection);

impl Poolable for BusyWaitSqliteConnection {
    type Manager = diesel::r2d2::ConnectionManager<BusyWaitSqliteConnection>;
    type Error = r2d2::Error;

    fn pool(config: DatabaseConfig) -> Result<r2d2::Pool<Self::Manager>, Self::Error> {
        let manager = diesel::r2d2::ConnectionManager::new(config.url);
        r2d2::Pool::builder().max_size(config.pool_size).build(manager)
    }
}

// Enable busy_timeout for SQLite connections by re-implementing the Connection trait
// (Note: busy_timeout is never the best solution, so the global RwLock is still needed,
//  and this busy_timeout is just to make sure that we won't fail due to disk sync lagging behind
//  when we acquire the RwLock because it may take some time for the SQLite lock state to be written to disk)
// <https://stackoverflow.com/questions/57123453/how-to-use-diesel-with-sqlite-connections-and-avoid-database-is-locked-type-of>
impl SimpleConnection for BusyWaitSqliteConnection {
    fn batch_execute(&self, query: &str) -> QueryResult<()> {
        self.0.batch_execute(query)
    }
}

impl Connection for BusyWaitSqliteConnection {
    type Backend = <SqliteConnection as Connection>::Backend;
    type TransactionManager = <SqliteConnection as Connection>::TransactionManager;

    fn establish(database_url: &str) -> ConnectionResult<Self> {
        let c = SqliteConnection::establish(database_url)?;
        c.batch_execute("PRAGMA foreign_keys = ON; PRAGMA busy_timeout = 60000;")
            .unwrap();
        Ok(Self(c))
    }

    fn execute(&self, query: &str) -> QueryResult<usize> {
        self.0.execute(query)
    }

    fn query_by_index<T, U>(&self, source: T) -> QueryResult<Vec<U>>
    where
        T: AsQuery,
        T::Query: QueryFragment<Self::Backend> + QueryId,
        Self::Backend: HasSqlType<T::SqlType>,
        U: Queryable<T::SqlType, Self::Backend>,
    {
        self.0.query_by_index(source)
    }

    fn query_by_name<T, U>(&self, source: &T) -> QueryResult<Vec<U>>
    where
        T: QueryFragment<Self::Backend> + QueryId,
        U: QueryableByName<Self::Backend>,
    {
        self.0.query_by_name(source)
    }

    fn execute_returning_count<T>(&self, source: &T) -> QueryResult<usize>
    where
        T: QueryFragment<Self::Backend> + QueryId,
    {
        self.0.execute_returning_count(source)
    }

    fn transaction_manager(&self) -> &Self::TransactionManager {
        self.0.transaction_manager()
    }
}