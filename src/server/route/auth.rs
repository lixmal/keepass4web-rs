use actix_session::Session;
use actix_web::{get, HttpRequest, HttpResponse, post, Responder, web};
use actix_web::web::Data;
use log::{error, info};
use mime::TEXT_HTML;
use serde::Serialize;
use serde_json::json;

use crate::{auth_backend, db_backend};
use crate::auth::{BackendLogin, DbLogin, gen_token, SESSION_KEY_USER, UserLogin};
use crate::auth_backend::{AuthCache, is_invalid_credentials, SESSION_KEY_AUTH_STATE, UserInfo};
use crate::config::config::Config;
use crate::keepass::db_cache::DbCache;
use crate::keepass::keepass::KeePass;
use crate::rate_limit::RateLimiter;
use crate::server::route::INDEX_FILE;
use crate::server::server::CSP;
use crate::server::route::util::{_close_db, check_user_session, db_is_open, get_db, revoke_key, set_user_session, store_key, SESSION_KEY_USED_KEYFILE};
use crate::session::AuthSession;

// a fresh value per response, so the policy names this page's script and no
// other
const NONCE_LENGTH: usize = 32;

#[derive(Serialize)]
struct Settings {
    cn: String,
    timeout: u64,
    interval: u64,
}

#[derive(Serialize)]
struct SessionData {
    csrf_token: String,
    settings: Settings,
}

#[get("/authenticated")]
async fn authenticated(session: Session, config: Data<Config>, db_cache: Data<DbCache>) -> impl Responder {
    let backend = db_backend::new(&config).authenticated();

    let db = match db_is_open(&session, &config, &db_cache).await {
        Ok(v) => v,
        Err(err) => return err,
    };

    let resp = json!({
        "success": false,
        "data": {
            "backend": backend,
            "db": db,
            // so the save form knows to ask for the key file again
            "used_keyfile": session.get::<bool>(SESSION_KEY_USED_KEYFILE).ok().flatten().unwrap_or(false),
        },
    });

    if backend && db {
        return HttpResponse::Ok().json(resp);
    }
    HttpResponse::Unauthorized().json(resp)
}


// forwarding headers are spoofable, only honor them when explicitly
// configured (i.e. behind a reverse proxy that sets them)
fn client_ip(request: &HttpRequest, config: &Config) -> String {
    if config.trust_proxy_headers {
        return request.connection_info().realip_remote_addr().unwrap_or("unknown").to_string();
    }

    request.peer_addr().map(|addr| addr.ip().to_string()).unwrap_or_else(|| "unknown".to_string())
}

#[post("/user_login")]
async fn user_login(request: HttpRequest, session: Session, config: Data<Config>, rate_limiter: Data<RateLimiter>, params: web::Form<UserLogin>) -> impl Responder {
    if let Err(err) = check_user_session(&session, &params.username) {
        return err;
    }

    let client_ip = client_ip(&request, &config);
    let rate_key = format!("{}|{}", client_ip, params.username.to_lowercase());
    if let Some(remaining) = rate_limiter.check(&rate_key).await {
        info!("user login from '{}' ({}): rate limited for {}s", params.username, client_ip, remaining.as_secs());
        return HttpResponse::TooManyRequests().json(json!(
            {
                "success": false,
                "message": "too many failed login attempts, try again later",
            }
        ));
    }

    let auth_backend = auth_backend::new(&config);
    let user_info = match auth_backend.login(params.username.as_str(), params.password.as_str()).await {
        Ok(user_info) => user_info,
        // a backend that could not answer is not a wrong password: it is logged
        // as the fault it is, it does not count towards the attempts that lock
        // an account out, and the user is not sent to check their password
        Err(err) if !is_invalid_credentials(&err) => {
            error!("user login from '{}': the auth backend failed: {:#}", params.username, err);
            return HttpResponse::ServiceUnavailable().json(json!(
                {
                    "success": false,
                    "message": "the authentication backend is unavailable",
                }
            ));
        }
        Err(err) => {
            rate_limiter.failure(&rate_key).await;
            info!("user login from '{}': {}", params.username, err);
            return HttpResponse::Unauthorized().json(json!(
                {
                    "success": false,
                    "message": "username or password incorrect",
                }
            ));
        }
    };
    rate_limiter.success(&rate_key).await;

    let csrf_token = match set_user_session(session, &user_info) {
        Ok(v) => v,
        Err(err) => return HttpResponse::InternalServerError().json(json!(
            {
                "success": false,
                "message": err.to_string(),
            }
        )),
    };


    info!("user login from '{}': successful", params.username);
    HttpResponse::Ok().json(json!(
        {
            "success": true,
            "data": SessionData {
                csrf_token,
                settings: Settings {
                    cn: user_info.name,
                    timeout: config.db_session_timeout.as_secs(),
                    interval: config.auth_check_interval.as_secs(),
                }
            }
        }
    ))
}


