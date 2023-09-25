use std::fs::File;
use std::path::PathBuf;
use std::time::Duration;

use actix_web::cookie;
use anyhow::Result;
use serde::Deserialize;
use serde_yaml::from_reader;

use crate::config::backend::{AuthBackend, DbBackend};
use crate::config::filesystem::Filesystem;
use crate::config::key::Key;
use crate::config::ldap::Ldap;
use crate::config::search::Search;

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(alias = "server")]
    pub listen: String,
    pub port: u16,
    #[serde(with = "humantime_serde")]
    pub db_session_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub auth_check_interval: Duration,
    pub auth_backend: AuthBackend,
    pub db_backend: DbBackend,
    pub session_secret_key: Key,
    pub search: Search,
    #[serde(alias = "LDAP", alias = "Ldap")]
    pub ldap: Ldap,
    #[serde(alias = "Filesystem")]
    pub filesystem: Filesystem,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            listen: "127.0.0.1".to_string(),
            port: 8080,
            // 10 minutes
            db_session_timeout: Duration::from_secs(10 * 60),
            // 1 hour, 5 minutes
            auth_check_interval: Duration::from_secs(60 * 60 + 5 * 60),
            auth_backend: Default::default(),
            db_backend: Default::default(),
            session_secret_key: Key(cookie::Key::generate()),
            search: Default::default(),
            ldap: Default::default(),
            filesystem: Default::default(),
        }
    }
}

impl Config {
    pub fn from_file(filename: PathBuf) -> Result<Self> {
        let file = File::open(filename)?;
        let conf: Config = from_reader(file)?;

        conf.validate()?;

        Ok(conf)
    }
    pub fn validate(&self) -> Result<()> {
        self.filesystem.validate()
    }
}
