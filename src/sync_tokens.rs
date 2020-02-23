use ring::aead::*;
use ring::digest::*;
use ring::pbkdf2::*;
use ring::rand::{SecureRandom, SystemRandom};

// In the API endpoint `/items/sync`, we use `max_id` of the
// current user as the sync token. However, this may be prone
// to side-channel leakage since all users in database share
// the same auto-incrementing ID. An attacker may be able to
// call `/items/sync` with one update each time and extract
// what others' are doing based on changes in ID.
// Therefore, we should at least not send the ID as a token
// in plain-text to the client.

lazy_static! {
    static ref TOKEN_KEY: [u8; 32] = get_token_key();
}

pub fn get_token_key() -> [u8; 32] {
    let pwd = std::env::var("SYNC_TOKEN_SECRET")
        .expect("Please set SYNC_TOKEN_SECRET").into_bytes();
    let salt = std::env::var("SYNC_TOKEN_SALT")
        .expect("Please set SYNC_TOKEN_SALT").into_bytes();
    let mut ret = [0; 32];
    derive(&SHA256, 100, &salt, &pwd, &mut ret);
    ret
}

pub fn max_id_to_token(max_id: i64) -> String {
    let sealing_key = SealingKey::new(&CHACHA20_POLY1305, &*TOKEN_KEY).unwrap();
    let mut nonce = [0u8; 12];
    SystemRandom::new().fill(&mut nonce).unwrap();
    let mut id_str = max_id.to_string().as_bytes().to_vec();
    id_str.resize(id_str.len() + CHACHA20_POLY1305.tag_len(), 0);
    let out_len = seal_in_place(&sealing_key, &nonce, &[], &mut id_str, CHACHA20_POLY1305.tag_len())
        .unwrap();
    let mut out = id_str[0..out_len].to_vec();
    out.extend_from_slice(&nonce);
    hex::encode(out)
}

pub fn token_to_max_id(token: &str) -> Result<i64, ()> {
    let opening_key = OpeningKey::new(&CHACHA20_POLY1305, &*TOKEN_KEY).unwrap();
    let data = hex::decode(token).map_err(|_| ())?;
    let len = data.len();
    if len <= 12 {
        return Err(());
    }

    let mut id_str = (&data[0..(len - 12)]).to_vec();
    let nonce = &data[(len - 12)..len];
    let decrypted = open_in_place(&opening_key, nonce, &[], 0, &mut id_str)
        .map_err(|_| ())?;
    String::from_utf8(decrypted.to_vec())
        .map_err(|_| ())?
        .parse()
        .map_err(|_| ())
}