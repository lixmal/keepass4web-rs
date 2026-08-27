use actix_web::web::{Path, Query};
use anyhow::{anyhow, bail};
use anyhow::Result;
use base64;
use base64::Engine;
use base64::engine::general_purpose;
use keepass::{Database, DatabaseKey};
use keepass::config::DatabaseConfig;
use keepass::db::{Entry as KpEntry, Group as KpGroup, Icon, Node, Value};
use regex::Regex;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::auth::DbLogin;
use crate::auth_backend::UserInfo;
use crate::config::config::Config;
use crate::config::search::Search;
use crate::db_backend::DbBackend;
use crate::keepass::encrypted::Encrypted;
use crate::keepass::entry::{
    Entry,
    EntryGroup,
    Group,
};
use crate::keepass::key::SecretKey;

// distinguishes missing entries/groups/icons from real server faults,
// so handlers can return 404 instead of 500
#[derive(Debug, Clone)]
pub struct NotFoundError(pub &'static str);

impl std::fmt::Display for NotFoundError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} not found", self.0)
    }
}

impl std::error::Error for NotFoundError {}

// the fields the format defines: everything else on an entry is a custom field
// the user added and is theirs to name
const STANDARD_FIELDS: [&str; 5] = ["Title", "UserName", "Password", "URL", "Notes"];

// a field the user added to an entry, protected ones hidden from the client
// until they ask for them
#[derive(Deserialize)]
pub struct CustomField {
    pub name: String,
    #[serde(default)]
    pub value: String,
    #[serde(default)]
    pub protected: bool,
}

#[derive(Deserialize)]
pub struct Id {
    pub id: Uuid,
}

#[derive(Deserialize)]
pub struct Protected {
    pub entry_id: Uuid,
    pub name: String,
}

#[derive(Deserialize)]
pub struct File {
    pub entry_id: Uuid,
    pub filename: String,
}

#[derive(Deserialize)]
pub struct SearchTerm {
    pub term: String,
}

pub struct KeePass {
    config: Config,
    db: Database,
}


impl KeePass {
    pub fn from_enc(config: &Config, key: SecretKey, enc: Encrypted) -> Result<Self> {
        // TODO: add some aad from the keepass db
        let ser_db = enc.decrypt(key, &[])?;

        let db: Database = postcard::from_bytes(ser_db.expose_secret())?;
        Ok(
            Self {
                config: config.clone(),
                db,
            }
        )
    }

    pub fn to_enc(self) -> Result<(SecretKey, Encrypted)> {
        // TODO: avoid vector realloc to make zeroize effective
        let ser_db = postcard::to_stdvec(&self.db)?;
        drop(self.db);

        // TODO: add some aad from the keepass db
        Encrypted::encrypt(ser_db, &[], self.config.db_session_timeout)
    }

    pub async fn from_backend(config: &Config, db_backend: &mut dyn DbBackend, params: &DbLogin, user_info: &UserInfo) -> Result<Self> {
        let db_key = Self::db_key_from_params(db_backend, params, user_info).await?;

        // Read the database into an owned buffer. The reader (which borrows db_backend)
        // is dropped when the async block completes, releasing the immutable borrow before
        // we potentially need &mut db_backend to create a new database.
        let load_result: Result<Vec<u8>> = async {
            let mut reader = db_backend.get_db_read(user_info).await?;
            let mut buf = vec![];
            reader.read_to_end(&mut buf).await?;
            Ok(buf)
        }.await;

        let buf = match load_result {
            Ok(buf) => buf,
            Err(err) => {
                // No database at the configured path yet — create a new empty one.
                if err.downcast_ref::<std::io::Error>()
                    .map(|e| e.kind() == std::io::ErrorKind::NotFound)
                    .unwrap_or(false)
                {
                    return Self::create_new(config, db_backend, db_key, user_info).await;
                }
                return Err(err);
            }
        };

        let db = tokio::task::spawn_blocking(move || {
            let mut buf = buf;
            let db = Database::open(&mut buf.as_slice(), db_key);
            buf.zeroize();
            db
        }).await??;

        Ok(KeePass { config: config.clone(), db })
    }

