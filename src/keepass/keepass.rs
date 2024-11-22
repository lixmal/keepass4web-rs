use actix_web::web::{Path, Query};
use anyhow::{anyhow, bail};
use anyhow::Result;
use base64;
use base64::Engine;
use base64::engine::general_purpose;
use keepass::{Database, DatabaseKey};
use keepass::db::{Icon, Node, Value};
use regex::Regex;
use secrecy::{ExposeSecret, SecretString};
use serde::__private::from_utf8_lossy;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use totp_rs::TOTP;
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

#[derive(Serialize)]
pub struct Totp {
    token: String,
    ttl: u64,
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

    pub async fn from_backend(config: &Config, db_backend: &dyn DbBackend, params: &DbLogin, user_info: &UserInfo) -> Result<Self> {
        let db_key = Self::db_key_from_params(db_backend, params, user_info).await?;

        let mut reader = db_backend.get_db_read(user_info).await?;

        // bridge sync and async by caching the whole file in memory for now
        let mut buf = vec![];
        reader.read_to_end(&mut buf).await?;

        let db = tokio::task::spawn_blocking(move || {
            let db = Database::open(&mut buf.as_slice(), db_key);
            buf.zeroize();
            db
        }).await??;

        Ok(
            KeePass {
                config: config.clone(),
                db,
            }
        )
    }

    #[allow(dead_code)]
    pub async fn to_backend(self, db_backend: &mut dyn DbBackend, params: &DbLogin, user_info: &UserInfo) -> Result<()> {
        let key = Self::db_key_from_params(db_backend, params, user_info).await?;

        let mut buf: Vec<u8> = vec![];
        let (result, mut buf) = tokio::task::spawn_blocking(move || {
            (self.db.save(&mut buf, key), buf)
        }).await?;
        result?;

        let (mut writer, rx) = db_backend.get_db_write(user_info).await?;
        writer.write_all(&buf).await?;
        buf.zeroize();

        // close our side to signal end of data
        // otherwise we could get a deadlock awaiting the channel
        writer.shutdown().await?;
        if let Some(rx) = rx {
            rx.await??;
        }

        Ok(())
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
        let group = Self::find_group_by_id(&self.db.root, &params.id).ok_or(anyhow!("group not found"))?;

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
                        otp: false,
                    }
                )
            }
        }

        Ok(EntryGroup {
            title: group.name.clone(),
            entries,
            icon: group.icon_id,
            custom_icon_uuid: group.custom_icon_uuid,
        })
    }

    pub fn get_entry(&self, params: &Query<Id>) -> Result<Entry> {
        let entry = Self::find_entry_by_id(&self.db.root, &params.id).ok_or(anyhow!("entry not found"))?;

        Ok(entry.into())
    }

    pub fn get_protected(&self, params: &Query<Protected>) -> Result<SecretString> {
        let entry = Self::find_entry_by_id(&self.db.root, &params.entry_id).ok_or(anyhow!("entry not found"))?;

        let field = match params.name.as_str() {
            "password" => entry.fields.get("Password").cloned(),
            k => entry.fields.get(k).cloned(),
        };

        let protected = match field {
            Some(v) => match v {
                Value::Protected(p) => p,
                _ => bail!("not a protected field"),
            },
            None => bail!("field not found"),
        };

        Ok(
            SecretString::new(
                String::from_utf8_lossy(protected.unsecure()).to_string()
            )
        )
    }

    pub fn get_otp(&self, params: &Query<Id>) -> Result<Totp> {
        let entry = Self::find_entry_by_id(&self.db.root, &params.id).ok_or(anyhow!("entry not found"))?;

        let otp = match entry.fields.get("otp") {
            Some(v) => match v {
                Value::Unprotected(u) => u.clone(),
                Value::Protected(p) => from_utf8_lossy(p.unsecure()).to_string(),
                Value::Bytes(b) => from_utf8_lossy(b).to_string(),
            },
            None => bail!("otp not found"),
        };

        let totp = TOTP::from_url(&otp)?;

        Ok(
            Totp {
                token: totp.generate_current()?,
                ttl: totp.ttl()?,
            }
        )
    }

    pub fn get_file(&self, params: &Query<File>) -> Result<Vec<u8>> {
        let _entry = Self::find_entry_by_id(&self.db.root, &params.entry_id).ok_or(anyhow!("entry not found"))?;

        todo!()
    }

    pub fn search_entries(&self, params: &Query<SearchTerm>) -> Result<EntryGroup> {
        let mut term = params.term.clone();
        if !self.config.search.allow_regex {
            term = regex::escape(&params.term);
        }
        let rgx = Regex::new(&format!("(?i){}", term))?;
        let entries = Self::find_entries_by_string(&self.db.root, &rgx, &self.config.search);

        Ok(EntryGroup {
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

        bail!("icon not found")
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

    use crate::config::backend::DbBackend;
    use crate::db_backend;
    use crate::db_backend::test::Test;

    use super::*;

    #[tokio::test]
    async fn database_roundtrip() {
        let params = DbLogin {
            password: Some("test".to_string()),
            key: None,
        };
        let mut config = Config::default();
        config.db_backend = DbBackend::Test;

        let mut db_backend = db_backend::new(&config);
        let test_backend: &mut Test = db_backend.as_any().downcast_mut().unwrap();
        test_backend.buf.extend_from_slice(&fs::read("tests/test.kdbx").await.unwrap());

        let user_info = UserInfo::default();
        let keepass = KeePass::from_backend(&config, test_backend, &params, &user_info).await.unwrap();

        let (mut key, enc) = keepass.to_enc().unwrap();

        key.store(config.db_session_timeout).unwrap();
        let ret_key = SecretKey::retrieve(&key.key_id, config.db_session_timeout).unwrap();

        let dec = KeePass::from_enc(&config, ret_key, enc).unwrap();

        // can't clone, so we read in another one
        let keepass = KeePass::from_backend(&config, test_backend, &params, &user_info).await.unwrap();

        assert_eq!(keepass.db, dec.db);

        test_backend.buf = Vec::new();
        keepass.to_backend(test_backend, &params, &user_info).await.unwrap();

        // TODO: compare KeePass::to_backend result
    }
}
