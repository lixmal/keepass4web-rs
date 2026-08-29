//! A record of what was done to the vault.
//!
//! Reading a password is the most sensitive thing this application does, and
//! until now only the failures were logged: a successful reveal left no trace
//! at all, so "which secrets were read, by whom, when" could not be answered
//! after an incident.
//!
//! The entries go to their own log target so an operator can route or keep
//! them separately from the ordinary application log:
//!
//! ```text
//! RUST_LOG=warn,audit=info
//! ```
//!
//! What they may contain is deliberately narrow. An audit trail that recorded
//! entry titles, group names or field values would itself become a copy of the
//! vault, in a file that is kept longer and guarded less. So it records who
//! acted, what they did, and the identifier of what they did it to. Resolving
//! an identifier back to an entry needs the database, which needs the master
//! password.

use log::info;
use uuid::Uuid;

use crate::keepass::keepass::{field_name as resolve, STANDARD_FIELDS};

/// The log target these records carry, so they can be filtered on their own.
pub const TARGET: &str = "audit";

pub const OPENED: &str = "db.opened";
pub const CLOSED: &str = "db.closed";
pub const SAVED: &str = "db.saved";
pub const REVEALED: &str = "entry.revealed";
pub const DOWNLOADED: &str = "entry.downloaded";
pub const ENTRY_CREATED: &str = "entry.created";
pub const ENTRY_UPDATED: &str = "entry.updated";
pub const ENTRY_DELETED: &str = "entry.deleted";
pub const ENTRY_MOVED: &str = "entry.moved";
pub const GROUP_CREATED: &str = "group.created";
pub const GROUP_UPDATED: &str = "group.updated";
pub const GROUP_DELETED: &str = "group.deleted";
pub const GROUP_MOVED: &str = "group.moved";

/// Something that happened to the database as a whole.
pub fn database(user: &str, action: &str) {
    info!(target: TARGET, "user={:?} action={}", user, action);
}

/// Something that happened to one entry or group, named by its identifier.
pub fn node(user: &str, action: &str, id: &Uuid) {
    info!(target: TARGET, "user={:?} action={} id={}", user, action, id);
}

/// A protected field that was handed to the client.
pub fn revealed(user: &str, id: &Uuid, field: &str) {
    info!(target: TARGET, "user={:?} action={} id={} field={}", user, REVEALED, id, field_name(field));
}

/// The name a field may be recorded under.
///
/// Only the ones the format defines. A custom field is named by whoever added
/// it, and those names describe the secret they hold closely enough to belong
/// with it rather than in a log.
///
/// The name is resolved the way the request was, so the reveal that matters
/// most is not filed under "custom" because the client asked for `password`
/// rather than `Password`.
fn field_name(asked: &str) -> &str {
    let field = resolve(asked);

    if STANDARD_FIELDS.contains(&field) { field } else { "custom" }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The trail says what was read, not what it said, and a field name the
    // user wrote is close enough to the secret to leave out.
    #[test]
    fn a_custom_field_is_not_named() {
        assert_eq!(field_name("Password"), "Password");
        assert_eq!(field_name("UserName"), "UserName");
        assert_eq!(field_name("Notes"), "Notes");

        assert_eq!(field_name("Recovery codes for my bank"), "custom");
    }

    // the interface asks for the password under its own name, and a password
    // reveal is the record this whole trail exists for: it must not end up
    // filed as an unnamed custom field
    #[test]
    fn the_password_is_named_however_it_was_asked_for() {
        assert_eq!(field_name("password"), "Password");
        assert_eq!(field_name("Password"), "Password");
    }
}