    async fn create_new(config: &Config, db_backend: &mut dyn DbBackend, db_key: DatabaseKey, user_info: &UserInfo) -> Result<Self> {
        let db = Database::new(DatabaseConfig::default());

        let mut buf: Vec<u8> = vec![];
        let (result, mut buf, db) = tokio::task::spawn_blocking(move || {
            let r = db.save(&mut buf, db_key);
            (r, buf, db)
        }).await?;
        result.map_err(|e| anyhow!("failed to initialise new database: {}", e))?;

        let (mut writer, rx) = db_backend.get_db_write(user_info).await?;
        writer.write_all(&buf).await?;
        buf.zeroize();
        writer.shutdown().await?;
        if let Some(rx) = rx {
            rx.await??;
        }

        Ok(KeePass { config: config.clone(), db })
    }

    pub(crate) async fn db_key_from_params_pub(db_backend: &dyn DbBackend, params: &DbLogin, user_info: &UserInfo) -> Result<DatabaseKey> {
        Self::db_key_from_params(db_backend, params, user_info).await
    }

    async fn db_key_from_params(db_backend: &dyn DbBackend, params: &DbLogin, user_info: &UserInfo) -> Result<DatabaseKey> {
        let mut db_key = DatabaseKey::new();
        let mut temp1;
        let mut temp2;
        let keyfile;
        if let Some(keyfile_b64) = &params.key {
            // TODO: use constant time decode against timing attacks
            keyfile = general_purpose::STANDARD.decode(keyfile_b64)?;

            temp1 = keyfile.as_slice();
            db_key = db_key.with_keyfile(&mut temp1)?;
        } else if let Some(keyfile) = db_backend.get_key_read(user_info).await {
            temp2 = keyfile?;
            // TODO: fix this
            let mut buf = vec![];
            temp2.read_to_end(&mut buf).await?;
            db_key = db_key.with_keyfile(&mut buf.as_slice())?;
            buf.zeroize();
        }

        if let Some(pw) = &params.password {
            db_key = db_key.with_password(pw);
        }
        Ok(db_key)
    }


    pub fn create_entry(
        &mut self,
        group_id: &Uuid,
        title: &str,
        username: &str,
        password: &str,
        url: &str,
        notes: &str,
        icon: Option<usize>,
        tags: &[String],
        custom_fields: &[CustomField],
    ) -> Result<Uuid> {
        let group = Self::find_group_by_id_mut(&mut self.db.root, group_id)
            .ok_or(NotFoundError("group"))?;

        let mut entry = KpEntry::new();
        entry.fields.insert("Title".to_string(), Value::Unprotected(title.to_string()));
        entry.fields.insert("UserName".to_string(), Value::Unprotected(username.to_string()));
        if !password.is_empty() {
            entry.fields.insert("Password".to_string(), Value::Protected(password.as_bytes().into()));
        }
        entry.fields.insert("URL".to_string(), Value::Unprotected(url.to_string()));
        entry.fields.insert("Notes".to_string(), Value::Unprotected(notes.to_string()));
        entry.icon_id = icon;
        entry.tags = tags.to_vec();
        Self::apply_custom_fields(&mut entry, custom_fields);

        let uuid = entry.uuid;
        group.add_child(entry);
        Ok(uuid)
    }

    pub fn update_entry(
        &mut self,
        entry_id: &Uuid,
        title: &str,
        username: &str,
        password: &str,
        url: &str,
        notes: &str,
        icon: Option<usize>,
        tags: &[String],
        custom_fields: &[CustomField],
    ) -> Result<()> {
        let entry = Self::find_entry_by_id_mut(&mut self.db.root, entry_id)
            .ok_or(NotFoundError("entry"))?;

        entry.fields.insert("Title".to_string(), Value::Unprotected(title.to_string()));
        entry.fields.insert("UserName".to_string(), Value::Unprotected(username.to_string()));
        if !password.is_empty() {
            entry.fields.insert("Password".to_string(), Value::Protected(password.as_bytes().into()));
        }
        entry.fields.insert("URL".to_string(), Value::Unprotected(url.to_string()));
        entry.fields.insert("Notes".to_string(), Value::Unprotected(notes.to_string()));
        if let Some(id) = icon {
            entry.icon_id = Some(id);
        }

        entry.tags = tags.to_vec();
        Self::apply_custom_fields(entry, custom_fields);

        Ok(())
    }

