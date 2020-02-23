use crate::build_rocket;
use rocket::local::Client;
use rocket::http::{Header, ContentType, Status};
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
fn sync_token_dec_1() {
    dotenv::from_filename(".env.test").unwrap();
    // We have to test decryption of a particular encrypted ID
    // to ensure we break nothing during updates
    let id = crate::sync_tokens::token_to_max_id("a3e43acc6c407dcb155598410be6524bfe483452b0c43b8c4cc8fe37ef183e6b6fc1").unwrap();
    assert_eq!(id, 114514);
}

#[test]
fn sync_token_dec_2() {
    dotenv::from_filename(".env.test").unwrap();
    // We have to test decryption of a particular encrypted ID
    // to ensure we break nothing during updates
    let id = crate::sync_tokens::token_to_max_id("cfb84e2eb08f8aaf959cc20a9f86225594abb0f0a40f56f692ea1475a00777f902a251").unwrap();
    assert_eq!(id, 1919810);
}

#[test]
fn sync_token_enc_dec_1() {
    dotenv::from_filename(".env.test").unwrap();
    let token = crate::sync_tokens::max_id_to_token(114514);
    let id = crate::sync_tokens::token_to_max_id(&token).unwrap();
    assert_eq!(id, 114514);
}

#[test]
fn sync_token_enc_dec_2() {
    dotenv::from_filename(".env.test").unwrap();
    let token = crate::sync_tokens::max_id_to_token(1919810);
    let id = crate::sync_tokens::token_to_max_id(&token).unwrap();
    assert_eq!(id, 1919810);
}


#[test]
fn should_add_user() {
    let mut resp = CLIENT
        .post("/auth")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test@example.com",
            "password": "testpw",
            "pw_cost": 100,
            "pw_nonce": "whatever",
            "version": "001"
        }"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    serde_json::from_str::<serde_json::Value>(&resp.body_string().unwrap()).unwrap()
        .get("token").unwrap().as_str().unwrap();
}

#[test]
fn should_not_add_user_twice() {
    CLIENT.post("/auth")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test1@example.com",
            "password": "testpw",
            "pw_cost": 100,
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
            "pw_cost": 100,
            "pw_nonce": "whatever",
            "version": "001"
        }"#)
        .dispatch();
    assert_eq!(resp.status(), Status::InternalServerError);
}

#[test]
fn should_not_add_user_invalid_email() {
    let resp = CLIENT
        .post("/auth")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test.example.com",
            "password": "testpw",
            "pw_cost": 100,
            "pw_nonce": "whatever",
            "version": "001"
        }"#)
        .dispatch();
    assert_eq!(resp.status(), Status::BadRequest);
}

#[test]
fn should_log_in_successfully() {
    CLIENT.post("/auth")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test2@example.com",
            "password": "testpw",
            "pw_cost": 100,
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
    serde_json::from_str::<serde_json::Value>(&resp.body_string().unwrap()).unwrap()
        .get("token").unwrap().as_str().unwrap();
}

#[test]
fn should_log_in_fail() {
    CLIENT.post("/auth")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test3@example.com",
            "password": "testpw",
            "pw_cost": 100,
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

#[test]
fn should_change_pw_successfully() {
    CLIENT.post("/auth")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test4@example.com",
            "password": "testpw",
            "pw_cost": 100,
            "pw_nonce": "whatever",
            "version": "001"
        }"#)
        .dispatch()
        .body_string()
        .unwrap();
    let resp = CLIENT
        .post("/auth/change_pw")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test4@example.com",
            "password": "testpw1",
            "current_password": "testpw"
        }"#)
        .dispatch();
    assert_eq!(resp.status(), Status::NoContent);
}

#[test]
fn should_change_pw_fail() {
    CLIENT.post("/auth")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test5@example.com",
            "password": "testpw",
            "pw_cost": 100,
            "pw_nonce": "whatever",
            "version": "001"
        }"#)
        .dispatch()
        .body_string()
        .unwrap();
    let resp = CLIENT
        .post("/auth/change_pw")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test5@example.com",
            "password": "testpw1",
            "current_password": "testpw2"
        }"#)
        .dispatch();
    assert_eq!(resp.status(), Status::InternalServerError);
}

#[test]
fn should_change_pw_successfully_and_log_in_successfully() {
    CLIENT.post("/auth")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test6@example.com",
            "password": "testpw",
            "pw_cost": 100,
            "pw_nonce": "whatever",
            "version": "001"
        }"#)
        .dispatch()
        .body_string()
        .unwrap();
    let resp = CLIENT
        .post("/auth/change_pw")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test6@example.com",
            "password": "testpw1",
            "current_password": "testpw"
        }"#)
        .dispatch();
    assert_eq!(resp.status(), Status::NoContent);
    let resp = CLIENT
        .post("/auth/sign_in")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test6@example.com",
            "password": "testpw1"
        }"#)
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
}

#[test]
fn should_fail_authorize() {
    let resp = CLIENT.get("/auth/ping").dispatch();
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[test]
fn should_fail_authorize_2() {
    let resp = CLIENT.get("/auth/ping")
        .header(Header::new("Authorization", "Bearer iwoe0nvie0bv024ibv043bv"))
        .dispatch();
    assert_eq!(resp.status(), Status::Unauthorized);
}

#[test]
fn should_success_authorize() {
    let token = CLIENT.post("/auth")
        .header(ContentType::JSON)
        .body(r#"{
            "email": "test7@example.com",
            "password": "testpw",
            "pw_cost": 100,
            "pw_nonce": "whatever",
            "version": "001"
        }"#)
        .dispatch()
        .body_string()
        .unwrap();
    let val = serde_json::from_str::<serde_json::Value>(&token).unwrap();
    let token = val.get("token").unwrap().as_str().unwrap();
    let mut resp = CLIENT.get("/auth/ping")
        .header(Header::new("Authorization", format!("Bearer {}", token)))
        .dispatch();
    assert_eq!(resp.status(), Status::Ok);
    assert_eq!(resp.body_string().unwrap(), "\"test7@example.com\"");
}