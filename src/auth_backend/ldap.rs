use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use ldap3::{drive, ldap_escape, LdapConnAsync, SearchEntry};

use crate::auth_backend::{AuthBackend, AuthCache, LoginType, UserInfo};
use crate::config::config::Config;
use crate::config::ldap;

const CN_ATTR: &str = "CN";

pub struct Ldap {
    pub(crate) config: ldap::Ldap,
}

impl Ldap {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.ldap.clone()
        }
    }

    // resulting filter: (&(<login_attribute>=<username>)<configured filter>),
    // or just (<login_attribute>=<username>) if no filter is configured
    fn search_filter(&self, username: &str) -> String {
        let user_match = format!(
            "({}={})",
            self.config.login_attribute,
            ldap_escape(username),
        );
        match self.config.filter.trim() {
            "" | "()" => user_match,
            filter => format!("(&{}{})", user_match, filter),
        }
    }
}

#[async_trait]
impl AuthBackend for Ldap {
    fn validate_config(&self) -> Result<()> {
        self.config.validate()
    }

    fn get_login_type(&self, _: &str, _: &AuthCache) -> Result<LoginType> {
        Ok(LoginType::Mask)
    }

    async fn login(&self, username: &str, password: &str) -> Result<UserInfo> {
        // an empty password would result in an anonymous bind, which most ldap
        // servers report as success, bypassing the password check entirely
        if password.is_empty() {
            bail!("empty password");
        }

        let (conn, mut ldap) = LdapConnAsync::new(self.config.uri.as_str()).await?;

        drive!(conn);
        ldap.simple_bind(
            self.config.bind.as_str(),
            self.config.password.as_str(),
        ).await?.success()?;

        let mut attrs = vec![CN_ATTR];
        if let Some(k) = &self.config.database_attribute {
            attrs.push(k);
        }
        if let Some(k) = &self.config.keyfile_attribute {
            attrs.push(k);
        }

        // find user dn
        let filter = self.search_filter(username);
        let (results, _res) = ldap.search(
            self.config.base_dn.as_str(),
            self.config.scope.clone().into(),
            filter.as_str(),
            attrs,
        ).await?.success()?;

        if results.is_empty() {
            bail!("no users found with filter '{}'", filter);
        }

        let user = SearchEntry::construct(results[0].clone());

        // verify credentials by rebinding as the found user,
        // before unbind: unbind terminates the connection
        ldap.simple_bind(
            user.dn.as_str(),
            password,
        ).await?.success()?;
        ldap.unbind().await?;

        let cn = user.attrs.get(CN_ATTR)
            .ok_or(anyhow!("CN attribute not found"))?;
        let id = user.attrs.get(&self.config.login_attribute)
            .ok_or(anyhow!("login attribute '{}' not found", &self.config.login_attribute))?;

        let mut db_location = None;
        let mut keyfile_location = None;
        if let Some(key) = &self.config.database_attribute {
            if let Some(v) = user.attrs.get(key) {
                db_location = Some(v[0].clone());
            }
        }
        if let Some(key) = &self.config.keyfile_attribute {
            if let Some(v) = user.attrs.get(key) {
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

    fn backend(filter: &str) -> Ldap {
        Ldap {
            config: ldap::Ldap {
                filter: filter.to_string(),
                ..Default::default()
            }
        }
    }

    #[test]
    fn filter_is_combined_with_login_attribute() {
        assert_eq!(
            backend("(objectClass=inetOrgPerson)").search_filter("alice"),
            "(&(uid=alice)(objectClass=inetOrgPerson))",
        );
    }

    #[test]
    fn empty_filter_matches_login_attribute_only() {
        assert_eq!(backend("").search_filter("alice"), "(uid=alice)");
        assert_eq!(backend("  ").search_filter("alice"), "(uid=alice)");
        assert_eq!(backend("()").search_filter("alice"), "(uid=alice)");
    }

    #[test]
    fn username_is_escaped() {
        assert_eq!(
            backend("").search_filter("a*)(uid=*"),
            "(uid=a\\2a\\29\\28uid=\\2a)",
        );
    }
}
