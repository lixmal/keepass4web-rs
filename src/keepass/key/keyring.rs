// linux kernel keyring backed key store

use std::time::Duration;

use anyhow::Result;
use linux_keyutils::{KeyError, KeyPermissions, KeyRing, KeyRingIdentifier};

use super::KeyUnavailableError;

const KEYRING_PERM: u32 = 0x3f000000;

// The kernel reads a timeout of zero as "never expires", so a timeout the
// keyring cannot express is rounded up to the shortest one it can rather than
// leaving the key in the keyring for good.
fn expiry_secs(timeout: Duration) -> usize {
    timeout.as_secs().max(1) as usize
}

pub(super) fn store(key_id: &str, secret: &[u8], timeout: Duration) -> Result<()> {
    let keyr = get_keyring()?;

    let key = keyr.add_key(key_id, secret).map_err(map_err)?;
    key.set_timeout(expiry_secs(timeout)).map_err(map_err)?;
    key.set_perms(KeyPermissions::from_u32(KEYRING_PERM)).map_err(map_err)?;

    Ok(())
}

pub(super) fn retrieve(key_id: &str, timeout: Duration) -> Result<Box<[u8]>> {
    let keyr = get_keyring()?;

    let key = keyr.search(key_id).map_err(map_err)?;

    // read the actual payload length instead of assuming 32 bytes,
    // so secrets of any length roundtrip unchanged
    let data = key.read_to_vec().map_err(map_err)?.into_boxed_slice();

    key.set_timeout(expiry_secs(timeout)).map_err(map_err)?;

    Ok(data)
}

pub(super) fn revoke(key_id: &str) -> Result<()> {
    let keyr = get_keyring()?;

    let key = keyr.search(key_id).map_err(map_err)?;
    key.revoke().map_err(map_err)?;

    Ok(())
}

fn map_err(err: KeyError) -> anyhow::Error {
    match err {
        KeyError::KeyDoesNotExist | KeyError::KeyExpired | KeyError::KeyRevoked => KeyUnavailableError.into(),
        err => anyhow::Error::new(err),
    }
}

fn get_keyring() -> Result<KeyRing> {
    // TODO: Make other keyrings available
    // TODO: Investigate why key in Process keyring doesn't persist
    Ok(KeyRing::from_special_id(KeyRingIdentifier::Session, false)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timeout_the_keyring_cannot_express_never_becomes_no_expiry() {
        // zero would clear the expiry timer and keep the key for good
        assert_eq!(expiry_secs(Duration::ZERO), 1);
        assert_eq!(expiry_secs(Duration::from_millis(1)), 1);
        assert_eq!(expiry_secs(Duration::from_secs(1)), 1);
        assert_eq!(expiry_secs(Duration::from_secs(300)), 300);
    }
}
