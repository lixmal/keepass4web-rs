use std::collections::HashMap;

use chrono::NaiveDateTime;
use keepass::db::{Entry as KpEntry, EntryRef};
use regex::Regex;
use serde::Serialize;
use uuid::Uuid;

use crate::config::search::{Field, Search};
use crate::keepass::keepass::{builtin_icon, custom_icon_uuid};

#[derive(Serialize)]
pub struct Group {
    pub id: Uuid,
    pub title: String,
    pub icon: Option<usize>,
    pub custom_icon_uuid: Option<Uuid>,
    pub children: Vec<Group>,
    pub expanded: bool,
    pub notes: Option<String>,
}

#[derive(Serialize)]
pub struct EntryGroup {
    pub id: Uuid,
    pub title: String,
    pub icon: Option<usize>,
    pub custom_icon_uuid: Option<Uuid>,
    pub entries: Vec<Entry>,
}

// When an entry was created, changed and last looked at. Read-only: the
// database keeps them, the user does not set them.
#[derive(Serialize)]
pub struct Times {
    pub created: Option<String>,
    pub modified: Option<String>,
    pub accessed: Option<String>,
    pub usage_count: Option<usize>,
}

// A previous version of an entry, enough of it to tell the versions apart and
// to see what a field used to hold.
#[derive(Serialize)]
pub struct HistoryEntry {
    pub title: Option<String>,
    pub username: Option<String>,
    pub url: Option<String>,
    pub notes: Option<String>,
    pub modified: Option<String>,
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
    pub expires: Option<bool>,
    pub expiry: Option<String>,
    pub times: Option<Times>,
    pub history: Option<Vec<HistoryEntry>>,
}

fn stamp(time: &Option<NaiveDateTime>) -> Option<String> {
    time.map(|t| t.and_utc().to_rfc3339())
}

// A field's value, but only when it is not protected. Any field can be marked
// protected, including the ones an entry normally shows, and a protected value
// is the user's to ask for by name rather than something to hand out with the
// entry. Entry::get would unprotect it for us, which is exactly what we do not
// want here.
fn unprotected(entry: &KpEntry, name: &str) -> Option<String> {
    entry.fields.get(name)
        .filter(|value| !value.is_protected())
        .map(|value| value.get().clone())
}

impl From<&EntryRef<'_>> for Entry {
    fn from(entry: &EntryRef<'_>) -> Self {
        let mut strings: HashMap<String, Option<String>> = Default::default();
        let mut protected: HashMap<String, ()> = Default::default();

        for (k, v) in &entry.fields {
            if v.is_protected() {
                protected.insert(k.clone(), ());
                strings.insert(k.clone(), None);
            } else {
                strings.insert(k.clone(), Some(v.get().clone()));
            }
        }
        strings.remove("Password");

        let files = entry.attachments_named()
            .map(|(name, _)| name.to_string())
            .collect();

        let history = entry.history.as_ref().map(|history| {
            history.get_entries().iter()
                .map(|old| HistoryEntry {
                    title: unprotected(old, "Title"),
                    username: unprotected(old, "UserName"),
                    url: unprotected(old, "URL"),
                    notes: unprotected(old, "Notes"),
                    modified: stamp(&old.times.last_modification),
                })
                .collect()
        });

        let icon = entry.icon().cloned();

        // TODO: Don't hide empty protected strings
        Entry {
            id: entry.id().uuid(),
            title: strings.remove("Title").flatten(),
            username: strings.remove("UserName").flatten(),
            notes: strings.remove("Notes").flatten(),
            binary: Some(files),
            protected: Some(protected),
            tags: Some(entry.tags.clone()),
            icon: builtin_icon(&icon),
            custom_icon_uuid: custom_icon_uuid(&icon),
            url: strings.remove("URL").flatten(),
            strings: Some(strings),
            expires: entry.times.expires,
            expiry: stamp(&entry.times.expiry),
            times: Some(Times {
                created: stamp(&entry.times.creation),
                modified: stamp(&entry.times.last_modification),
                accessed: stamp(&entry.times.last_access),
                usage_count: entry.times.usage_count,
            }),
            history,
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

#[cfg(test)]
mod tests {
    use keepass::db::{Database, Value};

    use super::*;

    // Any field can be marked protected, including the ones an entry normally
    // shows. A previous version of one must not hand the value out with the
    // entry when the current version would not.
    #[test]
    fn a_protected_field_stays_hidden_in_history() {
        let mut db = Database::new();
        let entry_id = {
            let mut root = db.root_mut();
            let mut entry = root.add_entry();
            entry.set_unprotected("Title", "visible");
            entry.set("Notes", Value::protected("a secret note"));
            entry.id()
        };

        // an edit through a tracked reference keeps the old version in history
        db.entry_mut(entry_id).unwrap().edit_tracking(|entry| {
            entry.set_unprotected("Title", "still visible");
        });

        let entry = Entry::from(&db.entry(entry_id).unwrap());
        let history = entry.history.expect("the edit should have been recorded");
        assert_eq!(history.len(), 1, "expected one previous version");

        assert_eq!(history[0].title.as_deref(), Some("visible"));
        assert_eq!(history[0].notes, None, "a protected note must not be handed out");

        // and the current version hides it the same way
        assert_eq!(entry.notes, None);
        assert!(entry.protected.unwrap().contains_key("Notes"));
    }
}
