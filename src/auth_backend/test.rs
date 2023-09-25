use anyhow::{bail, Result};

use crate::auth_backend::{AuthBackend, UserInfo};

pub struct Test;

impl AuthBackend for Test {
    fn login(&self, username: &str, password: &str) -> Result<UserInfo> {
        if username == "test" && password == "test" {
            return Ok(UserInfo {
                name: username.to_string(),
            });
        }

        bail!("username or password incorrect");
    }
}