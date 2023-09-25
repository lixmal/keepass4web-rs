use actix_session::Session;
use actix_web::{get, HttpResponse, post, Responder, web};
use actix_web::web::Data;
use log::{error, info};
use serde_json::json;

use crate::{auth_backend, db_backend};
use crate::auth::{BackendLogin, DbLogin, gen_token, SESSION_KEY_CSRF, SESSION_KEY_USER, UserLogin};
use crate::config::config::Config;
use crate::keepass::db_cache::DbCache;
use crate::keepass::keepass::KeePass;
use crate::server::route::util::{_close_db, db_is_open, revoke_key, store_key};
use crate::session::AuthSession;

const CSRF_TOKEN_LENGTH: usize = 32;

#[get("/authenticated")]
async fn authenticated(session: Session, config: Data<Config>, db_cache: Data<DbCache>) -> impl Responder {
    let backend = db_backend::new(&config, &session).authenticated();

    let db = match db_is_open(&session, &config, &db_cache).await {
        Ok(v) => v,
        Err(err) => return err,
    };

    let resp = json!({
        "success": false,
        "data": {
            "backend": backend,
            "db": db,
        },
    });

    if backend && db {
        return HttpResponse::Ok().json(resp);
    }
    HttpResponse::Unauthorized().json(resp)
}


#[post("/user_login")]
async fn user_login(session: Session, config: Data<Config>, params: web::Form<UserLogin>) -> impl Responder {
    // strictly check if session is available, the session backend might be down
    let session_user = match session.get::<String>(SESSION_KEY_USER) {
        Ok(s) => s,
        Err(err) => {
            error!("user login from '{}': {}", params.username, err);
            return HttpResponse::InternalServerError().json(json!(
                {
                    "success": false,
                    "message": "failed to retrieve session",
                }
            ));
        }
    };

    if session_user.is_some() {
        info!("user login from '{}': already logged in", params.username);
        return HttpResponse::BadRequest().json(json!(
            {
                "success": false,
                "message": "already logged in",
            }
        ));
    }

    let auth_backend = auth_backend::new(&config);
    let user_info = match auth_backend.login(params.username.as_str(), params.password.as_str()) {
        Ok(user_info) => user_info,
        Err(err) => {
            info!("user login from '{}': {}", params.username, err);
            return HttpResponse::Unauthorized().json(json!(
                {
                    "success": false,
                    "message": "username or password incorrect",
                }
            ));
        }
    };

    if let Err(err) = session.insert(SESSION_KEY_USER, user_info.name.as_str()) {
        error!("user login from '{}': {}", params.username, err);
        return HttpResponse::InternalServerError().json(json!(
            {
                "success": false,
                "message": "failed to set user session",
            }
        ));
    };

    let csrf_token = gen_token(CSRF_TOKEN_LENGTH);
    if let Err(err) = session.insert(SESSION_KEY_CSRF, csrf_token.as_str()) {
        session.destroy();
        error!("user login from '{}': {}", params.username, err);
        return HttpResponse::InternalServerError().json(json!(
            {
                "success": false,
                "message": "failed to set session csrf token",
            }
        ));
    };


    info!("user login from '{}': successful", params.username);
    HttpResponse::Ok().json(json!(
        {
            "success": true,
            "data": {
                "csrf_token": csrf_token,
                "settings": {
                    "cn": user_info.name,
                    "template": {},
                    "timeout": config.db_session_timeout.as_secs(),
                    "interval": config.auth_check_interval.as_secs(),
                }
            },
        }
    ))
}

#[post("/backend_login")]
async fn backend_login(session: Session, config: Data<Config>, params: web::Form<BackendLogin>) -> impl Responder {
    let username = session.get_username();

    let db_backend = db_backend::new(&config, &session);
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
async fn db_login(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Form<DbLogin>) -> impl Responder {
    let username = session.get_username();

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

    let db_backend = db_backend::new(&config, &session);
    let db = match KeePass::from_backend(&config, &db_backend, &params) {
        Ok(v) => v,
        Err(err) => {
            info!("db login from '{}': {}", username, err);

            return HttpResponse::Unauthorized().json(json!(
                {
                    "success": false,
                    "message": "keepass db initialization failed",
                }
            ));
        }
    };

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
                "success": true,
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
                "success": true,
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

#[post("/close_db")]
async fn close_db(session: Session, config: Data<Config>, db_cache: Data<DbCache>) -> impl Responder {
    if let Err(err) = _close_db(&session, &config, &db_cache).await {
        return err;
    }

    info!("close db from '{}': successful", session.get_username());
    HttpResponse::Ok().json(json!(
        {
            "success": true,
        }
    ))
}

#[post("/logout")]
async fn logout(session: Session, config: Data<Config>, db_cache: Data<DbCache>) -> impl Responder {
    let u = match session.get::<String>(SESSION_KEY_USER) {
        Ok(username) => username,
        Err(err) => {
            error!("failed to retrieve session: {}", err);
            return HttpResponse::InternalServerError().json(json!(
                {
                    "success": true,
                    "message": "failed to retrieve session",
                }
            ));
        }
    };


    // best effort, key expires anyway
    let _ = _close_db(&session, &config, &db_cache).await;

    session.destroy();

    let username = session.get_username();
    info!("logout from '{}': successful", username);
    HttpResponse::Ok().json(json!(
        {
            "success": true,
        }
    ))
}

