use std::path::PathBuf;

use anyhow::{bail, Result};
use serde::Deserialize;

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Htpasswd {
    pub path: PathBuf,
}

impl Htpasswd {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.path.as_os_str().is_empty() {
            bail!("Htpasswd: path must be specified");
        }
        Ok(())
    }
}