use serde::Deserialize;

#[derive(Clone, Default, Deserialize)]
pub enum AuthBackend {
    #[default]
    None,
    Test,
    #[serde(alias = "LDAP")]
    #[serde(alias = "ldap")]
    Ldap,
}

#[derive(Clone, Default, Deserialize)]
pub enum DbBackend {
    Test,
    #[default]
    Filesystem,
}

