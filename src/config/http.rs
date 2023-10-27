use serde::Deserialize;
use url::Url;

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Credentials {
    pub username: String,
    pub password: Option<String>,
}

#[derive(Clone, Default, Deserialize)]
#[serde(default)]
pub struct Http {
    pub database_url: Option<Url>,
    pub keyfile_url: Option<Url>,
    pub credentials: Option<Credentials>,
    pub bearer: Option<String>,
}