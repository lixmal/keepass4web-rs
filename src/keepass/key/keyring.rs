// linux kernel keyring backed key store

use std::time::Duration;

use anyhow::Result;
use linux_keyutils::{KeyError, KeyPermissions, KeyRing, KeyRingIdentifier};

use super::KeyUnavailableError;

const KEYRING_PERM: u32 = 0x3f000000;

pub(super) fn store(key_id: &str, secret: &[u8], timeout: Duration) -> Result<()> {
    let keyr = get_keyring()?;

    let key = keyr.add_key(key_id, secret).map_err(map_err)?;
    key.set_timeout(timeout.as_secs() as usize).map_err(map_err)?;
    key.set_perms(KeyPermissions::from_u32(KEYRING_PERM)).map_err(map_err)?;

    Ok(())
}

pub(super) fn retrieve(key_id: &str, timeout: Duration) -> Result<Box<[u8]>> {
    let keyr = get_keyring()?;

    let key = keyr.search(key_id).map_err(map_err)?;

    // read the actual payload length instead of assuming 32 bytes,
    // so secrets of any length roundtrip unchanged
    let data = key.read_to_vec().map_err(map_err)?.into_boxed_slice();

    key.set_timeout(timeout.as_secs() as usize).map_err(map_err)?;

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
