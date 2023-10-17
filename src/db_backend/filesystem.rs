use std::any::Any;
use std::fs::File;
use std::io::Read;

use anyhow::Result;

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

    fn get_db_read(&self) -> Result<Box<dyn Read>> {
        Ok(
            Box::new(File::open(
                self.config.db_location.as_path()
            )?)
        )
    }

    fn get_key_read(&self) -> Option<Result<Box<dyn Read>>> {
        // return key file only if the key file location was configured
        if let Some(loc) = self.config.keyfile_location.as_ref() {
            return match File::open(loc) {
                Ok(keyfile) => {
                    Some(Ok(Box::new(keyfile)))
                }
                Err(err) => Some(Err(err.into())),
            };
        }

        None
    }

    fn get_db_write(&mut self) -> Result<Box<dyn std::io::Write>> {
        Ok(
            Box::new(File::open(
                self.config.db_location.as_path()
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
