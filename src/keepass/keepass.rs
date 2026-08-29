use actix_web::web::{Path, Query};
use chrono::NaiveDateTime;
use anyhow::{anyhow, bail};
use anyhow::Result;
use base64;
use base64::Engine;
use base64::engine::general_purpose;
use keepass::{Database, DatabaseKey};
use keepass::config::DatabaseConfig;
use keepass::db::{CustomIcon, EntryId, GroupId, GroupRef, Icon, Value};
use regex::Regex;
use secrecy::{ExposeSecret, SecretString};
use serde::Deserialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;
use zeroize::Zeroize;

use crate::auth::DbLogin;
use crate::auth_backend::UserInfo;
use crate::config::config::Config;
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

// what clients call the recycle bin, and the bin icon from the standard set
const RECYCLE_BIN_NAME: &str = "Recycle Bin";
const RECYCLE_BIN_ICON: usize = 43;

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

// Everything a request can set on an entry. One struct rather than a dozen
// positional arguments, since create and update set exactly the same things.
pub struct EntryFields<'a> {
    pub title: &'a str,
    pub username: &'a str,
    pub password: &'a str,
    pub url: &'a str,
    pub notes: &'a str,
    pub icon: Option<usize>,
    pub tags: &'a [String],
    pub custom_fields: &'a [CustomField],
    pub expires: bool,
    pub expiry: Option<NaiveDateTime>,
}

