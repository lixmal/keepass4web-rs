use std::fmt::Display;
use std::str::FromStr;

use anyhow::{bail, Result};
use serde::{Deserialize, Deserializer};
use serde::de::DeserializeOwned;
use serde::de;
use serde_yaml::{Mapping, Value};

/// Prefix an environment variable needs to be considered configuration.
pub(crate) const PREFIX: &str = "KEEPASS4WEB_";

/// Separates the levels of a nested setting. A single underscore is part of a
/// name (`db_session_timeout`), two of them step into a section, so
/// `KEEPASS4WEB_LDAP__BASE_DN` is `base_dn` inside `ldap`.
pub(crate) const SEPARATOR: &str = "__";

/// Lay environment variables over a configuration tree, so a deployment can
/// keep its settings out of the file the image ships. Values already in the
/// tree are replaced, sections that do not exist yet are created.
pub(crate) fn apply<I>(root: &mut Value, vars: I) -> Result<()>
where
    I: IntoIterator<Item = (String, String)>,
{
    let mut names: Vec<(String, String)> = vars
        .into_iter()
        .filter(|(name, _)| name.starts_with(PREFIX))
        .collect();

    // deterministic order, so a run is reproducible when two variables
    // disagree about the same setting
    names.sort_by(|(a, _), (b, _)| a.cmp(b));

    for (name, raw) in names {
        let path = path_of(&name)?;
        insert(root, &path, parse(&raw))?;
    }

    Ok(())
}

/// `KEEPASS4WEB_LDAP__BASE_DN` becomes `["ldap", "base_dn"]`.
fn path_of(name: &str) -> Result<Vec<String>> {
    let stripped = name.strip_prefix(PREFIX).unwrap_or(name);

    let path: Vec<String> = stripped
        .split(SEPARATOR)
        .map(|segment| segment.to_lowercase())
        .collect();

    if path.iter().any(|segment| segment.is_empty()) {
        bail!("{}: empty name between separators", name);
    }

    Ok(path)
}

/// An environment variable is text, and is kept as text.
///
/// Nothing is guessed here: a secret of `12345`, `yes` or `[redacted]` is the
/// string it was typed as, because guessing would have to be wrong for one of
/// them. The settings that are not strings read that text themselves, see
/// [`scalar`] and [`list`].
fn parse(raw: &str) -> Value {
    Value::String(raw.to_string())
}

/// The spelling a mapping already uses for a name, ignoring case.
///
/// Sections are accepted under more than one spelling — the shipped file
/// writes `LDAP:` and `OIDC:` — and they are all the same field. Writing a
/// second, lowercase key beside one of them would be read as the field being
/// given twice, so an existing spelling is reused rather than added to.
fn existing_key(map: &Mapping, name: &str) -> Option<Value> {
    map.keys().find(|key| {
        key.as_str().is_some_and(|k| k.eq_ignore_ascii_case(name))
    }).cloned()
}

fn insert(root: &mut Value, path: &[String], value: Value) -> Result<()> {
    let (last, sections) = match path.split_last() {
        Some(v) => v,
        None => bail!("no setting named"),
    };

    let mut node = root;

    for section in sections {
        // a section that is currently a plain value has to give way, or the
        // setting underneath it could not be expressed at all
        if !node.is_mapping() {
            *node = Value::Mapping(Mapping::new());
        }

        let map = node.as_mapping_mut().expect("just made sure it is a mapping");
        let key = existing_key(map, section).unwrap_or_else(|| Value::String(section.clone()));

        node = map.entry(key).or_insert_with(|| Value::Mapping(Mapping::new()));
    }

    if !node.is_mapping() {
        *node = Value::Mapping(Mapping::new());
    }

    let map = node.as_mapping_mut().expect("just made sure it is a mapping");
    let key = existing_key(map, last).unwrap_or_else(|| Value::String(last.clone()));
    map.insert(key, value);

    Ok(())
}

