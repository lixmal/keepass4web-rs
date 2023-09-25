use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::Deserialize;

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Filesystem {
    pub db_location: PathBuf,
    pub keyfile_location: Option<PathBuf>,
}

impl Filesystem {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.db_location.as_os_str().is_empty() {
            bail!("db location must be specified");
        }
        Ok(())
    }
}