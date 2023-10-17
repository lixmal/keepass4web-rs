use anyhow::bail;
use anyhow::Result;
use serde::Deserialize;
use url::Url;

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct Oidc {
    pub issuer: Option<Url>,
    pub client_id: String,
    pub client_secret: String,
    pub scopes: Vec<String>,
}

impl Default for Oidc {
    fn default() -> Self {
        Oidc {
            issuer: None,
            client_id: "".to_string(),
            client_secret: "".to_string(),
            scopes: vec![],
        }
    }
}

impl Oidc {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.issuer.is_none() {
            bail!("issuer cannot be empty");
        }
        if self.client_id.is_empty() {
            bail!("OIDC: client_id must be specified");
        }
        if self.client_secret.is_empty() {
            bail!("OIDC: secret_key must be specified");
        }
        Ok(())
    }
}