#[post("/backend_login")]
async fn backend_login(session: Session, config: Data<Config>, params: web::Form<BackendLogin>) -> impl Responder {
    let username = session.get_user_id();

    let db_backend = db_backend::new(&config);
    if db_backend.authenticated() {
        return HttpResponse::BadRequest().json(json!(
            {
                "success": false,
                "message": "already logged into backend",
            }
        ));
    }

    if let Err(err) = db_backend.init(params) {
        info!("backend login from '{}': {}", username, err);
        return HttpResponse::Unauthorized().json(json!(
            {
                "success": false,
                "message": "backend initialization failed",
            }
        ));
    };


    info!("backend login from '{}': successful", username);
    HttpResponse::Ok().json(json!(
        {
            "success": true,
        }
    ))
}

#[post("/db_login")]
async fn db_login(request: HttpRequest, session: Session, config: Data<Config>, db_cache: Data<DbCache>, rate_limiter: Data<RateLimiter>, params: web::Form<DbLogin>) -> impl Responder {
    let username = session.get_user_id();

    // the master password guards the database itself, so it is throttled like
    // the user login, keyed by the user it belongs to
    let rate_key = format!("{}|db|{}", client_ip(&request, &config), username);
    if let Some(remaining) = rate_limiter.check(&rate_key).await {
        info!("db login from '{}': rate limited for {}s", username, remaining.as_secs());
        return HttpResponse::TooManyRequests().json(json!(
            {
                "success": false,
                "message": "too many failed login attempts, try again later",
            }
        ));
    }

    let is_open = match db_is_open(&session, &config, &db_cache).await {
        Ok(v) => v,
        Err(err) => return err,
    };

    if is_open {
        return HttpResponse::BadRequest().json(json!(
            {
                "success": false,
                "message": "database already open",
            }
        ));
    }

    let user_info = match get_user_info(&session) {
        Ok(v) => v,
        Err(err) => return err,
    };

    let mut db_backend = db_backend::new(&config);
    let db = match KeePass::from_backend(&config, db_backend.as_mut(), &params, &user_info).await {
        Ok(v) => v,
        Err(err) => {
            rate_limiter.failure(&rate_key).await;
            info!("db login from '{}': {}", username, err);

            return HttpResponse::Unauthorized().json(json!(
                {
                    "success": false,
                    "message": "keepass db initialization failed",
                }
            ));
        }
    };

    rate_limiter.success(&rate_key).await;

    // Remembered so the client can ask for the key file again when the
    // database is saved: the same credentials have to be presented, and a save
    // form that only asked for a password would be a dead end for anyone whose
    // database needs a file as well. The file itself is not kept.
    if let Err(err) = session.insert(SESSION_KEY_USED_KEYFILE, params.key.is_some()) {
        error!("db login from '{}': failed to record the key file use: {}", username, err);
        return HttpResponse::InternalServerError().json(json!(
            {
                "success": false,
                "message": "failed to store session",
            }
        ));
    }

    let (key, enc_db) = match db.to_enc() {
        Ok(v) => v,
        Err(err) => {
            error!("db login from '{}': {}", username, err);

            return HttpResponse::InternalServerError().json(json!(
                {
                    "success": false,
                    "message": "failed to encrypt database",
                }
            ));
        }
    };

    if let Err(err) = store_key(&config, &session, key) {
        error!("db login from '{}': failed to store key: {}", username, err);
        return HttpResponse::InternalServerError().json(json!(
            {
                "success": false,
                "message": "failed to store key",
            }
        ));
    }

    if let Err(err) = db_cache.store(&session, enc_db).await {
        error!("db login from '{}': failed to store db: {}", username, err);
        if let Err(err) = revoke_key(&config, &session) {
            error!("db login from '{}': failed to revoke db key: {}", username, err);
        }
        return HttpResponse::InternalServerError().json(json!(
            {
                "success": false,
                "message": "failed to store db",
            }
        ));
    }

    info!("db login from '{}': successful", username);
    HttpResponse::Ok().json(json!(
        {
            "success": true,
        }
    ))
}

