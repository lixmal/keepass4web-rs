use serde::Deserialize;

#[derive(Clone, Default, Deserialize)]
pub enum AuthBackend {
    #[default]
    None,
    Test,
    #[serde(alias = "LDAP", alias = "ldap")]
    Ldap,
    #[serde(alias = "OIDC", alias = "Oidc")]
    Oidc,
}

#[derive(Clone, Default, Deserialize)]
pub enum DbBackend {
    Test,
    #[default]
    Filesystem,
    Http,
}

