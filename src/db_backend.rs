use std::any::Any;
use std::io::{Read, Write};

use actix_session::Session;
use actix_web::web::Form;
use anyhow::Result;

use crate::auth::BackendLogin;
use crate::config::backend;
use crate::config::config::Config;
use crate::db_backend::filesystem::Filesystem;
use crate::db_backend::test::Test;

pub mod filesystem;
pub mod test;

pub trait DbBackend {
    fn init(&self, _: Form<BackendLogin>) -> Result<()> { Ok(()) }
    fn authenticated(&self) -> bool;
    fn get_db_read(&self) -> Result<Box<dyn Read + '_>>;
    // return None if the db backend doesn't return key files or is not configured to do so
    fn get_key_read(&self) -> Option<Result<Box<dyn Read + '_>>>;
    fn get_db_write(&mut self) -> Result<Box<dyn Write + '_>>;
    fn as_any(&mut self) -> &mut dyn Any;
}

pub fn new(config: &Config, _: &Session) -> Box<dyn DbBackend> {
    match config.db_backend {
        backend::DbBackend::Test => Box::new(Test::new()),
        backend::DbBackend::Filesystem => Box::new(Filesystem::new(config)),
    }
}
