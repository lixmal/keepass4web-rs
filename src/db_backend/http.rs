use std::any::Any;
use std::pin::Pin;
use std::str::FromStr;

use anyhow::{bail, Result};
use async_trait::async_trait;
use futures_util::TryStreamExt;
use reqwest;
use reqwest::{Client, Method, RequestBuilder, Response};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio_util::compat::FuturesAsyncReadCompatExt;
use url::Url;

use crate::auth_backend::UserInfo;
use crate::config::config::Config;
use crate::config::http;
use crate::db_backend::DbBackend;

pub struct Http {
    pub config: http::Http,
}

#[async_trait]
impl DbBackend for Http {
    fn authenticated(&self) -> bool {
        true
    }

    async fn get_db_read(&self, user_info: &UserInfo) -> Result<Pin<Box<dyn AsyncRead + '_>>> {
        let url = self.get_db_url(&user_info)?;

        let response = self.get_request(Method::GET, url)?.send().await?;
        Ok(
            Self::get_boxed_response(response)
        )
    }

    async fn get_key_read(&self, user_info: &UserInfo) -> Option<Result<Pin<Box<dyn AsyncRead + '_>>>> {
        let mut url = None;
        if let Some(u) = &user_info.keyfile_location {
            url = match Url::from_str(u) {
                Ok(v) => Some(v),
                Err(err) => {
                    return Some(Err(err.into()));
                }
            }
        } else if self.config.keyfile_url.is_some() {
            url = self.config.keyfile_url.clone();
        }


        let url = match url {
            Some(u) => u,
            None => return None,
        };

        let request = match self.get_request(Method::GET, url) {
            Ok(v) => v,
            Err(err) => return Some(Err(err))
        };

        match request.send().await {
            Ok(response) => {
                Some(Ok(Self::get_boxed_response(response)))
            }
            Err(err) => Some(Err(err.into())),
        }
    }

    async fn get_db_write(&mut self, _user_info: &UserInfo) -> Result<Pin<Box<dyn AsyncWrite + '_>>> {
        unimplemented!()
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

impl Http {
    pub fn new(config: &Config) -> Self {
        Self {
            config: config.http.clone()
        }
    }

    fn get_boxed_response(response: Response) -> Pin<Box<dyn AsyncRead>> {
        Box::pin(
            response.bytes_stream().map_err(|e|
                futures::io::Error::new(
                    futures::io::ErrorKind::Other,
                    e,
                )
            )
                .into_async_read().compat()
        )
    }

    fn get_db_url(&self, user_info: &&UserInfo) -> Result<Url> {
        let url;
        if let Some(u) = &user_info.db_location {
            url = Url::from_str(u)?;
        } else if let Some(u) = &self.config.database_url {
            url = u.to_owned();
        } else {
            bail!("database file not specified in config nor found in user info")
        }
        Ok(url)
    }

    fn get_request(&self, method: Method, url: Url) -> Result<RequestBuilder> {
        let mut req = Client::builder()
            .build()?.
            request(method, url);

        if let Some(cred) = &self.config.credentials {
            req = req.basic_auth(cred.username.clone(), cred.password.clone());
        }

        if let Some(bearer) = &self.config.bearer {
            req = req.bearer_auth(bearer);
        }

        Ok(
            req
        )
    }
}
