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
        // TODO: allow empty if provided by auth backend
        if self.db_location.as_os_str().is_empty() {
            bail!("Filesystem: db location must be specified");
        }
        Ok(())
    }
}