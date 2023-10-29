use std::any::Any;
use std::path::Path;
use std::pin::Pin;

use anyhow::Result;
use async_trait::async_trait;
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::oneshot::Receiver;

use crate::auth_backend::UserInfo;
use crate::config::config::Config;
use crate::config::filesystem;
use crate::db_backend::DbBackend;

pub struct Filesystem {
    pub config: filesystem::Filesystem,
}

#[async_trait]
impl DbBackend for Filesystem {
    fn authenticated(&self) -> bool {
        true
    }

    async fn get_db_read(&self, user_info: &UserInfo) -> Result<Pin<Box<dyn AsyncRead + '_>>> {
        let mut path = self.config.db_location.as_path();

        if let Some(db_location) = &user_info.db_location {
            path = Path::new(db_location);
        }

        Ok(
            Box::pin(File::open(
                path
            ).await?)
        )
    }

    async fn get_key_read(&self, user_info: &UserInfo) -> Option<Result<Pin<Box<dyn AsyncRead + '_>>>> {
        let mut path = None;
        if let Some(p) = &user_info.keyfile_location {
            path = Some(Path::new(p));
        } else if let Some(p) = &self.config.keyfile_location {
            path = Some(p.as_path())
        }

        // return key file only if the key file location was configured
        if let Some(loc) = path {
            return match File::open(loc).await {
                Ok(keyfile) => {
                    Some(Ok(Box::pin(keyfile)))
                }
                Err(err) => Some(Err(err.into())),
            };
        }

        None
    }

    async fn get_db_write(&mut self, user_info: &UserInfo) -> Result<(Pin<Box<dyn AsyncWrite + '_>>, Option<Receiver<Result<()>>>)> {
        let mut path = self.config.db_location.as_path();

        if let Some(db_location) = &user_info.db_location {
            path = Path::new(db_location);
        }

        Ok(
            (
                Box::pin(File::open(
                    path
                ).await?),
                None
            )
        )
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }

    fn validate_config(&self) -> Result<()> {
        self.config.validate()
    }
}

impl Filesystem {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.filesystem.clone()
        }
    }
}
