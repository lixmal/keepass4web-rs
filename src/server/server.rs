use actix_session::{config::PersistentSession, SessionMiddleware, storage::CookieSessionStore};
use actix_web::{App, HttpServer, web};
use actix_web::cookie::time::Duration;
use actix_web::middleware::{DefaultHeaders, Logger};
use log::warn;
use anyhow::Result;
use env_logger::Env;

use crate::{auth, auth_backend};
use crate::config::config::Config;
use crate::keepass::db_cache::DbCache;
use crate::rate_limit::RateLimiter;
use crate::server::route::setup_routes;

pub struct Server;

impl Server {
    pub async fn new(config: Config) -> Result<()> {
        let server = config.listen.clone();
        let port = config.port;
        env_logger::init_from_env(Env::default().default_filter_or("info"));

        // the downgrade belongs in the log, not only in a config file someone
        // edited once
        #[cfg(target_os = "linux")]
        if !config.use_keyring {
            warn!("use_keyring is off: database keys are held in process memory instead of the kernel keyring");
        }

        let secret_key = config.session_secret_key.0.clone();
        let config_data = web::Data::new(config);
        let auth_cache = web::Data::new(auth_backend::new(&config_data).init().await?);
        let db_cache = web::Data::new(DbCache::default());
        let rate_limiter = web::Data::new(RateLimiter::default());

        HttpServer::new(move || {
            App::new()
                .app_data(db_cache.clone())
                .app_data(auth_cache.clone())
                .app_data(rate_limiter.clone())
                .app_data(config_data.clone())
                .wrap(auth::CheckAuth)
                .wrap(
                    SessionMiddleware::builder(
                        CookieSessionStore::default(),
                        secret_key.clone(),
                    )
                        .session_lifecycle(
                            PersistentSession::default()
                                .session_ttl(Duration::new(
                                    config_data.session_lifetime.as_secs() as i64,
                                    0,
                                ))
                        )
                        .cookie_same_site(config_data.cookie_samesite)
                        .build(),
                )
                .wrap(Logger::default())
                // registered last = outermost, so the headers are also set on
                // responses short-circuited by the middlewares above.
                // CSP is omitted for now: the index page relies on
                // inline script injection (see embed_in_index)
                .wrap(
                    DefaultHeaders::new()
                        .add(("X-Frame-Options", "DENY"))
                        .add(("X-Content-Type-Options", "nosniff"))
                        .add(("Referrer-Policy", "no-referrer"))
                )
                .configure(setup_routes)
        }).bind((server, port))?
            .run()
            .await
            .map_err(anyhow::Error::new)
    }
}
