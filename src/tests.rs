use crate::build_rocket;
use rocket::local::Client;
use rocket::http::{ContentType, Status};
use lazy_static::*;

fn get_test_client() -> Client {
    dotenv::from_filename(".env.test").unwrap();
    Client::new(build_rocket())
        .expect("valid rocket instance")
}

lazy_static! {
    static ref CLIENT: Client = get_test_client();
}

#[test]
fn should_add_user() {
    let mut resp = CLIENT
        .post("/auth")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test@example.com",
            "password": "testpw",
            "pw_cost": "100",
            "pw_nonce": "whatever",
            "version": "001"
        }"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    assert!(resp.body_string().unwrap().contains(r#"{"token":"#));
}

#[test]
fn should_not_add_user_twice() {
    CLIENT.post("/auth")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test1@example.com",
            "password": "testpw",
            "pw_cost": "100",
            "pw_nonce": "whatever",
            "version": "001"
        }"#)
        .dispatch()
        .body_string()
        .unwrap();
    let resp = CLIENT
        .post("/auth")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test1@example.com",
            "password": "does not matter",
            "pw_cost": "100",
            "pw_nonce": "whatever",
            "version": "001"
        }"#)
        .dispatch();
    assert_eq!(resp.status(), Status::InternalServerError);
}

#[test]
fn should_log_in_successfully() {
    CLIENT.post("/auth")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test2@example.com",
            "password": "testpw",
            "pw_cost": "100",
            "pw_nonce": "whatever",
            "version": "001"
        }"#)
        .dispatch()
        .body_string()
        .unwrap();
    let mut resp = CLIENT
        .post("/auth/sign_in")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test2@example.com",
            "password": "testpw"
        }"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    let body = resp.body_string().unwrap();
    //println!("{}", body);
    assert!(body.contains(r#"{"token":"#));
}

#[test]
fn should_log_in_fail() {
    CLIENT.post("/auth")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test3@example.com",
            "password": "testpw",
            "pw_cost": "100",
            "pw_nonce": "whatever",
            "version": "001"
        }"#)
        .dispatch()
        .body_string()
        .unwrap();
    let resp = CLIENT
        .post("/auth/sign_in")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test3@example.com",
            "password": "testpw1"
        }"#)
        .dispatch();
    assert_eq!(resp.status(), Status::InternalServerError);
}