use std::fmt::{Display, Formatter};
use std::ops::Deref;
use std::time::Duration;

use anyhow::Result;
use secrecy::{ExposeSecret, SecretBox};

use crate::auth::gen_token;

#[cfg(target_os = "linux")]
mod keyring;

// always compiled: used on non-linux, in tests, and when use_keyring=false on linux
pub(super) mod memory;

pub type KeyId = String;

const ID_LENGTH: usize = 16;

// returned when a key is missing, expired or revoked - the http layer
// treats this as 'db closed' rather than a server fault
#[derive(Debug, Clone)]
pub struct KeyUnavailableError;

impl Display for KeyUnavailableError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "key not found, expired or revoked")
    }
}

impl std::error::Error for KeyUnavailableError {}

pub struct SecretKey {
    pub key_id: KeyId,
    data: SecretBox<[u8]>,
    timeout: Duration,
    use_keyring: bool,
}

impl SecretKey {
    pub fn new(secret: Box<[u8]>) -> Self {
        Self {
            key_id: gen_token(ID_LENGTH),
            data: SecretBox::new(secret),
            timeout: Duration::default(),
            use_keyring: false,
        }
    }

    pub fn retrieve(key_id: &KeyId, timeout: Duration, use_keyring: bool) -> Result<Self> {
        let data = Self::dispatch_retrieve(key_id, timeout, use_keyring)?;

        Ok(
            Self {
                key_id: key_id.clone(),
                data: SecretBox::new(data),
                timeout,
                use_keyring,
            }
        )
    }

    pub fn store(&mut self, timeout: Duration, use_keyring: bool) -> Result<&mut Self> {
        self.use_keyring = use_keyring;
        Self::dispatch_store(&self.key_id, self.expose_secret(), timeout, use_keyring)?;

        self.timeout = timeout;
        Ok(self)
    }

    // retrieve will fail after revoke
    pub fn revoke(&mut self) -> Result<&mut Self> {
        Self::dispatch_revoke(&self.key_id, self.use_keyring)?;

        Ok(self)
    }

    #[cfg(target_os = "linux")]
    fn dispatch_store(key_id: &str, secret: &[u8], timeout: Duration, use_keyring: bool) -> Result<()> {
        if use_keyring {
            keyring::store(key_id, secret, timeout)
        } else {
            memory::store(key_id, secret, timeout)
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn dispatch_store(key_id: &str, secret: &[u8], timeout: Duration, _use_keyring: bool) -> Result<()> {
        memory::store(key_id, secret, timeout)
    }

    #[cfg(target_os = "linux")]
    fn dispatch_retrieve(key_id: &str, timeout: Duration, use_keyring: bool) -> Result<Box<[u8]>> {
        if use_keyring {
            keyring::retrieve(key_id, timeout)
        } else {
            memory::retrieve(key_id, timeout)
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn dispatch_retrieve(key_id: &str, timeout: Duration, _use_keyring: bool) -> Result<Box<[u8]>> {
        memory::retrieve(key_id, timeout)
    }

    #[cfg(target_os = "linux")]
    fn dispatch_revoke(key_id: &str, use_keyring: bool) -> Result<()> {
        if use_keyring {
            keyring::revoke(key_id)
        } else {
            memory::revoke(key_id)
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn dispatch_revoke(key_id: &str, _use_keyring: bool) -> Result<()> {
        memory::revoke(key_id)
    }
}

impl Deref for SecretKey {
    type Target = SecretBox<[u8]>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use secrecy::ExposeSecret;

    use crate::keepass::key::{memory, SecretKey};

    #[test]
    fn key_roundtrip() {
        let mut key = SecretKey::new("some random string !@(as+=!#@_%$".to_string().into_bytes().into_boxed_slice());
        key.store(Duration::from_secs(10), false).unwrap();
        let data = SecretKey::retrieve(&key.key_id, Duration::from_secs(10), false).unwrap();

        assert_eq!(key.expose_secret(), data.expose_secret());
    }

    // regression: the store must not assume a fixed secret length
    #[test]
    fn key_roundtrip_other_lengths() {
        for secret in [&b"short"[..], &[7u8; 64][..]] {
            let mut key = SecretKey::new(secret.to_vec().into_boxed_slice());
            key.store(Duration::from_secs(10), false).unwrap();
            let data = SecretKey::retrieve(&key.key_id, Duration::from_secs(10), false).unwrap();

            assert_eq!(key.expose_secret(), data.expose_secret());
        }
    }

    #[test]
    fn memory_store_roundtrip_and_revoke() {
        let secret = b"0123456789abcdef0123456789abcdef";

        memory::store("test-key", secret, Duration::from_secs(10)).unwrap();
        let data = memory::retrieve("test-key", Duration::from_secs(10)).unwrap();
        assert_eq!(data.as_ref(), secret);

        memory::revoke("test-key").unwrap();
        assert!(memory::retrieve("test-key", Duration::from_secs(10)).is_err());
    }

    #[test]
    fn memory_store_expires() {
        let secret = b"0123456789abcdef0123456789abcdef";

        memory::store("expiring-key", secret, Duration::from_secs(0)).unwrap();
        assert!(memory::retrieve("expiring-key", Duration::from_secs(10)).is_err());
    }
}
