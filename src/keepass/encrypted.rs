use std::time::{Duration, Instant};

use aes_gcm::{AeadCore, AeadInPlace, Aes256Gcm, Key, KeyInit, Nonce};
use anyhow::Result;
use rand::thread_rng;
use secrecy::{ExposeSecret, SecretVec};
use serde::{Deserialize, Serialize};
use zeroize::Zeroize;

use crate::keepass::key::SecretKey;

const LENGTH_IV: usize = 16;

#[derive(Clone, Serialize, Deserialize)]
pub struct Encrypted {
    data: Vec<u8>,
    iv: Box<[u8]>,
    #[serde(with = "serde_millis")]
    pub expiry: Instant,
}

impl Encrypted {
    pub fn encrypt(mut input: Vec<u8>, aad: &[u8], timeout: Duration) -> Result<(SecretKey, Encrypted)> {
        input.extend([0; LENGTH_IV]);
        let iv = Aes256Gcm::generate_nonce(&mut thread_rng()); // 96-bits; unique per message
        let mut enc = Encrypted {
            data: input,
            iv: iv.to_vec().into_boxed_slice(),
            expiry: Instant::now() + timeout,
        };

        let key = Aes256Gcm::generate_key(&mut thread_rng());

        let cipher = Aes256Gcm::new(&key);
        cipher.encrypt_in_place(&iv, aad, &mut enc.data)?;

        Ok((SecretKey::new(key.to_vec().into_boxed_slice()), enc))
    }

    pub fn decrypt(mut self, key: SecretKey, aad: &[u8]) -> Result<SecretVec<u8>> {
        let key = Key::<Aes256Gcm>::from_slice(key.expose_secret());
        let cipher = Aes256Gcm::new(&key);
        cipher.decrypt_in_place(Nonce::from_slice(&self.iv), aad, &mut self.data)?;

        let v = self.data[0..self.data.len() - LENGTH_IV].to_vec();
        self.data.zeroize();
        Ok(SecretVec::new(v))
    }

    pub fn update_expiry(&mut self, timeout: Duration) -> &mut Self {
        self.expiry = Instant::now() + timeout;

        self
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use secrecy::ExposeSecret;

    use crate::keepass::encrypted::Encrypted;

    #[test]
    fn enc_roundtrip() {
        let data = "some random string !@(as+=!#@_%$".to_string();

        let (key, enc) = Encrypted::encrypt(data.clone().into_bytes(), &vec![1, 2, 3], Duration::from_secs(10)).unwrap();
        let dec_data = enc.decrypt(key, &vec![1, 2, 3]).unwrap();
        let a = dec_data.expose_secret().to_vec();

        assert_eq!(data.as_bytes().as_ref(), dec_data.expose_secret());
    }
}
