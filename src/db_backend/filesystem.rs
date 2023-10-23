use std::any::Any;
use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::Result;

use crate::auth_backend::UserInfo;
use crate::config::config::Config;
use crate::config::filesystem;
use crate::db_backend::DbBackend;

pub struct Filesystem {
    pub config: filesystem::Filesystem,
}

impl DbBackend for Filesystem {
    fn authenticated(&self) -> bool {
        true
    }

    fn get_db_read(&self, user_info: &UserInfo) -> Result<Box<dyn Read + '_>> {
        let mut path = self.config.db_location.as_path();

        if let Some(db_location) = &user_info.db_location {
            path = Path::new(db_location);
        }

        Ok(
            Box::new(File::open(
                path
            )?)
        )
    }

    // TODO implement in a non-blocking way
    fn get_key_read(&self, user_info: &UserInfo) -> Option<Result<Box<dyn Read + '_>>> {
        let mut path = None;
        if let Some(p) = &user_info.keyfile_location {
            path = Some(Path::new(p));
        } else if let Some(p) = &self.config.keyfile_location {
            path = Some(p.as_path())
        }

        // return key file only if the key file location was configured
        if let Some(loc) = path {
            return match File::open(loc) {
                Ok(keyfile) => {
                    Some(Ok(Box::new(keyfile)))
                }
                Err(err) => Some(Err(err.into())),
            };
        }

        None
    }

    fn get_db_write(&mut self, user_info: &UserInfo) -> Result<Box<dyn Write + '_>> {
        let mut path = self.config.db_location.as_path();

        if let Some(db_location) = &user_info.db_location {
            path = Path::new(db_location);
        }

        Ok(
            Box::new(File::open(
                path
            )?)
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
