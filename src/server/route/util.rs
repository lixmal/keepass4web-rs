use actix_session::Session;
use actix_web::HttpResponse;
use anyhow::{anyhow, bail};
use linux_keyutils::KeyError;
use log::{error, info};
use serde_json::json;

use crate::auth::{gen_token, SESSION_KEY_CSRF, SESSION_KEY_USER};
use crate::auth_backend::UserInfo;
use crate::config::config::Config;
use crate::keepass::db_cache::{CacheExpiredError, DbCache};
use crate::keepass::keepass::KeePass;
use crate::keepass::key::{KeyId, SecretKey};
use crate::session::AuthSession;

pub const SESSION_KEY_KEY_ID: &str = "key_id";

const CSRF_TOKEN_LENGTH: usize = 32;

pub(crate) type CsrfToken = String;

pub(crate) fn check_user_session(session: &Session, username: &str) -> Result<(), HttpResponse> {
    // strictly check if session is available, the session backend might be down
    let session_user = match session.get::<UserInfo>(SESSION_KEY_USER) {
        Ok(s) => s,
        Err(err) => {
            error!("user login from '{}': {}", username, err);
            return Err(HttpResponse::InternalServerError().json(json!(
                {
                    "success": false,
                    "message": "failed to retrieve session",
                }
            )));
        }
    };

    if session_user.is_some() {
        info!("user login from '{}': already logged in", username);
        return Err(HttpResponse::BadRequest().json(json!(
            {
                "success": false,
                "message": "already logged in",
            }
        )));
    }
    Ok(())
}

pub(crate) fn set_user_session(session: Session, user_info: &UserInfo) -> anyhow::Result<CsrfToken> {
    if let Err(err) = session.insert(SESSION_KEY_USER, user_info) {
        session.destroy();
        error!("user login from '{}': {}", user_info.id, err);
        bail!("failed to set user session");
    };

    let csrf_token = gen_token(CSRF_TOKEN_LENGTH);
    if let Err(err) = session.insert(SESSION_KEY_CSRF, csrf_token.as_str()) {
        session.destroy();
        error!("user login from '{}': {}", user_info.id, err);
        bail!("failed to set session csrf token");
    };

    Ok(csrf_token)
}

pub(crate) async fn _close_db(session: &Session, config: &Config, db_cache: &DbCache) -> Result<(), HttpResponse> {
    let err_resp = HttpResponse::InternalServerError().json(json!(
        {
            "success": true,
            "message": "failed to close db",
        }
    ));

    let username = session.get_user_id();

    // This is idempotent and only fails if there is an issue with the cache backend
    if let Err(err) = db_cache.clear(session).await {
        error!("close db from '{}': failed to clear db: {}", username, err);
        return Err(err_resp);
    }

    if let Err(err) = revoke_key(config, session) {
        error!("close db from '{}': failed to revoke key: {}", username, err);
        return Err(err_resp);
    }

    Ok(())
}

pub(crate) async fn get_db(session: &Session, config: &Config, db_cache: &DbCache) -> anyhow::Result<KeePass, HttpResponse> {
    let enc = match db_cache.retrieve(session, config.db_session_timeout).await {
        Ok(v) => v,
        Err(err) => {
            error!("failed to retrieve db: {}", err);

            let resp = json!(
                {
                    "success": false,
                    "message": "failed to retrieve db from cache",
                }
            );
            return match err.downcast_ref::<CacheExpiredError>() {
                Some(_) => {
                    _close_db(session, config, db_cache).await?;

                    Err(HttpResponse::Unauthorized().json(resp))
                }
                None => Err(HttpResponse::InternalServerError().json(resp))
            };
        }
    };

    let key = match retrieve_key(config, session) {
        Ok(k) => k,
        Err(err) => {
            error!("failed to retrieve key: {}", err);

            let resp = json!(
                {
                    "success": false,
                    "message": "failed to retrieve key",
                }
            );

            return match err.downcast_ref::<KeyError>() {
                Some(_) => {
                    _close_db(session, config, db_cache).await?;

                    Err(HttpResponse::Unauthorized().json(resp))
                }
                None => Err(HttpResponse::InternalServerError().json(resp))
            };
        }
    };

    match KeePass::from_enc(config, key, enc) {
        Ok(v) => Ok(v),
        Err(err) => {
            error!("failed to decrypt database: {}", err);
            Err(
                HttpResponse::InternalServerError().json(json!(
                    {
                        "success": false,
                        "message": "failed to decrypt database",
                    }
                ))
            )
        }
    }
}

pub(crate) async fn db_is_open(session: &Session, config: &Config, db_cache: &DbCache) -> anyhow::Result<bool, HttpResponse> {
    // TODO: distinguish real errors from non-existent db/key etc (= actually closed db)
    // The current behavior may suggest that the database is closed, while in reality it could be
    // that the session, db cache or key backend is currently unavailable. But this should be very rare.
    if get_db(session, config, db_cache).await.is_err() {
        let _ = _close_db(session, config, db_cache).await;
        return Ok(false);
    }
    Ok(true)
}

pub(crate) fn retrieve_key(config: &Config, session: &Session) -> anyhow::Result<SecretKey> {
    let key_id = session.get::<KeyId>(SESSION_KEY_KEY_ID)?
        .ok_or(anyhow!("failed to retrieve key id from session"))?;

    SecretKey::retrieve(&key_id, config.db_session_timeout)
}

pub(crate) fn store_key(config: &Config, session: &Session, mut key: SecretKey) -> anyhow::Result<()> {
    key.store(config.db_session_timeout)?;
    session.insert(SESSION_KEY_KEY_ID, key.key_id)?;

    Ok(())
}

pub(crate) fn revoke_key(config: &Config, session: &Session) -> anyhow::Result<()> {
    let ok = || {
        session.remove(SESSION_KEY_KEY_ID);
        Ok(())
    };

    let mut key = match retrieve_key(config, session) {
        Ok(v) => v,
        Err(err) => return check_key_err(ok, err),
    };

    match key.revoke() {
        Ok(_) => ok(),
        Err(err) => check_key_err(ok, err)
    }
}

fn check_key_err<F>(ok: F, err: anyhow::Error) -> anyhow::Result<()>
    where F: Fn() -> anyhow::Result<()>
{
    match err.downcast_ref::<KeyError>() {
        Some(e) => match e {
            // Ignore non-existent, expired or already revoked
            KeyError::KeyDoesNotExist | KeyError::KeyExpired | KeyError::KeyRevoked => ok(),
            _ => Err(err),
        }
        None => Err(err),
    }
}
