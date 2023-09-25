use anyhow::Result;

use crate::auth_backend::ldap::Ldap;
use crate::auth_backend::none::None;
use crate::auth_backend::test::Test;
use crate::config::backend;
use crate::config::config::Config;

pub mod test;
pub mod ldap;
pub mod none;

pub struct UserInfo {
    pub name: String,
}

pub trait AuthBackend {
    fn login(&self, username: &str, password: &str) -> Result<UserInfo>;
    //  TODO: handle case sensitivity
}

pub fn new(config: &Config) -> Box<dyn AuthBackend> {
    match config.auth_backend {
        backend::AuthBackend::None => Box::new(None {}),
        backend::AuthBackend::Test => Box::new(Test {}),
        backend::AuthBackend::Ldap => Box::new(Ldap::new(config)),
    }
}
