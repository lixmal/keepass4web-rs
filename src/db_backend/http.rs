use std::any::Any;
use std::pin::Pin;
use std::str::FromStr;

use anyhow::{bail, Result};
use anyhow::Error;
use async_trait::async_trait;
use futures_util::TryStreamExt;
use reqwest;
use reqwest::{Body, Client, Method, RequestBuilder, Response};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::oneshot;
use tokio::sync::oneshot::Receiver;
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
        let url = self.get_db_url(user_info)?;

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

    async fn get_db_write(&mut self, user_info: &UserInfo) -> Result<(Pin<Box<dyn AsyncWrite + '_>>, Option<Receiver<Result<()>>>)> {
        let url = self.get_db_url(user_info)?;

        let (asyncwriter, asyncreader) = tokio::io::duplex(256 * 1024);
        let streamreader = tokio_util::io::ReaderStream::new(asyncreader);

        let req = self.get_request(Method::PUT, url)?
            .body(Body::wrap_stream(streamreader));

        let (tx, rx) = oneshot::channel();
        tokio::spawn(async move {
            tx.send(match req.send().await {
                Ok(_) => Ok(()),
                Err(err) => Err(err).map_err(Error::new),
            }) // ignore failed send
        });

        Ok(
            (
                Box::pin(
                    asyncwriter
                ),
                Some(rx)
            )
        )
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

    fn get_db_url(&self, user_info: &UserInfo) -> Result<Url> {
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


#[cfg(test)]
mod tests {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use super::*;

    async fn write_http(url: Url) -> Result<()> {
        let mut config = Config::default();
        config.http.database_url = Some(url);
        let mut http = Http::new(&config);

        let (mut writer, rx) = http.get_db_write(&UserInfo::default()).await?;

        let data = "some random data";
        writer.write_all(data.as_bytes()).await?;
        writer.shutdown().await?;

        if let Some(rx) = rx {
            rx.await??
        }

        Ok(())
    }

    #[tokio::test]
    async fn read_ok() {
        let data = "some random data";

        let mut server = mockito::Server::new_async().await;
        let mock = server.mock("GET", "/")
            .with_body(data)
            .with_status(200)
            .create_async().await;

        let mut config = Config::default();
        config.http.database_url = Some(Url::from_str(&server.url()).unwrap());
        let http = Http::new(&config);
        let mut reader = http.get_db_read(&UserInfo::default()).await.unwrap();

        let mut str = String::new();
        reader.read_to_string(&mut str).await.unwrap();

        mock.assert_async().await;

        assert_eq!(data, &str);
    }

    #[tokio::test]
    async fn write_ok() {
        let mut server = mockito::Server::new_async().await;
        let mock = server.mock("PUT", "/")
            .match_body("some random data")
            .with_status(201)
            .create_async().await;

        write_http(Url::from_str(&server.url()).unwrap()).await.unwrap();

        mock.assert_async().await;
    }

    #[tokio::test]
    async fn write_fail() {
        let res = write_http(Url::from_str("http://0.0.0.0").unwrap()).await;

        assert!(res.is_err());
    }
}

