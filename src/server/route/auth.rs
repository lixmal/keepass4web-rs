use actix_session::Session;
use actix_web::{get, HttpRequest, HttpResponse, post, Responder, web};
use actix_web::web::Data;
use log::{error, info};
use mime::TEXT_HTML;
use serde::Serialize;
use serde_json::json;

use crate::{auth_backend, db_backend};
use crate::auth::{BackendLogin, DbLogin, SESSION_KEY_USER, UserLogin};
use crate::auth_backend::{AuthCache, SESSION_KEY_AUTH_STATE, UserInfo};
use crate::config::config::Config;
use crate::keepass::db_cache::DbCache;
use crate::keepass::keepass::KeePass;
use crate::server::route::INDEX_FILE;
use crate::server::route::util::{_close_db, check_user_session, db_is_open, revoke_key, set_user_session, store_key};
use crate::session::AuthSession;

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
        },
    });

    if backend && db {
        return HttpResponse::Ok().json(resp);
    }
    HttpResponse::Unauthorized().json(resp)
}


#[post("/user_login")]
async fn user_login(session: Session, config: Data<Config>, params: web::Form<UserLogin>) -> impl Responder {
    if let Err(err) = check_user_session(&session, &params.username) {
        return err;
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
                csrf_token: csrf_token,
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
async fn db_login(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Form<DbLogin>) -> impl Responder {
    let username = session.get_user_id();

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

    let db_backend = db_backend::new(&config);
    let db = match KeePass::from_backend(&config, db_backend.as_ref(), &params) {
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

    info!("close db from '{}': successful", session.get_user_id());
    HttpResponse::Ok().json(json!(
        {
            "success": true,
        }
    ))
}

#[post("/logout")]
async fn logout(request: HttpRequest, session: Session, config: Data<Config>, db_cache: Data<DbCache>, auth_cache: Data<AuthCache>) -> impl Responder {
    let resp = HttpResponse::InternalServerError().json(json!(
        {
            "success": true,
            "message": "failed to retrieve session",
        }
    ));
    let user_info = match session.get::<UserInfo>(SESSION_KEY_USER) {
        Err(err) => {
            error!("failed to retrieve session: {}", err);
            return resp;
        }
        Ok(Some(v)) => v,
        Ok(None) => return resp,
    };


    let host = format!("{}://{}", request.connection_info().scheme(), request.connection_info().host());
    let logout_type = match auth_backend::new(&config).get_logout_type(&user_info, &host, &auth_cache) {
        Ok(logout_type) => logout_type,
        Err(err) => {
            error!("failed to determine logout type: {}", err);
            return HttpResponse::Unauthorized().json(json!(
               {
                   "success": false,
                   "message": "failed to retrieve logout type/url",
               }
            ));
        }
    };

    // best effort, key expires anyway
    let _ = _close_db(&session, &config, &db_cache).await;

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
            info!("user login from '{}': ", err);
            return HttpResponse::InternalServerError().json(json!(
                {
                    "success": false,
                    "message": "failed to read index file",
                }
            ));
        }
    };

    index = index.replace("</head>", format!(r#"
        <script>
            window.KeePass4WebResponse = {}
        </script>
        </head>
    "#, json!({
       "success": success,
       "message": message,
       "data": data,
    })).as_str());

    HttpResponse::Ok().content_type(TEXT_HTML).body(index)
}
