use actix_session::Session;
use actix_web::HttpResponse;
use anyhow::bail;
use log::{debug, error, info};
use serde_json::json;

use crate::auth::{gen_token, SESSION_KEY_CSRF, SESSION_KEY_USER};
use crate::auth_backend::UserInfo;
use crate::config::config::Config;
use crate::keepass::db_cache::{CacheExpiredError, DbCache, NotOpenError};
use crate::keepass::keepass::KeePass;
use crate::keepass::key::{KeyId, KeyUnavailableError, SecretKey};
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
    // rotate the session on privilege change to prevent session fixation
    session.renew();

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
            "success": false,
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
            // a database that is not open yet, or has expired, is the ordinary
            // state of a session before the master password is entered; only a
            // cache that misbehaved is worth an error in the log
            let closed = err.is::<NotOpenError>() || err.is::<CacheExpiredError>();
            if closed {
                debug!("db not open: {}", err);
            } else {
                error!("failed to retrieve db: {}", err);
            }

            let resp = json!(
                {
                    "success": false,
                    "message": "failed to retrieve db from cache",
                }
            );
            return if closed {
                _close_db(session, config, db_cache).await?;

                Err(HttpResponse::Unauthorized().json(resp))
            } else {
                Err(HttpResponse::InternalServerError().json(resp))
            };
        }
    };

    let key = match retrieve_key(config, session) {
        Ok(k) => k,
        Err(err) => {
            // same again: a key that was never stored, or has expired, means
            // the database is closed rather than that something went wrong
            let closed = err.is::<KeyUnavailableError>();
            if closed {
                debug!("key not available: {}", err);
            } else {
                error!("failed to retrieve key: {}", err);
            }

            let resp = json!(
                {
                    "success": false,
                    "message": "failed to retrieve key",
                }
            );

            return if closed {
                _close_db(session, config, db_cache).await?;

                Err(HttpResponse::Unauthorized().json(resp))
            } else {
                Err(HttpResponse::InternalServerError().json(resp))
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

/// Decrypt the cached database, run `modify`, then re-encrypt and store back.
/// The old session key is revoked and replaced with a fresh one.
pub(crate) async fn modify_db<F>(
    session: &Session,
    config: &Config,
    db_cache: &DbCache,
    modify: F,
) -> Result<(), HttpResponse>
where
    F: FnOnce(&mut KeePass) -> anyhow::Result<()>,
{
    // held until this modification has been stored again, so a second one
    // cannot start from the copy this one is about to replace
    let _guard = match db_cache.mutation_guard(session).await {
        Ok(guard) => guard,
        Err(err) => {
            error!("failed to take the database lock: {}", err);
            return Err(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "failed to modify database",
            })));
        }
    };

    let mut keepass = get_db(session, config, db_cache).await?;

    if let Err(err) = modify(&mut keepass) {
        return Err(HttpResponse::UnprocessableEntity().json(json!({
            "success": false,
            "message": err.to_string(),
        })));
    }

    let (new_key, new_enc) = match keepass.to_enc() {
        Ok(v) => v,
        Err(err) => {
            error!("failed to re-encrypt database after modification: {}", err);
            return Err(HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "failed to encrypt database",
            })));
        }
    };

    // Revoke the old key before storing the new one so it is replaced atomically
    // from the session's perspective (the old key_id is still in the session here).
    let _ = revoke_key(config, session);

    if let Err(err) = store_key(config, session, new_key) {
        error!("failed to store new key after modification: {}", err);
        return Err(HttpResponse::InternalServerError().json(json!({
            "success": false,
            "message": "failed to store key",
        })));
    }

    if let Err(err) = db_cache.store(session, new_enc).await {
        error!("failed to store modified database: {}", err);
        return Err(HttpResponse::InternalServerError().json(json!({
            "success": false,
            "message": "failed to store database",
        })));
    }

    Ok(())
}

pub(crate) fn retrieve_key(config: &Config, session: &Session) -> anyhow::Result<SecretKey> {
    // no key id in the session is a key that is missing, not a fault: the
    // session was never unlocked, or has already been closed
    let key_id = session.get::<KeyId>(SESSION_KEY_KEY_ID)?
        .ok_or(KeyUnavailableError)?;

    SecretKey::retrieve(&key_id, config.db_session_timeout, config.use_keyring)
}

pub(crate) fn store_key(config: &Config, session: &Session, mut key: SecretKey) -> anyhow::Result<()> {
    key.store(config.db_session_timeout, config.use_keyring)?;
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
    // Ignore non-existent, expired or already revoked
    match err.downcast_ref::<KeyUnavailableError>() {
        Some(_) => ok(),
        None => Err(err),
    }
}
