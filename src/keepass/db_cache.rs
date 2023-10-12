use std::collections::HashMap;
use std::ops::Deref;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

use actix_session::Session;
use anyhow::anyhow;

use crate::auth::SESSION_KEY_USER;
use crate::keepass::encrypted::Encrypted;

use anyhow::Result;

const UPDATE_THRESHOLD: Duration = Duration::from_secs(1);


#[derive(Default)]
pub struct DbCache {
    lock: RwLock<HashMap<String, Encrypted>>,
}

impl Deref for DbCache {
    type Target = RwLock<HashMap<String, Encrypted>>;

    fn deref(&self) -> &Self::Target {
        &self.lock
    }
}

impl DbCache {
    pub async fn store(&self, session: &Session, enc_db: Encrypted) -> Result<()> {
        self.write().await
            .insert(
                self.get_user(session)?, enc_db,
            );

        Ok(())
    }

    pub async fn retrieve(&self, session: &Session, timeout: Duration) -> Result<Encrypted> {
        let mut enc = self.read().await.get(
            self.get_user(session)?.as_str()
        ).ok_or(anyhow!("enc db not found in store"))?.clone();

        // Don't update expiry if there are many requests in succession
        if Instant::now() + timeout - enc.expiry > UPDATE_THRESHOLD {
            enc.update_expiry(timeout);
            self.store(session, enc.clone()).await?;
        }

        Ok(enc)
    }

    pub async fn clear(&self, session: &Session) -> Result<()> {
        self.write().await
            .remove(
                self.get_user(session)?.as_str()
            );

        Ok(())
    }

    fn get_user(&self, session: &Session) -> Result<String> {
        session.get::<String>(SESSION_KEY_USER)?.ok_or(anyhow!("unable to retrieve user from session"))
    }
}
