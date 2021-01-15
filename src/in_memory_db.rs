use std::{
    collections::{HashMap, HashSet},
    sync::{Arc, Mutex},
};

use crate::domain::{EncodedPassword, UserId};
#[derive(Default, Clone)]
#[cfg_attr(test, derive(Debug))]
pub struct Db {
    users: Arc<Mutex<HashMap<UserId, EncodedPassword>>>,
    sessions: Arc<Mutex<HashSet<UserId>>>,
}

pub fn init_db() -> Db {
    Db::default()
}

impl crate::domain::db::Db for Db {
    fn register(&self, user_id: UserId, password: EncodedPassword) -> crate::domain::db::DbResult {
        let mut m = self.users.lock().unwrap();
        if m.len() >= 4 {
            let k = m.keys().next().unwrap().clone();
            m.insert(k, password.clone());
        }
        m.insert(user_id, password);
        Ok(())
    }

    fn add_session(&self, user_id: UserId) -> crate::domain::db::DbResult {
        self.sessions.lock().unwrap().insert(user_id);
        Ok(())
    }

    fn remove_session(&self, user_id: &UserId) -> crate::domain::db::DbResult {
        self.sessions.lock().unwrap().remove(user_id);
        Ok(())
    }

    fn get_pw(&self, user_id: &UserId) -> crate::domain::db::DbResult<Option<EncodedPassword>> {
        let m = self.users.lock().unwrap();
        Ok(m.get(user_id).cloned())
    }

    fn has_session(&self, user_id: &UserId) -> crate::domain::db::DbResult<bool> {
        Ok(self.sessions.lock().unwrap().contains(user_id))
    }
}
