use serde::Deserialize;

#[derive(Clone, Default, Deserialize)]
pub enum AuthBackend {
    #[default]
    None,
    Test,
    #[serde(alias = "LDAP", alias = "ldap")]
    Ldap,
    #[serde(alias = "OIDC", alias = "oidc")]
    Oidc,
    #[serde(alias = "htpasswd")]
    Htpasswd,
}

#[derive(Clone, Default, Deserialize)]
pub enum DbBackend {
    Test,
    #[default]
    #[serde(alias = "filesystem")]
    Filesystem,
    #[serde(alias = "HTTP", alias = "http")]
    Http,
}

