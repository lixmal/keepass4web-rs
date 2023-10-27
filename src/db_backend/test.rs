use std::any::Any;
use std::pin::Pin;

use anyhow::Result;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::auth_backend::UserInfo;
use crate::db_backend::DbBackend;

pub struct Test {
    pub buf: Vec<u8>,
}

#[async_trait]
impl DbBackend for Test {
    fn authenticated(&self) -> bool {
        true
    }

    async fn get_db_read(&self, user_info: &UserInfo) -> Result<Pin<Box<dyn AsyncRead + '_>>> {
        Ok(Box::pin(self.buf.as_slice()))
    }

    async fn get_key_read(&self, _: &UserInfo) -> Option<Result<Pin<Box<dyn AsyncRead + '_>>>> {
        None
    }

    async fn get_db_write(&mut self, _: &UserInfo) -> Result<Pin<Box<dyn AsyncWrite + '_>>> {
        Ok(Box::pin(&mut self.buf))
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

impl Test {
    pub fn new() -> Self {
        Self {
            buf: Vec::new(),
        }
    }
}
