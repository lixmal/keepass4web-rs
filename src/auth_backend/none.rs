use anyhow::Result;
use async_trait::async_trait;

use crate::auth_backend::{AuthBackend, AuthCache, LoginType, UserInfo};

pub struct None;

#[async_trait]
impl AuthBackend for None {
    fn get_login_type(&self, _: &str, _: &AuthCache) -> Result<LoginType> {
        Ok(LoginType::Mask)
    }

    fn login(&self, _: &str, _: &str) -> Result<UserInfo> {
        Ok(
            UserInfo {
                id: "".to_string(),
                name: "---".to_string(),
                db_location: Option::None,
                keyfile_location: Option::None,
                additional_data: Option::None,
            }
        )
    }
}