use anyhow::{anyhow, Result};
use async_trait::async_trait;
use tokio::fs::File;
use tokio::io;
use tokio::io::AsyncBufReadExt;

use crate::auth_backend::{AuthBackend, AuthCache, InvalidCredentialsError, LoginType, UserInfo};
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
        // a password file that cannot be read is the operator's problem, not
        // the visitor's, so it stays a plain error
        let file = File::open(&self.config.path)
            .await
            .map_err(|err| anyhow!("failed to read '{}': {}", self.config.path.display(), err))?;
        let mut lines = io::BufReader::new(file).lines();

        while let Some(line) = lines.next_line().await? {
            if htpasswd_verify::Htpasswd::from(line.as_str()).check(username, password) {
                return Ok(());
            }
        }

        Err(InvalidCredentialsError.into())
    }
}

#[async_trait]
impl AuthBackend for Htpasswd {
    fn validate_config(&self) -> Result<()> {
        self.config.validate()
    }

    async fn get_login_type(&self, _: &str, _: &AuthCache) -> Result<LoginType> {
        Ok(LoginType::Mask)
    }

    async fn login(&self, username: &str, password: &str) -> Result<UserInfo> {
        self.match_user(username, password).await?;

        Ok(
            UserInfo {
                // lowercase like the ldap backend, the id keys the db cache
                id: username.to_lowercase(),
                name: username.to_owned(),
                db_location: None,
                keyfile_location: None,
                additional_data: None,
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::auth_backend::is_invalid_credentials;

    use super::*;

    fn backend(path: &str) -> Htpasswd {
        let mut config = Config::default();
        config.htpasswd.path = path.into();

        Htpasswd::new(&config)
    }

    #[tokio::test]
    async fn a_wrong_password_is_told_apart_from_a_password_file_that_cannot_be_read() {
        let path = std::env::temp_dir().join("k4w-htpasswd-test");
        // alice:alicepass, bcrypt
        std::fs::write(&path, "alice:$2y$05$8ZK.ZQ1RRDpYqQFZ.d8lF.7X0Zz9pBqxYb0KZ5v5yGmVYqQZ8kJ5W\n").unwrap();

        let wrong = backend(path.to_str().unwrap())
            .login("alice", "not the password").await
            .err().expect("a wrong password is a failure");
        assert!(is_invalid_credentials(&wrong), "wrong password: {:#}", wrong);

        let missing = backend("/nonexistent/htpasswd")
            .login("alice", "whatever").await
            .err().expect("an unreadable file is a failure");
        assert!(
            !is_invalid_credentials(&missing),
            "an unreadable password file must not read as a wrong password: {:#}", missing,
        );
        assert!(missing.to_string().contains("/nonexistent/htpasswd"), "{:#}", missing);

        let _ = std::fs::remove_file(&path);
    }
}
