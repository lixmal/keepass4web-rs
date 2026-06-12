use anyhow::{bail, Result};
use serde::Deserialize;
use url::Url;

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

impl Ldap {
    pub(crate) fn validate(&self) -> Result<()> {
        if self.uri.trim().is_empty() {
            bail!("LDAP: uri must be specified");
        }
        match Url::parse(&self.uri) {
            Ok(url) => match url.scheme() {
                "ldap" | "ldaps" | "ldapi" => {}
                scheme => bail!("LDAP: unsupported uri scheme '{}', expected ldap, ldaps or ldapi", scheme),
            },
            Err(err) => bail!("LDAP: invalid uri '{}': {}", self.uri, err),
        }
        if self.base_dn.trim().is_empty() {
            bail!("LDAP: base_dn must be specified");
        }
        if self.login_attribute.trim().is_empty() {
            bail!("LDAP: login_attribute must be specified");
        }
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn valid() -> Ldap {
        Ldap {
            base_dn: "ou=users,dc=example,dc=org".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn valid_config_passes() {
        assert!(valid().validate().is_ok());

        let mut ldaps = valid();
        ldaps.uri = "ldaps://example.org:636".to_string();
        assert!(ldaps.validate().is_ok());
    }

    #[test]
    fn invalid_config_fails() {
        let mut c = valid();
        c.uri = "".to_string();
        assert!(c.validate().is_err());

        let mut c = valid();
        c.uri = "http://example.org".to_string();
        assert!(c.validate().is_err());

        let mut c = valid();
        c.uri = "not a url".to_string();
        assert!(c.validate().is_err());

        let mut c = valid();
        c.base_dn = "".to_string();
        assert!(c.validate().is_err());

        let mut c = valid();
        c.login_attribute = " ".to_string();
        assert!(c.validate().is_err());
    }
}
