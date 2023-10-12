use std::collections::HashMap;

use keepass::db::Value;
use regex::Regex;
use serde::Serialize;
use uuid::Uuid;

use crate::config::search::{Field, Search};

#[derive(Serialize)]
pub struct Group {
    pub id: Uuid,
    pub title: String,
    pub icon: Option<usize>,
    pub custom_icon_uuid: Option<Uuid>,
    pub children: Vec<Group>,
    pub expanded: bool,
}

#[derive(Serialize)]
pub struct EntryGroup {
    pub title: String,
    pub icon: Option<usize>,
    pub custom_icon_uuid: Option<Uuid>,
    pub entries: Vec<Entry>,
}

#[derive(Serialize)]
pub struct Entry {
    pub id: Uuid,
    pub title: Option<String>,
    pub username: Option<String>,
    pub notes: Option<String>,
    pub binary: Option<Vec<String>>,
    pub protected: Option<HashMap<String, ()>>,
    pub tags: Option<Vec<String>>,
    pub icon: Option<usize>,
    pub custom_icon_uuid: Option<Uuid>,
    pub url: Option<String>,
    pub strings: Option<HashMap<String, Option<String>>>,
}

impl From<&keepass::db::Entry> for Entry {
    fn from(entry: &keepass::db::Entry) -> Self {
        let files = vec![];
        let mut strings: HashMap<String, Option<String>> = Default::default();
        let mut protected: HashMap<String, ()> = Default::default();

        for (k, v) in &entry.fields {
            match v {
                Value::Bytes(_) => {}
                Value::Unprotected(s) => {
                    strings.insert(k.clone(), Some(s.clone()));
                }
                Value::Protected(_) => {
                    protected.insert(k.clone(), ());
                    strings.insert(k.clone(), None);
                }
            }
        }
        strings.remove("Password");

        // TODO: Don't hide empty protected strings
        Entry {
            id: entry.uuid,
            title: strings.remove("Title").flatten(),
            username: strings.remove("UserName").flatten(),
            notes: strings.remove("Notes").flatten(),
            binary: Some(files),
            protected: Some(protected),
            tags: Some(entry.tags.clone()),
            icon: entry.icon_id,
            custom_icon_uuid: entry.custom_icon_uuid,
            url: strings.remove("URL").flatten(),
            strings: Some(strings),
        }
    }
}

impl Entry {
    pub fn matches_regex(&self, term: &Regex, config: &Search) -> bool {
        for field in &config.fields {
            let tmp;
            let match_str = match field {
                Field::Title => self.title.as_deref().unwrap_or_default(),
                Field::Username => self.username.as_deref().unwrap_or_default(),
                Field::Tags => {
                    match &self.tags {
                        None => "",
                        Some(v) => {
                            tmp = v.join(";");
                            &tmp
                        }
                    }
                }
                Field::Notes => self.notes.as_deref().unwrap_or_default(),
                Field::Url => self.url.as_deref().unwrap_or_default(),
            };

            if term.is_match(match_str) {
                return true;
            }
        }

        if config.extra_fields && self.strings.is_some() {
            for (k, v) in self.strings.as_ref().unwrap() {
                if term.is_match(k) || term.is_match(v.as_deref().unwrap_or_default()) {
                    return true;
                }
            }
        }

        false
    }
}


