use std::future::{ready, Ready};

use actix_session::SessionExt;
use actix_web::{
    body::EitherBody,
    dev::{self, Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpResponse,
};
use anyhow::Result;
use constant_time_eq::constant_time_eq;
use futures_util::future::LocalBoxFuture;
use rand::distributions::{Alphanumeric, DistString};
use rand::thread_rng;
use serde::{Deserialize, Deserializer};
use serde_json::json;
use zeroize::ZeroizeOnDrop;


use crate::server::route::STATIC_DIR;
use crate::session::AuthSession;

pub(crate) const SESSION_KEY_USER: &str = "user";
pub(crate) const SESSION_KEY_CSRF: &str = "csrf";

pub(crate) const SESSION_USER_UNKNOWN: &str = "unknown";

pub(crate) const CSRF_HEADER: &str = "X-CSRF-Token";

pub(crate) const ROUTE_ROOT: &str = "/";
pub(crate) const ROUTE_ICON: &str = "/icon";
pub(crate) const ROUTE_USER_LOGIN: &str = "/user_login";


#[derive(Deserialize, ZeroizeOnDrop)]
pub struct UserLogin {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, ZeroizeOnDrop)]
pub struct BackendLogin {
    pub username: String,
    pub password: String,
}

#[derive(Deserialize, ZeroizeOnDrop)]
pub struct DbLogin {
    #[serde(deserialize_with = "empty_string_is_none")]
    pub password: Option<String>,
    #[serde(deserialize_with = "empty_box_is_none")]
    pub key: Option<Box<[u8]>>,
}

fn empty_string_is_none<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(s))
    }
}

fn empty_box_is_none<'de, D>(deserializer: D) -> Result<Option<Box<[u8]>>, D::Error>
    where D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    if s.is_empty() {
        Ok(None)
    } else {
        Ok(Some(s.into_bytes().into_boxed_slice()))
    }
}

pub struct CheckAuth;

impl<S, B> Transform<S, ServiceRequest> for CheckAuth
    where
        S: Service<ServiceRequest, Response=ServiceResponse<B>, Error=Error>,
        S::Future: 'static,
        B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = CheckAuthMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(CheckAuthMiddleware { service }))
    }
}

pub struct CheckAuthMiddleware<S> {
    service: S,
}

impl<S, B> Service<ServiceRequest> for CheckAuthMiddleware<S>
    where
        S: Service<ServiceRequest, Response=ServiceResponse<B>, Error=Error>,
        S::Future: 'static,
        B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    dev::forward_ready!(service);

    fn call(&self, request: ServiceRequest) -> Self::Future {
        // TODO: don't return unauthorized on session backend error, but the actual error
        // Saves the user from some weird redirects
        if !request.get_session().is_authorized() {
            if request.path() != ROUTE_ROOT
                && request.path() != ROUTE_USER_LOGIN
                && !request.path().starts_with(format!("{}/", STATIC_DIR).as_str()) {
                let (request, _) = request.into_parts();

                let response = HttpResponse::Unauthorized().json(json!(
                   {
                       "success": false,
                       "message": "unauthorized",
                   }
                )).map_into_right_body();

                return Box::pin(async { Ok(ServiceResponse::new(request, response)) });
            }
        }
        // CSRF token is required for all routes from the moment user auth succeeds
        // with the exception for dynamic icons, as these are fetched by the browser without csrf
        else if request.path() != ROUTE_ROOT
            && !request.path().starts_with(format!("{}/", ROUTE_ICON).as_str())
            && !request.path().starts_with(format!("{}/", STATIC_DIR).as_str())
            && !csrf_matches(&request) {
            let (request, _) = request.into_parts();

            let response = HttpResponse::Forbidden().json(json!(
               {
                   "success": false,
                   "message": "csrf token mismatch",
               }
            )).map_into_right_body();

            return Box::pin(async { Ok(ServiceResponse::new(request, response)) });
        }

        let res = self.service.call(request);

        Box::pin(async move {
            res.await.map(ServiceResponse::map_into_left_body)
        })
    }
}


fn csrf_matches(request: &ServiceRequest) -> bool {
    if let Some(session_token) = request.get_session().get_key(SESSION_KEY_CSRF) {
        if let Some(request_token) = request.headers().get(CSRF_HEADER) {
            return constant_time_eq(session_token.as_bytes(), request_token.as_bytes());
        }
    }

    false
}

pub fn gen_token(length: usize) -> String {
    Alphanumeric.sample_string(&mut thread_rng(), length)
}