    // The standard fields of an entry are named by the format, everything else
    // the user added is a custom field and is replaced wholesale by what the
    // request carries. A protected field sent without a value keeps the value
    // it has, the same way an empty password leaves the password alone: the
    // client never received it to send back.
    fn apply_custom_fields(entry: &mut keepass::db::Entry, custom_fields: &[CustomField]) {
        let kept: Vec<(String, Value)> = custom_fields.iter()
            .map(|field| {
                let value = match (field.protected, field.value.is_empty()) {
                    (true, true) => match entry.fields.get(&field.name) {
                        Some(Value::Protected(existing)) => Value::Protected(existing.unsecure().into()),
                        _ => Value::Protected(field.value.as_bytes().into()),
                    },
                    (true, false) => Value::Protected(field.value.as_bytes().into()),
                    (false, _) => Value::Unprotected(field.value.clone()),
                };

                (field.name.clone(), value)
            })
            .collect();

        entry.fields.retain(|name, _| STANDARD_FIELDS.contains(&name.as_str()));
        for (name, value) in kept {
            entry.fields.insert(name, value);
        }
    }

    pub fn create_group(&mut self, parent_id: &Uuid, name: &str) -> Result<Uuid> {
        // Root accepts unlimited groups. Non-root groups are capped at 2 subgroups,
        // which also blocks nesting beyond one level below root.
        if *parent_id != self.db.root.uuid {
            let parent = Self::find_group_by_id(&self.db.root, parent_id)
                .ok_or(NotFoundError("group"))?;
            let subgroup_count = parent.children.iter()
                .filter(|n| matches!(n, Node::Group(_)))
                .count();
            if subgroup_count >= 2 {
                return Err(anyhow!("a group may have at most 2 subgroups"));
            }
        }

        let parent = Self::find_group_by_id_mut(&mut self.db.root, parent_id)
            .ok_or(NotFoundError("group"))?;
        let group = KpGroup::new(name);
        let id = group.uuid;
        parent.children.push(Node::Group(group));
        Ok(id)
    }

    pub fn rename_group(&mut self, group_id: &Uuid, name: &str) -> Result<()> {
        let group = Self::find_group_by_id_mut(&mut self.db.root, group_id)
            .ok_or(NotFoundError("group"))?;
        group.name = name.to_string();
        Ok(())
    }

    pub fn delete_entry(&mut self, entry_id: &Uuid) -> Result<()> {
        if !Self::remove_entry_from_group(&mut self.db.root, entry_id) {
            return Err(NotFoundError("entry").into());
        }
        Ok(())
    }

    // The parser drops the reference an entry holds to a file attached to it,
    // and writing the database back therefore leaves every attachment in it
    // orphaned: the bytes stay in the file, nothing points at them any more and
    // no client shows them again. Refusing to write is the only way to keep
    // them until the underlying library carries the reference through.
    pub fn attachment_count(&self) -> usize {
        self.db.header_attachments.len()
    }

    pub async fn to_backend_with_key(self, db_backend: &mut dyn DbBackend, db_key: DatabaseKey, user_info: &UserInfo) -> Result<()> {
        if self.attachment_count() > 0 {
            bail!(
                "the database has {} file attachment(s), which saving would leave orphaned",
                self.attachment_count(),
            );
        }

        let mut buf: Vec<u8> = vec![];
        let (result, mut buf) = tokio::task::spawn_blocking(move || {
            let r = self.db.save(&mut buf, db_key);
            (r, buf)
        }).await?;
        result.map_err(|e| anyhow!("failed to save database: {}", e))?;

        let (mut writer, rx) = db_backend.get_db_write(user_info).await?;
        writer.write_all(&buf).await?;
        buf.zeroize();
        writer.shutdown().await?;
        if let Some(rx) = rx {
            rx.await??;
        }

        Ok(())
    }

    pub fn get_groups(&self) -> Result<(Group, Option<Uuid>)> {
        let mut last_selected = self.db.meta.last_selected_group;

        if let Some(v) = last_selected {
            if Self::find_group_by_id(&self.db.root, &v).is_none() {
                last_selected = None;
            }
        }

        Ok(
            (
                Self::find_all_groups(&self.db.root),
                last_selected,
            )
        )
    }