fn get_user_info(session: &Session) -> Result<UserInfo, HttpResponse> {
    let resp = HttpResponse::InternalServerError().json(json!(
        {
            "success": false,
            "message": "failed to retrieve session",
        }
    ));
    let user_info = match session.get::<UserInfo>(SESSION_KEY_USER) {
        Err(err) => {
            error!("failed to retrieve session: {}", err);
            return Err(resp);
        }
        Ok(Some(v)) => v,
        Ok(None) => return Err(resp),
    };
    Ok(user_info)
}

#[post("/close_db")]
async fn close_db(session: Session, config: Data<Config>, db_cache: Data<DbCache>) -> impl Responder {
    // a modification that is halfway through would otherwise store the database
    // and its key again after this cleared them, leaving it open
    let _guard = match db_cache.mutation_guard(&session).await {
        Ok(guard) => guard,
        Err(err) => {
            error!("close db from '{}': {}", session.get_user_id(), err);
            return HttpResponse::InternalServerError().json(json!({
                "success": false,
                "message": "failed to close db",
            }));
        }
    };

    if let Err(err) = _close_db(&session, &config, &db_cache).await {
        return err;
    }

    info!("close db from '{}': successful", session.get_user_id());
    HttpResponse::Ok().json(json!(
        {
            "success": true,
        }
    ))
}

#[post("/logout")]
async fn logout(request: HttpRequest, session: Session, config: Data<Config>, db_cache: Data<DbCache>, auth_cache: Data<AuthCache>) -> impl Responder {
    let user_info = match get_user_info(&session) {
        Ok(v) => v,
        Err(err) => return err,
    };

    let host = format!("{}://{}", request.connection_info().scheme(), request.connection_info().host());
    let logout_type = match auth_backend::new(&config).get_logout_type(&user_info, &host, &auth_cache).await {
        Ok(logout_type) => logout_type,
        Err(err) => {
            error!("failed to determine logout type: {}", err);
            return HttpResponse::InternalServerError().json(json!(
               {
                   "success": false,
                   "message": "failed to retrieve logout type/url",
               }
            ));
        }
    };

    // best effort, key expires anyway, but still after any modification in
    // flight rather than in the middle of one
    {
        let _guard = db_cache.mutation_guard(&session).await;
        let _ = _close_db(&session, &config, &db_cache).await;
    }

    session.destroy();

    let username = session.get_user_id();
    info!("logout from '{}': successful", username);

    HttpResponse::Ok().json(json!(
        {
            "success": true,
            "data": logout_type,
        }
    ))
}

#[post("/save_db")]
async fn save_db(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Form<DbLogin>) -> impl Responder {
    let username = session.get_user_id();

    let keepass = match get_db(&session, &config, &db_cache).await {
        Ok(v) => v,
        Err(err) => return err,
    };

    let user_info = match get_user_info(&session) {
        Ok(v) => v,
        Err(err) => return err,
    };

    let mut db_backend = db_backend::new(&config);
    let db_key = match KeePass::db_key_from_params_pub(db_backend.as_ref(), &params, &user_info).await {
        Ok(k) => k,
        Err(err) => {
            info!("save_db from '{}': {}", username, err);
            return HttpResponse::Unauthorized().json(json!({
                "success": false,
                "message": "incorrect key",
            }));
        }
    };

    // The save re-encrypts the database with whatever was sent, so credentials
    // that do not open the stored database would quietly replace the ones that
    // do: saving with only a password would drop the key file the database
    // needs, leaving it open to the password alone and locking out the person
    // who still presents the key file. Opening it with them first is what says
    // they are the same credentials.
    if let Err(err) = KeePass::key_opens_stored(db_backend.as_ref(), &params, &user_info).await {
        info!("save_db from '{}': the credentials do not open the stored database: {}", username, err);
        return HttpResponse::Unauthorized().json(json!({
            "success": false,
            "message": "the master password and key file must be the ones the database was opened with",
        }));
    }

    if let Err(err) = keepass.to_backend_with_key(db_backend.as_mut(), db_key, &user_info).await {
        error!("save_db from '{}': {}", username, err);
        return HttpResponse::InternalServerError().json(json!({
            "success": false,
            "message": "failed to save database",
        }));
    }

    info!("save_db from '{}': successful", username);
    HttpResponse::Ok().json(json!({ "success": true }))
}

