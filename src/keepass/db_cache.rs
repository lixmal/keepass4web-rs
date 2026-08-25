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
use crate::auth_backend::UserInfo;
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
        let user = self.get_user(session)?;
        let mut cache = self.write().await;

        // evict expired entries opportunistically, so the cache
        // doesn't grow unboundedly with inactive users
        let now = Instant::now();
        cache.retain(|_, enc| now < enc.expiry);

        cache.insert(user, enc_db);

        Ok(())
    }

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
        Ok(
            session.get::<UserInfo>(SESSION_KEY_USER)?.ok_or(anyhow!("unable to retrieve user from session"))?.id
        )
    }
}

#[cfg(test)]
mod tests {
    use actix_session::SessionExt;
    use actix_web::test::TestRequest;

    use super::*;

    #[tokio::test]
    async fn expired_entries_are_evicted_on_store() {
        let cache = DbCache::default();
        let req = TestRequest::default().to_http_request();
        let session = req.get_session();
        session.insert(SESSION_KEY_USER, UserInfo {
            id: "alice".to_string(),
            ..Default::default()
        }).unwrap();

        // expired entry of an inactive user
        let (_, expired) = Encrypted::encrypt(vec![1, 2, 3], &[], Duration::from_secs(0)).unwrap();
        cache.write().await.insert("bob".to_string(), expired);

        let (_, fresh) = Encrypted::encrypt(vec![4, 5, 6], &[], Duration::from_secs(60)).unwrap();
        cache.store(&session, fresh).await.unwrap();

        let entries = cache.read().await;
        assert!(entries.contains_key("alice"));
        assert!(!entries.contains_key("bob"));
    }
}
