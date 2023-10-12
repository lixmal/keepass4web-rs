use std::any::Any;
use std::io::{Read, Write};

use anyhow::Result;

use crate::db_backend::DbBackend;

pub struct Test {
    pub buf: Vec<u8>,
}

impl DbBackend for Test {
    fn authenticated(&self) -> bool {
        true
    }

    fn get_db_read(&self) -> Result<Box<dyn Read + '_>> {
        Ok(Box::new(self.buf.as_slice()))
    }

    fn get_key_read(&self) -> Option<Result<Box<dyn Read>>> {
        None
    }

    fn get_db_write(&mut self) -> Result<Box<dyn Write + '_>> {
        Ok(Box::new(&mut self.buf))
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
