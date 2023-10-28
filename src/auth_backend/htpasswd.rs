use anyhow::{bail, Result};
use async_trait::async_trait;
use tokio::fs::File;
use tokio::io;
use tokio::io::AsyncBufReadExt;

use crate::auth_backend::{AuthBackend, AuthCache, LoginType, UserInfo};
use crate::config::config::Config;
use crate::config::htpasswd;

pub struct Htpasswd {
    pub(crate) config: htpasswd::Htpasswd,
}

impl Htpasswd {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.htpasswd.clone()
        }
    }

    async fn match_user(&self, username: &str, password: &str) -> Result<()> {
        let file = File::open(&self.config.path).await?;
        let mut lines = io::BufReader::new(file).lines();

        while let Some(line) = lines.next_line().await? {
            if htpasswd_verify::Htpasswd::from(line.as_str()).check(username, password) {
                return Ok(());
            }
        }

        bail!("username or password incorrect");
    }
}

#[async_trait]
impl AuthBackend for Htpasswd {
    fn validate_config(&self) -> Result<()> {
        self.config.validate()
    }

    fn get_login_type(&self, _: &str, _: &AuthCache) -> Result<LoginType> {
        Ok(LoginType::Mask)
    }

    async fn login(&self, username: &str, password: &str) -> Result<UserInfo> {
        self.match_user(username, password).await?;

        Ok(
            UserInfo {
                id: username.to_owned(),
                name: username.to_owned(),
                db_location: None,
                keyfile_location: None,
                additional_data: None,
            }
        )
    }
}
