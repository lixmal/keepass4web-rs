use std::ops::Deref;
use std::time::Duration;

use anyhow::Result;
use linux_keyutils::{Key, KeyPermissions, KeyRing, KeyRingIdentifier};
use secrecy::{ExposeSecret, SecretBox};

use crate::auth::gen_token;

pub struct SecretKey {
    pub key_id: KeyId,
    data: SecretBox<[u8]>,
    timeout: Duration,
}

pub type KeyId = String;


const ID_LENGTH: usize = 16;
const KEYRING_PERM: u32 = 0x3f000000;


impl SecretKey {
    pub fn new(secret: Box<[u8]>) -> Self {
        Self {
            key_id: gen_token(ID_LENGTH),
            data: SecretBox::new(secret),
            timeout: Duration::default(),
        }
    }

    pub fn retrieve(key_id: &KeyId, timeout: Duration) -> Result<Self> {
        let keyr = get_keyring()?;

        let mut k = keyr.search(key_id.as_str())?;

        // TODO: determine buffer size
        let mut data = vec![0; 32].into_boxed_slice();
        k.read(&mut data)?;

        let key = Self {
            key_id: key_id.clone(),
            data: SecretBox::new(data),
            timeout,
        };

        key.update_timeout(&mut k, timeout)?;

        Ok(key)
    }

    pub fn store(&mut self, timeout: Duration) -> Result<&mut Self> {
        let keyr = get_keyring()?;

        let key = keyr.add_key(self.key_id.as_str(), self.expose_secret())?;
        key.set_timeout(timeout.as_secs() as usize)?;
        key.set_perms(KeyPermissions::from_u32(KEYRING_PERM))?;

        self.timeout = timeout;
        Ok(self)
    }

    // retrieve will fail after revoke
    pub fn revoke(&mut self) -> Result<&mut Self> {
        let keyr = get_keyring()?;

        let k = keyr.search(self.key_id.as_str())?;

        k.revoke()?;

        Ok(self)
    }

    fn update_timeout(&self, key: &mut Key, timeout: Duration) -> Result<()> {
        Ok(key.set_timeout(timeout.as_secs() as usize)?)
    }
}

impl Deref for SecretKey {
    type Target = SecretBox<[u8]>;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

fn get_keyring() -> Result<KeyRing> {
    // TODO: Make other keyrings available
    // TODO: Investigate why key in Process keyring doesn't persist
    Ok(KeyRing::from_special_id(KeyRingIdentifier::Session, false)?)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use secrecy::ExposeSecret;

    use crate::keepass::key::SecretKey;

    #[test]
    fn key_roundtrip() {
        let mut key = SecretKey::new("some random string !@(as+=!#@_%$".to_string().into_bytes().into_boxed_slice());
        key.store(Duration::from_secs(10)).unwrap();
        let data = SecretKey::retrieve(&key.key_id, Duration::from_secs(10)).unwrap();

        assert_eq!(key.expose_secret(), data.expose_secret());
    }
}
