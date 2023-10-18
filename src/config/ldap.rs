use serde::Deserialize;

#[derive(Clone, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    #[default]
    Subtree,
    Base,
    OneLevel,
}

impl From<Scope> for ldap3::Scope {
    fn from(val: Scope) -> Self {
        match val {
            Scope::Subtree => ldap3::Scope::Subtree,
            Scope::Base => ldap3::Scope::Base,
            Scope::OneLevel => ldap3::Scope::OneLevel,
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct Ldap {
    pub uri: String,
    pub scope: Scope,
    pub base_dn: String,
    pub filter: String,
    pub login_attribute: String,
    pub bind: String,
    pub password: String,
    pub database_attribute: Option<String>,
    pub keyfile_attribute: Option<String>,
}

impl Default for Ldap {
    fn default() -> Self {
        Ldap {
            uri: "ldap://localhost:339".to_string(),
            scope: Scope::default(),
            base_dn: "".to_string(),
            filter: "()".to_string(),
            login_attribute: "uid".to_string(),
            bind: "".to_string(),
            password: "".to_string(),
            database_attribute: None,
            keyfile_attribute: None,
        }
    }
}