impl EntryFields<'_> {
    fn apply(&self, entry: &mut keepass::db::EntryMut<'_>) {
        entry.set_unprotected("Title", self.title);
        entry.set_unprotected("UserName", self.username);
        // the client never receives the password, so it cannot send it back:
        // an empty one means "leave it alone", not "clear it"
        if !self.password.is_empty() {
            entry.set_protected("Password", self.password);
        }
        entry.set_unprotected("URL", self.url);
        entry.set_unprotected("Notes", self.notes);
        if let Some(id) = self.icon {
            entry.set_icon_builtin(id);
        }

        entry.times.expires = Some(self.expires);
        if self.expires {
            entry.times.expiry = self.expiry;
        }

        entry.tags = self.tags.to_vec();
        KeePass::apply_custom_fields(entry, self.custom_fields);
    }
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
        let db = Database::with_config(DatabaseConfig::default());

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


    pub fn create_entry(&mut self, group_id: &Uuid, fields: &EntryFields) -> Result<Uuid> {
        let group_id = GroupId::from_uuid(*group_id);
        let mut group = self.db.group_mut(group_id).ok_or(NotFoundError("group"))?;

        let mut entry = group.add_entry();
        fields.apply(&mut entry);

        Ok(entry.id().uuid())
    }

    pub fn update_entry(&mut self, entry_id: &Uuid, fields: &EntryFields) -> Result<()> {
        let entry_id = EntryId::from_uuid(*entry_id);
        let mut entry = self.db.entry_mut(entry_id).ok_or(NotFoundError("entry"))?;

        // editing through a tracked reference keeps the previous version in the
        // entry's history, the way every other client records an edit
        entry.edit_tracking(|entry| fields.apply(&mut entry.as_mut()));

        Ok(())
    }

    // The standard fields of an entry are named by the format, everything else
    // the user added is a custom field and is replaced wholesale by what the
    // request carries. A protected field sent without a value keeps the value
    // it has, the same way an empty password leaves the password alone: the
    // client never received it to send back.
    fn apply_custom_fields(entry: &mut keepass::db::EntryMut<'_>, custom_fields: &[CustomField]) {
        let kept: Vec<(String, Value<String>)> = custom_fields.iter()
            .map(|field| {
                let value = match (field.protected, field.value.is_empty()) {
                    (true, true) => match entry.fields.get(&field.name) {
                        Some(existing) if existing.is_protected() => Value::protected(existing.get().clone()),
                        _ => Value::protected(field.value.clone()),
                    },
                    (true, false) => Value::protected(field.value.clone()),
                    (false, _) => Value::unprotected(field.value.clone()),
                };

                (field.name.clone(), value)
            })
            .collect();

        entry.fields.retain(|name, _| STANDARD_FIELDS.contains(&name.as_str()));
        for (name, value) in kept {
            entry.set(name, value);
        }
    }

    pub fn create_group(&mut self, parent_id: &Uuid, name: &str) -> Result<Uuid> {
        let parent_id = GroupId::from_uuid(*parent_id);
        let mut parent = self.db.group_mut(parent_id).ok_or(NotFoundError("group"))?;

        let mut group = parent.add_group();
        group.name = name.to_string();

        Ok(group.id().uuid())
    }

    pub fn update_group(&mut self, group_id: &Uuid, name: &str, notes: Option<&str>, icon: Option<usize>) -> Result<()> {
        let group_id = GroupId::from_uuid(*group_id);
        let mut group = self.db.group_mut(group_id).ok_or(NotFoundError("group"))?;

        group.name = name.to_string();
        if let Some(notes) = notes {
            group.notes = Some(notes.to_string()).filter(|n| !n.is_empty());
        }
        if let Some(id) = icon {
            group.set_icon_builtin(id);
        }

        Ok(())
    }

    // Deleting sends the entry to the recycle bin, which is where every other
    // client puts it and where the user expects to find it again. An entry
    // already in the bin, or a database with no bin, is deleted outright, and a
    // tombstone records the deletion so other clients replicate it.
    pub fn delete_entry(&mut self, entry_id: &Uuid) -> Result<()> {
        let entry_id = EntryId::from_uuid(*entry_id);

        if self.db.entry(entry_id).is_none() {
            return Err(NotFoundError("entry").into());
        }

        if let Some(bin_id) = self.recycle_bin_for(entry_id)? {
            let mut entry = self.db.entry_mut(entry_id).ok_or(NotFoundError("entry"))?;
            entry.edit_tracking(|entry| {
                // the destination was just resolved, so it is there
                let _ = entry.move_to(bin_id);
            });
            return Ok(());
        }

        let mut entry = self.db.entry_mut(entry_id).ok_or(NotFoundError("entry"))?;
        entry.track_changes().remove();

        Ok(())
    }

    // The bin to send an entry to, or None when it is already in there and the
    // next delete has to be permanent.
    fn recycle_bin_for(&mut self, entry_id: EntryId) -> Result<Option<GroupId>> {
        if self.db.meta.recyclebin_enabled == Some(false) {
            return Ok(None);
        }

        if let Some(bin_id) = self.db.recycle_bin().map(|bin| bin.id()) {
            let parent = self.db.entry(entry_id).ok_or(NotFoundError("entry"))?.parent().id();
            if self.is_within(parent, bin_id) {
                return Ok(None);
            }
            return Ok(Some(bin_id));
        }

        Ok(Some(self.create_recycle_bin()))
    }

    // Creates the bin the way clients do, on the first delete that needs it.
    fn create_recycle_bin(&mut self) -> GroupId {
        let bin_id = {
            let mut root = self.db.root_mut();
            let mut bin = root.add_group();
            bin.name = RECYCLE_BIN_NAME.to_string();
            bin.set_icon_builtin(RECYCLE_BIN_ICON);
            bin.id()
        };

        self.db.meta.recyclebin_enabled = Some(true);
        self.db.meta.recyclebin_uuid = Some(bin_id.uuid());
        self.db.meta.recyclebin_changed = Some(keepass::db::Times::now());

        bin_id
    }

    // Whether a group is the given ancestor or sits somewhere below it.
    fn is_within(&self, group_id: GroupId, ancestor: GroupId) -> bool {
        let mut current = Some(group_id);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.db.group(id).and_then(|group| group.parent().map(|p| p.id()));
        }
        false
    }

    // Deleting a group takes everything under it, so it goes to the recycle bin
    // as a whole. A group already in the bin is removed outright, and the root
    // and the bin itself cannot be deleted at all.
    pub fn delete_group(&mut self, group_id: &Uuid) -> Result<()> {
        let group_id = GroupId::from_uuid(*group_id);

        if self.db.group(group_id).is_none() {
            return Err(NotFoundError("group").into());
        }
        if group_id == self.db.root().id() {
            bail!("the root group cannot be deleted");
        }

        let bin_id = self.db.recycle_bin().map(|bin| bin.id());
        if bin_id == Some(group_id) {
            bail!("the recycle bin cannot be deleted");
        }

        let in_bin = bin_id.is_some_and(|bin_id| self.is_within(group_id, bin_id));
        let disabled = self.db.meta.recyclebin_enabled == Some(false);

        if in_bin || disabled {
            let mut group = self.db.group_mut(group_id).ok_or(NotFoundError("group"))?;
            group.track_changes().remove()?;
            return Ok(());
        }

        let bin_id = match bin_id {
            Some(id) => id,
            None => self.create_recycle_bin(),
        };

        let mut group = self.db.group_mut(group_id).ok_or(NotFoundError("group"))?;
        group.track_changes().move_to(bin_id)?;

        Ok(())
    }

    // Moving an entry to another group, which is how a vault gets reorganised.
    pub fn move_entry(&mut self, entry_id: &Uuid, group_id: &Uuid) -> Result<()> {
        let entry_id = EntryId::from_uuid(*entry_id);
        let group_id = GroupId::from_uuid(*group_id);

        if self.db.group(group_id).is_none() {
            return Err(NotFoundError("group").into());
        }

        let mut entry = self.db.entry_mut(entry_id).ok_or(NotFoundError("entry"))?;
        entry.edit_tracking(|entry| {
            // the destination was just checked, so it is there
            let _ = entry.move_to(group_id);
        });

        Ok(())
    }

    // Moving a group under another one. A group cannot be moved into itself or
    // into one of its own descendants, which the library rejects for us.
    pub fn move_group(&mut self, group_id: &Uuid, parent_id: &Uuid) -> Result<()> {
        let group_id = GroupId::from_uuid(*group_id);
        let parent_id = GroupId::from_uuid(*parent_id);

        if self.db.group(parent_id).is_none() {
            return Err(NotFoundError("group").into());
        }

        let mut group = self.db.group_mut(group_id).ok_or(NotFoundError("group"))?;
        group.track_changes().move_to(parent_id)?;

        Ok(())
    }

    #[cfg(test)]
    pub fn attachment_count(&self) -> usize {
        self.db.num_attachments()
    }

    // The bytes of one file attached to an entry, so it can be downloaded.
    pub fn get_file(&self, params: &Query<File>) -> Result<Vec<u8>> {
        let entry_id = EntryId::from_uuid(params.entry_id);
        let entry = self.db.entry(entry_id).ok_or(NotFoundError("entry"))?;

        let attachment = entry.attachment_by_name(&params.filename)
            .ok_or(NotFoundError("attachment"))?;

        Ok(attachment.data.get().clone())
    }

    pub async fn to_backend_with_key(self, db_backend: &mut dyn DbBackend, db_key: DatabaseKey, user_info: &UserInfo) -> Result<()> {
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
            if self.db.group(GroupId::from_uuid(v)).is_none() {
                last_selected = None;
            }
        }

        Ok((Self::group_tree(&self.db.root()), last_selected))
    }

    pub fn get_group_entries(&self, params: &Query<Id>) -> Result<EntryGroup> {
        let group = self.db.group(GroupId::from_uuid(params.id)).ok_or(NotFoundError("group"))?;

        // Populate (potentially) visible fields only
        let entries = group.entries()
            .map(|entry| Entry {
                id: entry.id().uuid(),
                title: entry.get_title().map(String::from),
                username: entry.get_username().map(String::from),
                notes: None,
                strings: None,
                binary: None,
                protected: None,
                tags: None,
                icon: builtin_icon(&entry.icon().cloned()),
                custom_icon_uuid: custom_icon_uuid(&entry.icon().cloned()),
                url: entry.get_url().map(String::from),
                // the list marks expired entries, so it needs the date
                expires: entry.times.expires,
                expiry: entry.times.expiry.map(|t| t.and_utc().to_rfc3339()),
                times: None,
                history: None,
            })
            .collect();

        Ok(EntryGroup {
            id: group.id().uuid(),
            title: group.name.clone(),
            entries,
            icon: builtin_icon(&group.icon().cloned()),
            custom_icon_uuid: custom_icon_uuid(&group.icon().cloned()),
        })
    }

    pub fn get_entry(&self, params: &Query<Id>) -> Result<Entry> {
        let entry = self.db.entry(EntryId::from_uuid(params.id)).ok_or(NotFoundError("entry"))?;

        Ok(Entry::from(&entry))
    }

    pub fn get_protected(&self, params: &Query<Protected>) -> Result<SecretString> {
        let entry = self.db.entry(EntryId::from_uuid(params.entry_id)).ok_or(NotFoundError("entry"))?;

        let name = match params.name.as_str() {
            "password" => "Password",
            k => k,
        };

        let field = entry.fields.get(name).ok_or(NotFoundError("field"))?;
        if !field.is_protected() {
            bail!("not a protected field");
        }

        Ok(SecretString::new(field.get().clone()))
    }

    pub fn search_entries(&self, params: &Query<SearchTerm>) -> Result<EntryGroup> {
        let mut term = params.term.clone();
        if !self.config.search.allow_regex {
            term = regex::escape(&params.term);
        }
        let rgx = Regex::new(&format!("(?i){}", term))?;

        let entries = self.db.iter_all_entries()
            .map(|entry| Entry::from(&entry))
            .filter(|entry| entry.matches_regex(&rgx, &self.config.search))
            .collect();

        Ok(EntryGroup {
            id: Uuid::nil(),
            title: format!("Search results for '{}'", params.term),
            entries,
            // search icon
            icon: Some(40),
            custom_icon_uuid: None,
        })
    }

    pub fn get_icon(&self, params: &Path<Id>) -> Result<CustomIcon> {
        self.db.iter_all_custom_icons()
            .find(|icon| icon.id().uuid() == params.id)
            .map(|icon| icon.clone())
            .ok_or(NotFoundError("icon").into())
    }

    fn group_tree(group: &GroupRef<'_>) -> Group {
        Group {
            id: group.id().uuid(),
            title: group.name.clone(),
            icon: builtin_icon(&group.icon().cloned()),
            custom_icon_uuid: custom_icon_uuid(&group.icon().cloned()),
            children: group.groups().map(|child| Self::group_tree(&child)).collect(),
            expanded: group.is_expanded,
            notes: group.notes.clone(),
        }
    }
}

