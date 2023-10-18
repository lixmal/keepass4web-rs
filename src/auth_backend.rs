use std::any::Any;

use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::Serialize;
use url::Url;

use crate::auth_backend::ldap::Ldap;
use crate::auth_backend::none::None;
use crate::auth_backend::oidc::Oidc;
use crate::auth_backend::test::Test;
use crate::config::backend;
use crate::config::config::Config;

pub mod test;
pub mod ldap;
pub mod none;
pub mod oidc;

pub const SESSION_KEY_AUTH_STATE: &str = "auth_state";

pub type AuthCache = Box<dyn Any + Send + Sync>;

pub struct UserInfo {
    pub id: String,
    pub name: String,
    pub db_location: Option<String>,
    pub keyfile_location: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type")]
pub enum LoginType {
    Mask,
    Redirect {
        url: Url,
        #[serde(skip)]
        state: String,
    },
}

#[async_trait]
pub trait AuthBackend: Send + Sync {
    fn validate_config(&self) -> Result<()> { Ok(()) }

    async fn init(&self) -> Result<AuthCache> { Ok(Box::new(())) }

    fn get_login_type(&self, host: &str, cache: &AuthCache) -> Result<LoginType>;

    //  TODO: handle case sensitivity
    fn login(&self, _username: &str, _password: &str) -> Result<UserInfo> {
        bail!("login method not supported")
    }

    async fn callback(&self, _from_session: String, _cache: &AuthCache, _params: serde_json::Value, _host: &str) -> Result<UserInfo> {
        bail!("login method not supported")
    }
}

pub fn new(config: &Config) -> Box<dyn AuthBackend> {
    match config.auth_backend {
        backend::AuthBackend::None => Box::new(None {}),
        backend::AuthBackend::Test => Box::new(Test {}),
        backend::AuthBackend::Ldap => Box::new(Ldap::new(config)),
        backend::AuthBackend::Oidc => Box::new(Oidc::new(config)),
    }
}
