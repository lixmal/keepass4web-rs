use std::collections::HashMap;

use actix_session::Session;
use anyhow::{bail, Result};
use log::error;

use crate::auth::{SESSION_KEY_USER, SESSION_USER_UNKNOWN};

pub trait AuthSession {
    fn destroy(&self);
    fn get_key(&self, key: &str) -> Option<String>;
    fn get_keys(&self, keys: Vec<String>) -> Result<HashMap<String, String>>;
    fn insert_keys(&self, from_session: &HashMap<String, String>) -> Result<()>;
    fn get_username(&self) -> String;
    fn is_authorized(&self) -> bool;
}

impl AuthSession for Session {
    fn destroy(&self) {
        self.purge();
    }

    fn get_key(&self, key: &str) -> Option<String> {
        match self.get::<String>(key) {
            Ok(s) => s,
            Err(err) => {
                error!("failed to retrieve session: {}", err);
                None
            }
        }
    }

    fn get_keys(&self, keys: Vec<String>) -> Result<HashMap<String, String>> {
        let mut from_session: HashMap<String, String> = Default::default();
        for key in keys {
            match self.get_key(&key) {
                Some(v) => {
                    from_session.insert(key, v);
                }
                None => bail!("key not found in session: {}", &key),
            }
        }

        Ok(from_session)
    }

    fn insert_keys(&self, from_session: &HashMap<String, String>) -> Result<()> {
        for (key, value) in from_session {
            self.insert(key.clone(), value.clone())?
        }

        Ok(())
    }

    fn get_username(&self) -> String {
        match self.get_key(SESSION_KEY_USER) {
            Some(u) => u,
            None => SESSION_USER_UNKNOWN.to_string(),
        }
    }

    fn is_authorized(&self) -> bool {
        self.get_key(SESSION_KEY_USER).is_some()
    }
}
