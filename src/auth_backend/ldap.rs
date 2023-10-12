use anyhow::{anyhow, bail, Result};
use ldap3::{ldap_escape, LdapConn, SearchEntry};

use crate::auth_backend::{AuthBackend, UserInfo};
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

impl AuthBackend for Ldap {
    fn login(&self, username: &str, password: &str) -> Result<UserInfo> {
        let mut ldap = LdapConn::new(self.config.uri.as_str())?;

        ldap.simple_bind(
            self.config.bind.as_str(),
            self.config.password.as_str(),
        )?;

        // find user dn
        let (results, _res) = ldap.search(
            self.config.base_dn.as_str(),
            self.config.scope.clone().into(),
            format!(
                "(&({}={}){})",
                ldap_escape(self.config.login_attribute.clone()),
                ldap_escape(username),
                self.config.filter
            ).as_str(),
            vec![
                CN_ATTR,
                &self.config.keyfile_attribute,
                &self.config.database_attribute,
            ],
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

        let cn = user.attrs.get(CN_ATTR).ok_or(anyhow!("CN attribute not found"))?;
        Ok(UserInfo {
            name: cn[0].clone(),
        })
    }
}