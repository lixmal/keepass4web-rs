use actix_session::Session;
use actix_web::{get, HttpResponse, Responder, web};
use actix_web::web::Data;
use log::info;
use mime::IMAGE_PNG;
use serde_json::json;
use secrecy::ExposeSecret;

use crate::config::config::Config;
use crate::keepass::db_cache::DbCache;
use crate::keepass::keepass::{File, Id, Protected, SearchTerm};
use crate::server::route::util;
use crate::session::AuthSession;

#[get("/get_groups")]
async fn get_groups(session: Session, config: Data<Config>, db_cache: Data<DbCache>) -> impl Responder {
    let keepass = match util::get_db(&session, &config, &db_cache).await {
        Ok(v) => v,
        Err(err) => return err,
    };

    let username = session.get_user_id();
    let (groups, last_selected) = match keepass.get_groups() {
        Ok(v) => v,
        Err(err) => {
            info!("{}: failed to get groups: {}", username, err);
            return HttpResponse::InternalServerError().json(json!(
                {
                    "success": false,
                    "message": "failed to get groups",
                }
            ));
        }
    };

    HttpResponse::Ok().json(json!(
        {
            "success": true,
            "data": {
                "groups": groups,
                "last_selected": last_selected,
            },
        }
    ))
}

#[get("/get_group_entries")]
async fn get_group_entries(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Query<Id>) -> impl Responder {
    let keepass = match util::get_db(&session, &config, &db_cache).await {
        Ok(v) => v,
        Err(err) => return err,
    };

    let username = session.get_user_id();
    let group_entries = match keepass.get_group_entries(&params) {
        Ok(v) => v,
        Err(err) => {
            info!("{}: failed to get entries for group '{}': {}", username, params.id, err);
            return HttpResponse::InternalServerError().json(json!(
                {
                    "success": false,
                    "message": "failed to get group entries",
                }
            ));
        }
    };

    HttpResponse::Ok().json(json!(
        {
            "success": true,
            "data": group_entries,
        }
    ))
}

#[get("/get_entry")]
async fn get_entry(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Query<Id>) -> impl Responder {
    let keepass = match util::get_db(&session, &config, &db_cache).await {
        Ok(v) => v,
        Err(err) => return err,
    };

    let username = session.get_user_id();
    let entry = match keepass.get_entry(&params) {
        Ok(v) => v,
        Err(err) => {
            info!("{}: failed to get entry '{}': {}", username, params.id, err);
            return HttpResponse::InternalServerError().json(json!(
                {
                    "success": false,
                    "message": "failed to get entry",
                }
            ));
        }
    };

    HttpResponse::Ok().json(json!(
        {
            "success": true,
            "data": entry,
        }
    ))
}

#[get("/get_protected")]
async fn get_protected(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Query<Protected>) -> impl Responder {
    let keepass = match util::get_db(&session, &config, &db_cache).await {
        Ok(v) => v,
        Err(err) => return err,
    };

    let username = session.get_user_id();
    let protected = match keepass.get_protected(&params) {
        Ok(v) => v,
        Err(err) => {
            info!("{}: failed to get protected '{}' of entry '{}': {}", username, params.name, params.entry_id, err);
            return HttpResponse::InternalServerError().json(json!(
                {
                    "success": false,
                    "message": "failed to get protected field",
                }
            ));
        }
    };

    HttpResponse::Ok().json(json!(
        {
            "success": true,
            "data": protected.expose_secret(),
        }
    ))
}

#[get("/get_otp")]
async fn get_otp(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Query<Id>) -> impl Responder {
    let keepass = match util::get_db(&session, &config, &db_cache).await {
        Ok(v) => v,
        Err(err) => return err,
    };

    let username = session.get_user_id();
    let otp = match keepass.get_otp(&params) {
        Ok(v) => v,
        Err(err) => {
            info!("{}: failed to get otp of entry '{}': {}", username, params.id, err);
            return HttpResponse::InternalServerError().json(json!(
                {
                    "success": false,
                    "message": "failed to get otp field",
                }
            ));
        }
    };

    HttpResponse::Ok().json(json!(
        {
            "success": true,
            "data": otp,
        }
    ))
}

#[get("/get_file")]
async fn get_file(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Query<File>) -> impl Responder {
    let keepass = match util::get_db(&session, &config, &db_cache).await {
        Ok(v) => v,
        Err(err) => return err,
    };

    let username = session.get_user_id();
    let file = match keepass.get_file(&params) {
        Ok(v) => v,
        Err(err) => {
            info!("{}: failed to get file '{}' of entry '{}': {}", username, params.filename, params.entry_id, err);
            return HttpResponse::InternalServerError().json(json!(
                {
                    "success": false,
                    "message": "failed to get file",
                }
            ));
        }
    };

    HttpResponse::Ok().body(file)
}

#[get("/search_entries")]
async fn search_entries(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Query<SearchTerm>) -> impl Responder {
    let keepass = match util::get_db(&session, &config, &db_cache).await {
        Ok(v) => v,
        Err(err) => return err,
    };

    let username = session.get_user_id();
    let entries = match keepass.search_entries(&params) {
        Ok(v) => v,
        Err(err) => {
            info!("{}: failed to search entries for term '{}': {}", username, params.term, err);

            let mut msg = "failed to search entries".to_string();
            if err.downcast_ref::<regex::Error>().is_some() {
                msg = format!("failed to search entries: {}", err);
            }

            return HttpResponse::InternalServerError().json(json!(
                {
                    "success": false,
                    "message": msg,
                }
            ));
        }
    };

    HttpResponse::Ok().json(json!(
        {
            "success": true,
            "data": entries,
        }
    ))
}

#[get("/icon/{id}")]
async fn get_icon(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Path<Id>) -> impl Responder {
    let keepass = match util::get_db(&session, &config, &db_cache).await {
        Ok(v) => v,
        Err(err) => return err,
    };
    let username = session.get_user_id();
    let icon = match keepass.get_icon(&params) {
        Ok(v) => v,
        Err(err) => {
            info!("{}: failed to get icon '{}': {}", username, params.id, err);
            // TODO: Serve 404 if icon not found
            return HttpResponse::InternalServerError().json(json!(
                {
                    "success": false,
                    "message": "failed to get icon",
                }
            ));
        }
    };

    HttpResponse::Ok()
        // UUID is unique, cache this forever
        .append_header(("Cache-Control", "max-age=31536000; public; s-max-age=31536000"))
        .append_header(("ETag", icon.uuid.to_string()))
        // TODO: sniff content type?
        .content_type(IMAGE_PNG)
        .body(icon.data.clone())
}

