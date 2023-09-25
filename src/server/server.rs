use std::io::Error;

use actix_session::{SessionMiddleware, storage::CookieSessionStore};
use actix_web::{App, HttpServer, web};
use actix_web::middleware::Logger;
use anyhow::Result;
use env_logger::Env;

use crate::auth;
use crate::config::config::Config;
use crate::keepass::db_cache::DbCache;
use crate::server::route::setup_routes;

pub struct Server;

impl Server {
    pub fn new(config: Config) -> Result<actix_server::Server, Error> {
        let server = config.listen.clone();
        let port = config.port;
        env_logger::init_from_env(Env::default().default_filter_or("info"));


        let secret_key = config.session_secret_key.0.clone();
        let config_data = web::Data::new(config);
        let db_cache = web::Data::new(DbCache::default());

        let server = HttpServer::new(move || {
            App::new()
                .app_data(db_cache.clone())
                .app_data(config_data.clone())
                .wrap(auth::CheckAuth)
                .wrap(
                    SessionMiddleware::new(
                        CookieSessionStore::default(),
                        secret_key.clone(),
                    )
                )
                .wrap(Logger::default())
                .configure(setup_routes)
        }).bind((server, port))?.run();

        Ok(server)
    }
}