/// Accepts a list either as a list or as the text spelling of one.
///
/// An environment variable has no way to hold a sequence, so it carries either
/// the YAML form, `[title, url]`, or just the items separated by commas.
pub(crate) fn list<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    #[derive(Deserialize)]
    #[serde(untagged, bound = "T: DeserializeOwned")]
    enum Either<T> {
        Native(Vec<T>),
        Text(String),
    }

    let text = match Either::<T>::deserialize(deserializer)? {
        Either::Native(v) => return Ok(v),
        Either::Text(s) => s,
    };

    let trimmed = text.trim();

    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    if trimmed.starts_with('[') {
        return serde_yaml::from_str(trimmed).map_err(de::Error::custom);
    }

    // the items on their own, so a single-item list needs no punctuation
    let items = Value::Sequence(
        trimmed
            .split(',')
            .map(|item| Value::String(item.trim().to_string()))
            .collect(),
    );

    serde_yaml::from_value(items).map_err(de::Error::custom)
}

/// Accepts a value either as its own type or as the text spelling of it.
///
/// Environment variables can only carry text, so `KEEPASS4WEB_PORT=8080` would
/// otherwise be rejected for not being a YAML integer. Applied to the settings
/// that are not strings; string settings need nothing, and keeping them plain
/// is what stops a numeric password from being read as a number.
pub(crate) fn scalar<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de> + FromStr,
    <T as FromStr>::Err: Display,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Either<T> {
        Native(T),
        Text(String),
    }

    match Either::<T>::deserialize(deserializer)? {
        Either::Native(v) => Ok(v),
        Either::Text(s) => s.trim().parse().map_err(de::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlay(yaml: &str, vars: &[(&str, &str)]) -> Value {
        let mut root: Value = serde_yaml::from_str(yaml).unwrap();
        let vars = vars.iter().map(|(k, v)| (k.to_string(), v.to_string()));
        apply(&mut root, vars).unwrap();
        root
    }

    fn at<'a>(root: &'a Value, path: &[&str]) -> &'a Value {
        let mut node = root;
        for key in path {
            node = node.get(*key).unwrap_or_else(|| panic!("no {} in {:?}", key, root));
        }
        node
    }

    #[test]
    fn a_variable_replaces_the_setting_of_the_same_name() {
        let root = overlay("listen: '127.0.0.1'\n", &[("KEEPASS4WEB_LISTEN", "::")]);

        assert_eq!(at(&root, &["listen"]), "::");
    }

    #[test]
    fn a_variable_adds_a_setting_the_file_never_mentioned() {
        let root = overlay("listen: '::'\n", &[("KEEPASS4WEB_PORT", "9000")]);

        assert_eq!(at(&root, &["port"]), "9000");
        assert_eq!(at(&root, &["listen"]), "::");
    }

    #[test]
    fn two_underscores_step_into_a_section_and_one_does_not() {
        let root = overlay(
            "ldap:\n  uri: 'ldap://old:389'\n",
            &[
                ("KEEPASS4WEB_LDAP__BASE_DN", "ou=users,dc=example,dc=org"),
                ("KEEPASS4WEB_DB_SESSION_TIMEOUT", "30 minutes"),
            ],
        );

        assert_eq!(at(&root, &["ldap", "base_dn"]), "ou=users,dc=example,dc=org");
        // the rest of the section survives
        assert_eq!(at(&root, &["ldap", "uri"]), "ldap://old:389");
        // a single underscore is part of the name, not a step into a section
        assert_eq!(at(&root, &["db_session_timeout"]), "30 minutes");
    }

    #[test]
    fn a_section_is_created_on_the_way_down() {
        let root = overlay(
            "listen: '::'\n",
            &[("KEEPASS4WEB_HTTP__CREDENTIALS__USERNAME", "someone")],
        );

        assert_eq!(at(&root, &["http", "credentials", "username"]), "someone");
    }

    // the whole point of the feature is keeping secrets out of the file, so a
    // secret that happens to look like something else must not be reinterpreted
    #[test]
    fn a_secret_is_kept_as_written() {
        let root = overlay(
            "ldap: {}\n",
            &[
                ("KEEPASS4WEB_LDAP__PASSWORD", "12345"),
                ("KEEPASS4WEB_OIDC__CLIENT_SECRET", "yes"),
                ("KEEPASS4WEB_HTPASSWD__PATH", "true"),
            ],
        );

        assert_eq!(at(&root, &["ldap", "password"]), &Value::String("12345".into()));
        assert_eq!(at(&root, &["oidc", "client_secret"]), &Value::String("yes".into()));
        assert_eq!(at(&root, &["htpasswd", "path"]), &Value::String("true".into()));
    }

    // a secret is free to start with a bracket or a brace, and reading it as a
    // list or a mapping would lose it
    #[test]
    fn a_secret_that_looks_like_a_collection_is_still_a_secret() {
        let root = overlay(
            "oidc: {}\n",
            &[
                ("KEEPASS4WEB_OIDC__CLIENT_SECRET", "[secret]"),
                ("KEEPASS4WEB_LDAP__PASSWORD", "{brace}"),
            ],
        );

        assert_eq!(at(&root, &["oidc", "client_secret"]), &Value::String("[secret]".into()));
        assert_eq!(at(&root, &["ldap", "password"]), &Value::String("{brace}".into()));
    }

    #[test]
    fn an_empty_variable_is_an_empty_string_not_a_missing_setting() {
        // an empty ldap password means an anonymous bind, so it has to survive
        let root = overlay("ldap: {}\n", &[("KEEPASS4WEB_LDAP__PASSWORD", "")]);

        assert_eq!(at(&root, &["ldap", "password"]), &Value::String(String::new()));
    }

    // nothing is interpreted on the way in, whatever it looks like; the
    // settings that are not strings do the reading themselves
    #[test]
    fn every_value_arrives_as_text() {
        let root = overlay(
            "search: {}\n",
            &[
                ("KEEPASS4WEB_SEARCH__FIELDS", "[title, url]"),
                ("KEEPASS4WEB_PORT", "8080"),
                ("KEEPASS4WEB_USE_KEYRING", "false"),
            ],
        );

        assert_eq!(at(&root, &["search", "fields"]), &Value::String("[title, url]".into()));
        assert_eq!(at(&root, &["port"]), &Value::String("8080".into()));
        assert_eq!(at(&root, &["use_keyring"]), &Value::String("false".into()));
    }

    #[test]
    fn variables_without_the_prefix_are_left_alone() {
        let root = overlay(
            "listen: '::'\n",
            &[("PATH", "/usr/bin"), ("HOME", "/root"), ("LISTEN", "0.0.0.0")],
        );

        assert_eq!(at(&root, &["listen"]), "::");
        assert!(root.get("path").is_none());
    }

    #[test]
    fn a_name_with_nothing_between_the_separators_is_refused() {
        let mut root: Value = serde_yaml::from_str("{}").unwrap();
        let vars = [("KEEPASS4WEB_LDAP____URI".to_string(), "x".to_string())];

        assert!(apply(&mut root, vars).is_err());
    }

    // the shipped file writes 'LDAP:', and serde treats that as the same field
    // as 'ldap'; adding a second spelling would be read as a duplicate
    #[test]
    fn an_existing_spelling_of_a_section_is_reused() {
        let root = overlay(
            "LDAP:\n  uri: 'ldap://old:389'\n",
            &[("KEEPASS4WEB_LDAP__PASSWORD", "secret")],
        );

        assert!(root.get("ldap").is_none(), "a second spelling was added: {:?}", root);
        assert_eq!(at(&root, &["LDAP", "password"]), "secret");
        assert_eq!(at(&root, &["LDAP", "uri"]), "ldap://old:389");
    }

    #[test]
    fn a_setting_below_a_plain_value_replaces_it() {
        // 'ldap' is a string here, and something has to give for ldap.uri to exist
        let root = overlay("ldap: 'nonsense'\n", &[("KEEPASS4WEB_LDAP__URI", "ldap://x:389")]);

        assert_eq!(at(&root, &["ldap", "uri"]), "ldap://x:389");
    }
}