// The two icon shapes the client understands: a built-in index, or the uuid of
// an icon stored in the database itself.
pub(crate) fn builtin_icon(icon: &Option<Icon>) -> Option<usize> {
    match icon {
        Some(Icon::BuiltIn(id)) => Some(*id),
        _ => None,
    }
}

pub(crate) fn custom_icon_uuid(icon: &Option<Icon>) -> Option<Uuid> {
    match icon {
        Some(Icon::Custom(id)) => Some(id.uuid()),
        _ => None,
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

    // saving used to leave attachments orphaned, so this is the case that has
    // to keep working: the bytes come back out of a saved database unchanged
    #[tokio::test]
    async fn attachments_survive_a_save() {
        let params = DbLogin {
            password: Some("test".to_string()),
            key: None,
        };
        let config = Config::default();
        let user_info = UserInfo::default();

        let mut backend = Test::new();
        backend.buf = fs::read("tests/test.kdbx").await.unwrap();

        let keepass = KeePass::from_backend(&config, &mut backend, &params, &user_info).await.unwrap();
        let before = keepass.attachment_count();
        assert!(before > 0, "the fixture is expected to carry an attachment");

        let names: Vec<(Uuid, String, Vec<u8>)> = keepass.db.iter_all_entries()
            .flat_map(|entry| {
                entry.attachments_named()
                    .map(|(name, attachment)| {
                        (entry.id().uuid(), name.to_string(), attachment.data.get().clone())
                    })
                    .collect::<Vec<_>>()
            })
            .collect();

        keepass.to_backend_with_key(&mut backend, db_key(), &user_info).await.unwrap();

        let after = KeePass::from_backend(&config, &mut backend, &params, &user_info).await.unwrap();
        assert_eq!(after.attachment_count(), before, "an attachment was lost on save");

        for (entry_id, name, data) in names {
            let params = Query::from_query(&format!(
                "entry_id={}&filename={}", entry_id, urlencoding(&name),
            )).unwrap();

            assert_eq!(after.get_file(&params).unwrap(), data, "'{}' came back different", name);
        }
    }

    fn urlencoding(name: &str) -> String {
        name.replace(' ', "%20").replace('&', "%26")
    }
}
