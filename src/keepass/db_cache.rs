use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Debug, Display, Formatter};
use std::ops::Deref;
use std::time::{Duration, Instant};

use actix_session::Session;
use anyhow::anyhow;
use anyhow::Result;
use log::info;
use tokio::sync::RwLock;

use crate::auth::SESSION_KEY_USER;
use crate::keepass::encrypted::Encrypted;

const UPDATE_THRESHOLD: Duration = Duration::from_secs(1);


#[derive(Debug, Clone)]
pub struct CacheExpiredError;

impl Display for CacheExpiredError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "entry expired")
    }
}

impl Error for CacheExpiredError {}

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

    // TODO: expire old entries periodically
    pub async fn retrieve(&self, session: &Session, timeout: Duration) -> Result<Encrypted> {
        let user = self.get_user(session)?;
        let mut enc = self.read().await.get(user.as_str()).ok_or(anyhow!("enc db not found in store"))?.clone();

        if Instant::now() >= enc.expiry {
            info!("database of user '{}' expired", user);
            return Err(CacheExpiredError.into());
        }
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
