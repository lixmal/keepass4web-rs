use anyhow::Result;

use crate::auth_backend::{AuthBackend, UserInfo};

pub struct None;

impl AuthBackend for None {
    fn login(&self, _: &str, _: &str) -> Result<UserInfo> {
        Ok(UserInfo { name: "---".to_string() })
    }
}