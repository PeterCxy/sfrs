use std::collections::HashMap;
use std::sync::{Arc, RwLock, Mutex};

// A per-user lock used for sync requests
pub struct UserLock {
    lock_map: RwLock<HashMap<i32, Arc<Mutex<()>>>>
}

impl UserLock {
    pub fn new() -> UserLock {
        UserLock {
            lock_map: RwLock::new(HashMap::new())
        }
    }

    pub fn get_mutex(&self, uid: i32) -> Arc<Mutex<()>> {
        if !self.lock_map.read().unwrap().contains_key(&uid) {
            self.lock_map.write().unwrap().insert(uid, Arc::new(Mutex::new(())));
        }

        self.lock_map.read().unwrap().get(&uid).unwrap().clone()
    }
}