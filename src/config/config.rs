use std::path::PathBuf;
use std::time::Duration;

use actix_web::cookie;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_yaml::{Mapping, Value};

use crate::{auth_backend, db_backend};
use crate::config::backend::{AuthBackend, DbBackend};
use crate::config::cookie::SameSiteDef;
use crate::config::env;
use crate::config::filesystem::Filesystem;
use crate::config::htpasswd::Htpasswd;
use crate::config::http::Http;
use crate::config::key::Key;
use crate::config::ldap::Ldap;
use crate::config::oidc::Oidc;
use crate::config::search::Search;

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(alias = "server")]
    pub listen: String,
    #[serde(deserialize_with = "env::scalar")]
    pub port: u16,
    #[serde(with = "humantime_serde")]
    pub db_session_timeout: Duration,
    #[serde(with = "humantime_serde")]
    pub auth_check_interval: Duration,
    pub auth_backend: AuthBackend,
    pub db_backend: DbBackend,
    pub session_secret_key: Key,
    #[serde(with = "humantime_serde")]
    pub session_lifetime: Duration,
    #[serde(with = "SameSiteDef")]
    pub cookie_samesite: cookie::SameSite,
    // only enable behind a reverse proxy: forwarding headers are
    // client-controlled and would let rate limiting be evaded
    #[serde(deserialize_with = "env::scalar")]
    pub trust_proxy_headers: bool,
    // use the linux kernel keyring for session key storage (linux only).
    // set to false if your container runtime blocks keyctl/add_key/request_key
    // (e.g. docker desktop on macos without a custom seccomp profile).
    // ignored on non-linux platforms, which always use the in-memory store.
    #[serde(deserialize_with = "env::scalar")]
    pub use_keyring: bool,
    pub search: Search,
    #[serde(alias = "LDAP", alias = "Ldap")]
    pub ldap: Ldap,
    #[serde(alias = "OIDC", alias = "Oidc")]
    pub oidc: Oidc,
    #[serde(alias = "Htpasswd")]
    pub htpasswd: Htpasswd,
    #[serde(alias = "Filesystem")]
    pub filesystem: Filesystem,
    #[serde(alias = "HTTP", alias = "Http")]
    pub http: Http,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            listen: "127.0.0.1".to_string(),
            port: 8080,
            // 10 minutes
            db_session_timeout: Duration::from_secs(10 * 60),
            // 1 hour, 5 minutes
            auth_check_interval: Duration::from_secs(60 * 60 + 5 * 60),
            auth_backend: Default::default(),
            db_backend: Default::default(),
            session_secret_key: Key(cookie::Key::generate()),
            // 1 hour
            session_lifetime: Duration::from_secs(60 * 60),
            cookie_samesite: cookie::SameSite::Strict,
            trust_proxy_headers: false,
            use_keyring: true,
            search: Default::default(),
            ldap: Default::default(),
            oidc: Default::default(),
            htpasswd: Default::default(),
            filesystem: Default::default(),
            http: Default::default(),
        }
    }
}

impl Config {
    /// Read the configuration file, if there is one, and lay the environment
    /// over it.
    ///
    /// `path` is what was asked for on the command line. Asking for a file
    /// that is not there is an error; the default file is optional, so a
    /// deployment can hand everything in through the environment and ship no
    /// file at all.
    pub fn load(path: Option<PathBuf>, default_path: &str) -> Result<Self> {
        let text = match &path {
            Some(p) => Some(
                std::fs::read_to_string(p)
                    .with_context(|| format!("failed to read config file {}", p.display()))?,
            ),
            None => match std::fs::read_to_string(default_path) {
                Ok(t) => Some(t),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
                Err(err) => {
                    return Err(err).with_context(|| format!("failed to read config file {}", default_path));
                }
            },
        };

        Self::from_parts(text.as_deref(), std::env::vars())
    }

