use std::io::Read;

use actix_session::Session;
use actix_web::web::Form;
use anyhow::Result;

use crate::auth::BackendLogin;
use crate::config::backend;
use crate::config::config::Config;
use crate::db_backend::filesystem::Filesystem;

pub mod filesystem;

pub trait DbBackend {
    fn init(&self, _: Form<BackendLogin>) -> Result<()> { Ok(()) }
    fn authenticated(&self) -> bool;
    fn get_db(&self) -> Result<Box<dyn Read>>;
    // return None if the db backend doesn't return key files or is not configured to do so
    fn get_key(&self) -> Option<Result<Box<dyn Read>>>;
    fn put_db(&self, db: &[u8]) -> Result<()>;
}

pub fn new(config: &Config, _: &Session) -> Box<dyn DbBackend> {
    match config.db_backend {
        backend::DbBackend::Filesystem => Box::new(Filesystem::new(config)),
    }
}