    pub fn get_group_entries(&self, params: &Query<Id>) -> Result<EntryGroup> {
        let group = Self::find_group_by_id(&self.db.root, &params.id).ok_or(NotFoundError("group"))?;

        let mut entries = Vec::with_capacity(group.children.len());
        for node in &group.children {
            if let Node::Entry(entry) = node {
                entries.push(
                    // Populate (potentially) visible fields only
                    Entry {
                        id: entry.uuid,
                        title: entry.get_title().map(String::from),
                        username: entry.get_username().map(String::from),
                        notes: None,
                        strings: None,
                        binary: None,
                        protected: None,
                        tags: None,
                        icon: entry.icon_id,
                        custom_icon_uuid: entry.custom_icon_uuid,
                        url: entry.get_url().map(String::from),
                    }
                )
            }
        }

        Ok(EntryGroup {
            id: group.uuid,
            title: group.name.clone(),
            entries,
            icon: group.icon_id,
            custom_icon_uuid: group.custom_icon_uuid,
        })
    }

    pub fn get_entry(&self, params: &Query<Id>) -> Result<Entry> {
        let entry = Self::find_entry_by_id(&self.db.root, &params.id).ok_or(NotFoundError("entry"))?;

        Ok(entry.into())
    }

    pub fn get_protected(&self, params: &Query<Protected>) -> Result<SecretString> {
        let entry = Self::find_entry_by_id(&self.db.root, &params.entry_id).ok_or(NotFoundError("entry"))?;

        let field = match params.name.as_str() {
            "password" => entry.fields.get("Password").cloned(),
            k => entry.fields.get(k).cloned(),
        };

        let protected = match field {
            Some(v) => match v {
                Value::Protected(p) => p,
                _ => bail!("not a protected field"),
            },
            None => return Err(NotFoundError("field").into()),
        };

        Ok(
            SecretString::new(
                String::from_utf8_lossy(protected.unsecure()).to_string()
            )
        )
    }

    pub fn search_entries(&self, params: &Query<SearchTerm>) -> Result<EntryGroup> {
        let mut term = params.term.clone();
        if !self.config.search.allow_regex {
            term = regex::escape(&params.term);
        }
        let rgx = Regex::new(&format!("(?i){}", term))?;
        let entries = Self::find_entries_by_string(&self.db.root, &rgx, &self.config.search);

        Ok(EntryGroup {
            id: Uuid::nil(),
            title: format!("Search results for '{}'", params.term),
            entries,
            // search icon
            icon: Some(40),
            custom_icon_uuid: None,
        })
    }

    pub fn get_icon(&self, params: &Path<Id>) -> Result<Icon> {
        // TODO: can we improve this?
        for icon in &self.db.meta.custom_icons.icons {
            if icon.uuid == params.id {
                return Ok(icon.clone());
            }
        }

        Err(NotFoundError("icon").into())
    }

    pub(crate) fn find_all_groups(group: &keepass::db::Group) -> Group {
        let mut children: Vec<Group> = Vec::with_capacity(group.children.len());
        for node in &group.children {
            if let Node::Group(group) = node {
                children.push(Self::find_all_groups(group));
            }
        }
        Group {
            id: group.uuid,
            title: group.name.clone(),
            icon: group.icon_id,
            custom_icon_uuid: None,
            children,
            expanded: group.is_expanded,
        }
    }

