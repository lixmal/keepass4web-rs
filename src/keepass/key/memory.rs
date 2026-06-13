// in-memory key store for platforms without kernel keyrings.
// keys stay in (zeroized-on-drop) process memory instead of the kernel,
// which is weaker than the linux keyring but enables native macos/windows runs.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use anyhow::Result;
use zeroize::Zeroize;

use super::KeyUnavailableError;

struct Entry {
    secret: Vec<u8>,
    expiry: Instant,
}

impl Drop for Entry {
    fn drop(&mut self) {
        self.secret.zeroize();
    }
}

fn entries() -> &'static Mutex<HashMap<String, Entry>> {
    static STORE: OnceLock<Mutex<HashMap<String, Entry>>> = OnceLock::new();
    STORE.get_or_init(Default::default)
}

pub(super) fn store(key_id: &str, secret: &[u8], timeout: Duration) -> Result<()> {
    let mut store = entries().lock().unwrap();

    // drop expired keys so the map doesn't grow unboundedly
    let now = Instant::now();
    store.retain(|_, entry| now < entry.expiry);

    store.insert(key_id.to_string(), Entry {
        secret: secret.to_vec(),
        expiry: now + timeout,
    });

    Ok(())
}

pub(super) fn retrieve(key_id: &str, timeout: Duration) -> Result<Box<[u8]>> {
    let mut store = entries().lock().unwrap();

    let entry = store.get_mut(key_id).ok_or(KeyUnavailableError)?;
    if Instant::now() >= entry.expiry {
        store.remove(key_id);
        return Err(KeyUnavailableError.into());
    }

    entry.expiry = Instant::now() + timeout;
    Ok(entry.secret.clone().into_boxed_slice())
}

pub(super) fn revoke(key_id: &str) -> Result<()> {
    entries().lock().unwrap()
        .remove(key_id)
        .ok_or(KeyUnavailableError)?;

    Ok(())
}
