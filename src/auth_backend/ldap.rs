use anyhow::{anyhow, bail, Result};
use async_trait::async_trait;
use ldap3::{ldap_escape, LdapConn, SearchEntry};

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
}

#[async_trait]
impl AuthBackend for Ldap {
    fn get_login_type(&self, _: &str, _: &AuthCache) -> Result<LoginType> {
        Ok(LoginType::Mask)
    }

    fn login(&self, username: &str, password: &str) -> Result<UserInfo> {
        let mut ldap = LdapConn::new(self.config.uri.as_str())?;

        ldap.simple_bind(
            self.config.bind.as_str(),
            self.config.password.as_str(),
        )?;

        let mut attrs = vec![CN_ATTR];
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
        )?.success()?;
        ldap.unbind()?;

        if results.is_empty() {
            bail!("no users found");
        }

        let user = SearchEntry::construct(results[0].clone());

        ldap.simple_bind(
            user.dn.as_str(),
            password,
        )?.success()?;
        ldap.unbind()?;

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