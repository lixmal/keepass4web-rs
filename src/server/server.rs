use actix_session::{config::PersistentSession, SessionMiddleware, storage::CookieSessionStore};
use actix_web::{App, HttpServer, web};
use actix_web::cookie::SameSite::Strict;
use actix_web::cookie::time::Duration;
use actix_web::middleware::Logger;
use anyhow::Result;
use env_logger::Env;

use crate::{auth, auth_backend};
use crate::config::config::Config;
use crate::keepass::db_cache::DbCache;
use crate::server::route::setup_routes;

pub struct Server;

impl Server {
    pub async fn new(config: Config) -> Result<actix_server::Server> {
        let server = config.listen.clone();
        let port = config.port;
        env_logger::init_from_env(Env::default().default_filter_or("info"));

        let secret_key = config.session_secret_key.0.clone();
        let config_data = web::Data::new(config);
        let auth_cache = web::Data::new(auth_backend::new(&config_data).init().await?);
        let db_cache = web::Data::new(DbCache::default());

        let server = HttpServer::new(move || {
            App::new()
                .app_data(db_cache.clone())
                .app_data(auth_cache.clone())
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
                        .cookie_same_site(Strict)
                        .build(),
                )
                .wrap(Logger::default())
                .configure(setup_routes)
        }).bind((server, port))?.run();

        Ok(server)
    }
}
