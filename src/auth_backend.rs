use std::any::Any;
use std::fmt::{Display, Formatter};

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

// A login fails either because the credentials were wrong or because the
// backend could not answer, and the two want different treatment: the first is
// the user's to correct, the second is the operator's, and telling a user their
// password is wrong when the directory is unreachable sends them chasing the
// wrong thing.
#[derive(Debug, Clone)]
pub struct InvalidCredentialsError;

impl Display for InvalidCredentialsError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "username or password incorrect")
    }
}

impl std::error::Error for InvalidCredentialsError {}

pub fn is_invalid_credentials(err: &anyhow::Error) -> bool {
    err.downcast_ref::<InvalidCredentialsError>().is_some()
}

#[async_trait]
pub trait AuthBackend: Send + Sync {
    fn validate_config(&self) -> Result<()> { Ok(()) }

    async fn init(&self) -> Result<AuthCache> { Ok(Box::new(())) }

    async fn get_login_type(&self, host: &str, cache: &AuthCache) -> Result<LoginType>;

    async fn get_logout_type(&self, _user_info: &UserInfo, _host: &str, _cache: &AuthCache) -> Result<LogoutType> { Ok(LogoutType::None) }

    // backends returning case-insensitive user names must lowercase
    // UserInfo.id: it keys the db cache (see ldap/htpasswd). OIDC subjects
    // are case-sensitive opaque strings and are used as-is.
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