    fn from_parts<I>(text: Option<&str>, vars: I) -> Result<Self>
    where
        I: IntoIterator<Item = (String, String)>,
    {
        let mut root: Value = match text {
            Some(t) => serde_yaml::from_str(t).context("failed to parse config file")?,
            None => Value::Mapping(Mapping::new()),
        };

        // an empty file parses as null, which is simply nothing to start from
        if root.is_null() {
            root = Value::Mapping(Mapping::new());
        }

        // anything else that is not a mapping is not a configuration, and must
        // be reported rather than quietly replaced by the environment below
        if !root.is_mapping() {
            bail!("config file must be a mapping of settings");
        }

        env::apply(&mut root, vars)?;

        let conf: Config = serde_yaml::from_value(root).context("failed to parse config")?;

        auth_backend::new(&conf).validate_config()?;
        db_backend::new(&conf).validate_config()?;

        Ok(conf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::backend::AuthBackend;

    // Config deliberately has no Debug (it holds the session key), so an
    // expected failure is unwrapped by hand rather than with unwrap_err
    fn error(yaml: Option<&str>, vars: &[(&str, &str)]) -> String {
        match load(yaml, vars) {
            Ok(_) => panic!("expected the configuration to be rejected"),
            Err(err) => format!("{:#}", err),
        }
    }

    // the filesystem backend refuses to start without somewhere to read the
    // database from, so every configuration below needs one; supplying it here
    // keeps each test about the thing it is testing
    const DB_LOCATION: &str = "KEEPASS4WEB_FILESYSTEM__DB_LOCATION";

    fn load(yaml: Option<&str>, vars: &[(&str, &str)]) -> Result<Config> {
        let mut vars: Vec<(String, String)> = vars
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();

        let has_location = vars.iter().any(|(k, _)| k == DB_LOCATION)
            || yaml.is_some_and(|y| y.contains("db_location"));
        if !has_location {
            vars.push((DB_LOCATION.to_string(), "./db.kdbx".to_string()));
        }

        Config::from_parts(yaml, vars)
    }

    #[test]
    fn the_environment_wins_over_the_file() {
        let conf = load(
            Some("listen: '127.0.0.1'\nport: 8080\n"),
            &[("KEEPASS4WEB_PORT", "9000")],
        ).unwrap();

        assert_eq!(conf.port, 9000);
        assert_eq!(conf.listen, "127.0.0.1");
    }

    // the reason the feature exists: a deployment that ships no file at all
    #[test]
    fn there_need_not_be_a_file() {
        let conf = load(None, &[("KEEPASS4WEB_LISTEN", "::"), ("KEEPASS4WEB_PORT", "9000")]).unwrap();

        assert_eq!(conf.listen, "::");
        assert_eq!(conf.port, 9000);
        // everything else falls back to the defaults
        assert_eq!(conf.db_session_timeout, Duration::from_secs(10 * 60));
    }

    #[test]
    fn an_empty_file_is_not_an_error() {
        let conf = load(Some(""), &[]).unwrap();

        assert_eq!(conf.port, 8080);
    }

    #[test]
    fn a_setting_that_is_not_a_string_takes_the_text_spelling() {
        let conf = load(
            None,
            &[
                ("KEEPASS4WEB_PORT", "9000"),
                ("KEEPASS4WEB_TRUST_PROXY_HEADERS", "true"),
                ("KEEPASS4WEB_USE_KEYRING", "false"),
                ("KEEPASS4WEB_SEARCH__EXTRA_FIELDS", "false"),
            ],
        ).unwrap();

        assert_eq!(conf.port, 9000);
        assert!(conf.trust_proxy_headers);
        assert!(!conf.use_keyring);
        assert!(!conf.search.extra_fields);
    }

    #[test]
    fn a_duration_is_written_the_way_the_file_writes_it() {
        let conf = load(None, &[("KEEPASS4WEB_DB_SESSION_TIMEOUT", "45 minutes")]).unwrap();

        assert_eq!(conf.db_session_timeout, Duration::from_secs(45 * 60));
    }

    #[test]
    fn the_backend_and_its_section_can_both_come_from_the_environment() {
        let conf = load(
            None,
            &[
                ("KEEPASS4WEB_AUTH_BACKEND", "LDAP"),
                ("KEEPASS4WEB_LDAP__URI", "ldap://directory:389"),
                ("KEEPASS4WEB_LDAP__BASE_DN", "ou=users,dc=example,dc=org"),
                ("KEEPASS4WEB_LDAP__LOGIN_ATTRIBUTE", "uid"),
            ],
        ).unwrap();

        assert!(matches!(conf.auth_backend, AuthBackend::Ldap));
        assert_eq!(conf.ldap.uri, "ldap://directory:389");
        assert_eq!(conf.ldap.base_dn, "ou=users,dc=example,dc=org");
    }

    // a secret is the thing most likely to be handed over this way, and the
    // most damaging to reinterpret
    #[test]
    fn a_numeric_secret_stays_a_string() {
        let conf = load(
            Some("auth_backend: 'LDAP'\nLDAP:\n  uri: 'ldap://x:389'\n  base_dn: 'dc=example'\n  login_attribute: 'uid'\n"),
            &[("KEEPASS4WEB_LDAP__PASSWORD", "12345")],
        ).unwrap();

        assert_eq!(conf.ldap.password, "12345");
    }

    #[test]
    fn a_list_setting_can_be_replaced() {
        let conf = load(None, &[("KEEPASS4WEB_SEARCH__FIELDS", "[title, url]")]).unwrap();

        assert_eq!(conf.search.fields.len(), 2);
    }

    #[test]
    fn a_list_can_also_be_written_without_the_brackets() {
        let conf = load(
            None,
            &[
                ("KEEPASS4WEB_SEARCH__FIELDS", "title, url"),
                ("KEEPASS4WEB_OIDC__SCOPES", "profile"),
            ],
        ).unwrap();

        assert_eq!(conf.search.fields.len(), 2);
        assert_eq!(conf.oidc.scopes, vec!["profile".to_string()]);
    }

    // reported on the pull request: a secret is free to start with a bracket,
    // and reading it as a list loses it
    #[test]
    fn a_secret_that_looks_like_a_list_is_still_a_secret() {
        let conf = load(
            None,
            &[
                ("KEEPASS4WEB_AUTH_BACKEND", "OIDC"),
                ("KEEPASS4WEB_OIDC__ISSUER", "https://issuer.example.org/"),
                ("KEEPASS4WEB_OIDC__CLIENT_ID", "keepass4web"),
                ("KEEPASS4WEB_OIDC__CLIENT_SECRET", "[secret]"),
            ],
        ).unwrap();

        assert_eq!(conf.oidc.client_secret, "[secret]");
    }

    #[test]
    fn a_secret_that_looks_like_a_mapping_is_still_a_secret() {
        let conf = load(
            Some("auth_backend: 'LDAP'\nLDAP:\n  uri: 'ldap://x:389'\n  base_dn: 'dc=example'\n  login_attribute: 'uid'\n"),
            &[("KEEPASS4WEB_LDAP__PASSWORD", "{brace}")],
        ).unwrap();

        assert_eq!(conf.ldap.password, "{brace}");
    }

    #[test]
    fn the_config_is_still_validated_after_the_environment_is_applied() {
        // OIDC without an issuer is rejected, wherever the setting came from
        let err = error(None, &[("KEEPASS4WEB_AUTH_BACKEND", "OIDC")]);

        assert!(err.contains("issuer"), "unexpected error: {}", err);
    }

    // reported on the pull request: with the environment in play, a file that
    // parses but is not a set of settings used to be replaced instead of refused
    #[test]
    fn a_file_that_is_not_a_mapping_is_refused_even_with_the_environment_set() {
        let err = error(Some("[]\n"), &[("KEEPASS4WEB_PORT", "9000")]);

        assert!(err.contains("mapping of settings"), "unexpected error: {}", err);
    }

    #[test]
    fn a_file_that_is_not_yaml_is_reported_as_such() {
        let err = error(Some("listen: '::'\n  port: nope\n"), &[]);

        assert!(err.contains("config file"), "unexpected error: {}", err);
    }
}
