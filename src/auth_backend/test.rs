use anyhow::{bail, Result};
use async_trait::async_trait;

use crate::auth_backend::{AuthBackend, AuthCache, LoginType, UserInfo};

pub struct Test;

#[async_trait]
impl AuthBackend for Test {
    fn get_login_type(&self, _: &str, _: &AuthCache) -> Result<LoginType> {
        Ok(LoginType::Mask)
    }

    fn login(&self, username: &str, password: &str) -> Result<UserInfo> {
        if username == "test" && password == "test" {
            return Ok(UserInfo {
                name: username.to_string(),
            });
        }

        bail!("username or password incorrect");
    }
}