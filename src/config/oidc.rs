use anyhow::bail;
use anyhow::Result;
use serde::Deserialize;
use url::Url;

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct Oidc {
    pub issuer: Option<Url>,
    /// Override the URL used to fetch OIDC provider metadata.
    /// Useful in Docker/container setups where the app reaches the provider
    /// via an internal hostname (e.g. `http://keycloak:8180`) but the issuer
    /// in tokens is an external hostname (e.g. `http://localhost:8180`).
    /// When unset, metadata is fetched from `<issuer>/.well-known/openid-configuration`.
    pub discovery_url: Option<Url>,
    pub client_id: String,
    pub client_secret: String,
    #[serde(deserialize_with = "crate::config::env::list")]
    pub scopes: Vec<String>,
    #[serde(deserialize_with = "crate::config::env::scalar")]
    pub save_id_token: bool,
}

impl Default for Oidc {
    fn default() -> Self {
        Oidc {
            issuer: None,
            discovery_url: None,
            client_id: "".to_string(),
            client_secret: "".to_string(),
            scopes: vec![],
            save_id_token: true,
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