    pub(crate) fn find_group_by_id<'a>(group: &'a keepass::db::Group, id: &Uuid) -> Option<&'a keepass::db::Group> {
        if &group.uuid == id {
            return Some(group);
        }
        for node in &group.children {
            if let Node::Group(group) = node {
                let found = Self::find_group_by_id(group, id);
                if found.is_some() {
                    return found;
                }
            }
        }

        None
    }

    pub(crate) fn find_entry_by_id<'a>(group: &'a keepass::db::Group, id: &Uuid) -> Option<&'a keepass::db::Entry> {
        for node in &group.children {
            match node {
                Node::Group(group) => {
                    let found = Self::find_entry_by_id(group, id);
                    if found.is_some() {
                        return found;
                    }
                }
                Node::Entry(entry) => {
                    if &entry.uuid == id {
                        return Some(entry);
                    }
                }
            }
        }

        None
    }

    fn find_group_by_id_mut<'a>(group: &'a mut KpGroup, id: &Uuid) -> Option<&'a mut KpGroup> {
        if &group.uuid == id {
            return Some(group);
        }
        for node in &mut group.children {
            if let Node::Group(g) = node {
                let found = Self::find_group_by_id_mut(g, id);
                if found.is_some() {
                    return found;
                }
            }
        }
        None
    }

    fn find_entry_by_id_mut<'a>(group: &'a mut KpGroup, id: &Uuid) -> Option<&'a mut KpEntry> {
        for node in &mut group.children {
            match node {
                Node::Group(g) => {
                    let found = Self::find_entry_by_id_mut(g, id);
                    if found.is_some() {
                        return found;
                    }
                }
                Node::Entry(e) => {
                    if &e.uuid == id {
                        return Some(e);
                    }
                }
            }
        }
        None
    }

    fn remove_entry_from_group(group: &mut KpGroup, id: &Uuid) -> bool {
        let before = group.children.len();
        group.children.retain(|node| match node {
            Node::Entry(e) => &e.uuid != id,
            _ => true,
        });
        if group.children.len() < before {
            return true;
        }
        for node in &mut group.children {
            if let Node::Group(g) = node {
                if Self::remove_entry_from_group(g, id) {
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn find_entries_by_string(group: &keepass::db::Group, term: &Regex, config: &Search) -> Vec<Entry> {
        let mut entries = vec![];

        for node in &group.children {
            match node {
                Node::Group(group) => {
                    entries.append(&mut Self::find_entries_by_string(group, term, config));
                }
                Node::Entry(entry) => {
                    let entry: Entry = entry.into();
                    if entry.matches_regex(term, config) {
                        entries.push(entry);
                    }
                }
            }
        }

        entries
    }
}

#[cfg(test)]
mod tests {
    use tokio::fs;

    use crate::db_backend::test::Test;

    use super::*;

    fn db_key() -> DatabaseKey {
        DatabaseKey::new().with_password("test")
    }

    #[tokio::test]
    async fn database_roundtrip() {
        let params = DbLogin {
            password: Some("test".to_string()),
            key: None,
        };
        let config = Config::default();

        let mut backend = Test::new();
        backend.buf = fs::read("tests/test.kdbx").await.unwrap();

        let user_info = UserInfo::default();
        let keepass = KeePass::from_backend(&config, &mut backend, &params, &user_info).await.unwrap();

        let (mut key, enc) = keepass.to_enc().unwrap();

        // tests always use the in-memory store; the keyring is unavailable in most CI/test environments
        key.store(config.db_session_timeout, false).unwrap();
        let ret_key = SecretKey::retrieve(&key.key_id, config.db_session_timeout, false).unwrap();

        let dec = KeePass::from_enc(&config, ret_key, enc).unwrap();

        // can't clone, so we read in another one
        let keepass = KeePass::from_backend(&config, &mut backend, &params, &user_info).await.unwrap();

        assert_eq!(keepass.db, dec.db);
    }

    // saving cannot carry file attachments over yet, so it has to refuse
    // rather than write a database that silently lost them
    #[tokio::test]
    async fn saving_a_database_with_attachments_is_refused() {
        let params = DbLogin {
            password: Some("test".to_string()),
            key: None,
        };
        let config = Config::default();
        let user_info = UserInfo::default();

        let mut backend = Test::new();
        backend.buf = fs::read("tests/test.kdbx").await.unwrap();

        let keepass = KeePass::from_backend(&config, &mut backend, &params, &user_info).await.unwrap();
        assert!(keepass.attachment_count() > 0, "the fixture is expected to carry an attachment");

        let before = backend.buf.clone();
        let err = keepass.to_backend_with_key(&mut backend, db_key(), &user_info).await
            .err().expect("saving must refuse");

        assert!(err.to_string().contains("attachment"), "{:#}", err);
        assert_eq!(backend.buf, before, "the database must be left untouched");
    }
}
