#![feature(proc_macro_hygiene, decl_macro)]

#[macro_use]
extern crate rocket;
#[macro_use]
extern crate rocket_contrib;
#[macro_use]
extern crate diesel;
#[macro_use]
extern crate diesel_migrations;
extern crate dotenv;
#[macro_use]
extern crate serde;

mod schema;
mod api;
mod user;

use diesel::prelude::*;
use diesel::sqlite::SqliteConnection;
use dotenv::dotenv;
use rocket::Rocket;
use rocket::config::{Config, Environment, Value};
use rocket::fairing::AdHoc;
use std::collections::HashMap;
use std::env;

embed_migrations!();

#[database("db")]
pub struct DbConn(SqliteConnection);

#[get("/")]
fn index() -> &'static str {
    "Hello, world!"
}

fn db_path() -> String {
    env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set")
}

fn db_config() -> HashMap<&'static str, Value> {
    let mut database_config = HashMap::new();
    let mut databases = HashMap::new();

    database_config.insert("url", Value::from(db_path()));
    databases.insert("db", Value::from(database_config));

    return databases;
}

fn get_environment() -> Environment {
    let v = env::var("SFRS_ENV").unwrap_or("development".to_string());

    if v == "development" {
        Environment::Development
    } else {
        Environment::Production
    }
}

fn build_config() -> Config {
    Config::build(get_environment())
        .extra("databases", db_config())
        .finalize()
        .unwrap()
}

fn run_db_migrations(rocket: Rocket) -> Result<Rocket, Rocket> {
    let db = DbConn::get_one(&rocket).expect("Could not connect to Database");
    match embedded_migrations::run(&*db) {
        Ok(()) => Ok(rocket),
        Err(e) => {
            // We should not do anything if database failed to migrate
            panic!("Failed to run database migrations: {:?}", e);
        }
    }
}

fn main() {
    dotenv().ok();
    rocket::custom(build_config())
        .attach(DbConn::fairing())
        .attach(AdHoc::on_attach("Database Migrations", run_db_migrations))
        .mount("/", api::routes())
        .launch();
}
