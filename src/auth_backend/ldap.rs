use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use ldap3::{drive, ldap_escape, LdapConnAsync, SearchEntry};

use crate::auth_backend::{AuthBackend, AuthCache, LoginType, UserInfo};
use crate::config::config::Config;
use crate::config::ldap;

const CN_ATTR: &str = "CN";

// ldap attribute names are case-insensitive (RFC 4512), but servers return
// them in their own canonical casing, e.g. openldap returns 'cn' for 'CN'
fn get_attr_ci<'a>(attrs: &'a std::collections::HashMap<String, Vec<String>>, name: &str) -> Option<&'a Vec<String>> {
    attrs.iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(name))
        .map(|(_, v)| v)
}

pub struct Ldap {
    pub(crate) config: ldap::Ldap,
}

impl Ldap {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.ldap.clone()
        }
    }
}

#[async_trait]
impl AuthBackend for Ldap {
    fn get_login_type(&self, _: &str, _: &AuthCache) -> Result<LoginType> {
        Ok(LoginType::Mask)
    }

    async fn login(&self, username: &str, password: &str) -> Result<UserInfo> {
        let (conn, mut ldap) = LdapConnAsync::new(self.config.uri.as_str()).await?;

        drive!(conn);
        ldap.simple_bind(
            self.config.bind.as_str(),
            self.config.password.as_str(),
        ).await?;

        let mut attrs = vec![CN_ATTR, self.config.login_attribute.as_str()];
        if let Some(k) = &self.config.database_attribute {
            attrs.push(k);
        }
        if let Some(k) = &self.config.keyfile_attribute {
            attrs.push(k);
        }

        // find user dn
        let (results, _res) = ldap.search(
            self.config.base_dn.as_str(),
            self.config.scope.clone().into(),
            format!(
                "(&({}={}){})",
                ldap_escape(&self.config.login_attribute),
                ldap_escape(username),
                self.config.filter
            ).as_str(),
            attrs,
        ).await?.success()?;
        ldap.unbind().await?;

        if results.is_empty() {
            bail!("no users found");
        }

        let user = SearchEntry::construct(results[0].clone());

        ldap.simple_bind(
            user.dn.as_str(),
            password,
        ).await?.success()?;
        ldap.unbind().await?;

        let cn = get_attr_ci(&user.attrs, CN_ATTR)
            .ok_or(anyhow!("CN attribute not found"))?;
        let id = get_attr_ci(&user.attrs, &self.config.login_attribute)
            .ok_or(anyhow!("login attribute '{}' not found", &self.config.login_attribute))?;

        let mut db_location = None;
        let mut keyfile_location = None;
        if let Some(key) = &self.config.database_attribute {
            if let Some(v) = get_attr_ci(&user.attrs, key) {
                db_location = Some(v[0].clone());
            }
        }
        if let Some(key) = &self.config.keyfile_attribute {
            if let Some(v) = get_attr_ci(&user.attrs, key) {
                keyfile_location = Some(v[0].clone());
            }
        }

        Ok(
            UserInfo {
                id: id[0].to_lowercase(),
                name: cn[0].clone(),
                db_location,
                keyfile_location,
                additional_data: None,
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attribute_lookup_is_case_insensitive() {
        let mut attrs = std::collections::HashMap::new();
        attrs.insert("cn".to_string(), vec!["Test User".to_string()]);
        attrs.insert("uID".to_string(), vec!["testuser".to_string()]);

        assert_eq!(get_attr_ci(&attrs, "CN").unwrap()[0], "Test User");
        assert_eq!(get_attr_ci(&attrs, "uid").unwrap()[0], "testuser");
        assert!(get_attr_ci(&attrs, "mail").is_none());
    }
}
