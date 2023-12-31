use std::any::Any;

use anyhow::{bail, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::auth_backend::htpasswd::Htpasswd;
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
pub mod htpasswd;

pub const SESSION_KEY_AUTH_STATE: &str = "auth_state";
pub const ROUTE_CALLBACK_USER_AUTH: &str = "/callback_user_auth";

pub type AuthCache = Box<dyn Any + Send + Sync>;

#[derive(Default, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: String,
    pub name: String,
    pub db_location: Option<String>,
    pub keyfile_location: Option<String>,
    pub additional_data: Option<String>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type")]
pub enum LoginType {
    None,
    Mask,
    Redirect {
        url: Url,
        #[serde(skip)]
        state: String,
    },
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "lowercase")]
#[serde(tag = "type")]
pub enum LogoutType {
    None,
    Redirect {
        url: Url,
    },
}

#[async_trait]
pub trait AuthBackend: Send + Sync {
    fn validate_config(&self) -> Result<()> { Ok(()) }

    async fn init(&self) -> Result<AuthCache> { Ok(Box::new(())) }

    fn get_login_type(&self, host: &str, cache: &AuthCache) -> Result<LoginType>;

    fn get_logout_type(&self, _user_info: &UserInfo, _host: &str, _cache: &AuthCache) -> Result<LogoutType> { Ok(LogoutType::None) }

    //  TODO: handle case sensitivity
    async fn login(&self, _username: &str, _password: &str) -> Result<UserInfo> {
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
        backend::AuthBackend::Htpasswd => Box::new(Htpasswd::new(config)),
    }
}