#[get("/callback_user_auth")]
async fn callback_user_auth(
    request: HttpRequest,
    session: Session,
    config: Data<Config>,
    auth_cache: Data<AuthCache>,
    params: web::Query<serde_json::Value>,
) -> impl Responder {
    let username = session.get_user_id();

    if let Err(err) = check_user_session(&session, &username) {
        return err;
    }

    let from_session = match session.get_key(SESSION_KEY_AUTH_STATE) {
        Some(v) => v,
        None => {
            session.destroy();
            return embed_in_index(false, Some("failed to retrieve session auth state".to_string()), None).await;
        }
    };
    session.remove(SESSION_KEY_AUTH_STATE);

    let host = format!("{}://{}", request.connection_info().scheme(), request.connection_info().host());
    let user_info = match auth_backend::new(&config).callback(from_session, &auth_cache, params.0, &host).await {
        Ok(user_info) => user_info,
        Err(err) => {
            info!("user login from '{}': {:?}", username, err);
            session.destroy();
            return embed_in_index(false, Some(err.to_string()), None).await;
        }
    };

    let csrf_token = match set_user_session(session, &user_info) {
        Err(err) => return embed_in_index(false, Some(err.to_string()), None).await,
        Ok(v) => v,
    };

    info!("user login from '{}': successful", &user_info.id);

    embed_in_index(true, None, Some(
        SessionData {
            csrf_token,
            settings: Settings {
                cn: user_info.name,
                timeout: config.db_session_timeout.as_secs(),
                interval: config.auth_check_interval.as_secs(),
            },
        }
    )).await
}

// TODO: fix this:w
async fn embed_in_index(success: bool, message: Option<String>, data: Option<SessionData>) -> HttpResponse {
    let mut index = match tokio::fs::read_to_string(INDEX_FILE).await {
        Ok(v) => v,
        Err(err) => {
            error!("failed to read index file: {}", err);
            return HttpResponse::InternalServerError().json(json!(
                {
                    "success": false,
                    "message": "failed to read index file",
                }
            ));
        }
    };

    let payload = script_safe_json(&json!({
       "success": success,
       "message": message,
       "data": data,
    }));

    // this page carries the one inline script in the app, so it names it in
    // its own policy rather than the whole app having to allow inline scripts
    let nonce = gen_token(NONCE_LENGTH);

    index = index.replace("</head>", format!(r#"
        <script nonce="{}">
            window.KeePass4WebResponse = {}
        </script>
        </head>
    "#, nonce, payload).as_str());

    HttpResponse::Ok()
        .content_type(TEXT_HTML)
        .append_header((
            "Content-Security-Policy",
            CSP.replace("script-src 'self'", &format!("script-src 'self' 'nonce-{}'", nonce)),
        ))
        .body(index)
}

/// JSON that is safe to write inside a `<script>` block.
///
/// The values here carry things the server did not choose: a display name from
/// the directory, an error naming what a provider sent back. JSON escaping
/// leaves `<` and `/` alone, so a name containing `</script>` would end the
/// script block and whatever followed would run as this origin. Escaping them
/// as `\uXXXX` keeps the same string while leaving nothing for the HTML parser
/// to act on. U+2028 and U+2029 go too: they are line terminators to a
/// JavaScript parser and would break the literal.
fn script_safe_json(value: &serde_json::Value) -> String {
    value.to_string()
        .replace('<', "\\u003c")
        .replace('>', "\\u003e")
        .replace('&', "\\u0026")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

#[cfg(test)]
mod tests {
    use super::*;

    // A display name comes from the directory or the identity provider, not
    // from us. Written into the page as it stands, one containing a closing
    // script tag would end the block and run as this origin, which in a
    // password manager means the session and everything it can read.
    #[test]
    fn a_name_cannot_end_the_script_block() {
        let hostile = "x</script><script>alert(1)</script>";
        let payload = script_safe_json(&json!({ "data": { "cn": hostile } }));

        assert!(!payload.contains("</script>"), "{}", payload);
        assert!(!payload.contains('<'), "{}", payload);
        assert!(!payload.contains('>'), "{}", payload);
    }

    // and it still means the same thing once the browser reads it back
    #[test]
    fn the_escaped_payload_still_carries_the_value() {
        let name = "Ada <Lovelace> & co";
        let payload = script_safe_json(&json!({ "cn": name }));

        let back: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(back["cn"], name);
    }

    #[test]
    fn javascript_line_terminators_are_escaped() {
        let payload = script_safe_json(&json!({ "message": "a\u{2028}b\u{2029}c" }));

        assert!(!payload.contains('\u{2028}'), "{}", payload);
        assert!(!payload.contains('\u{2029}'), "{}", payload);

        let back: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(back["message"], "a\u{2028}b\u{2029}c");
    }
}
