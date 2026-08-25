use actix_session::Session;
use actix_web::{get, HttpResponse, Responder, web};
use actix_web::web::Data;
use log::info;
use serde_json::json;
use secrecy::ExposeSecret;

use crate::config::config::Config;
use crate::keepass::db_cache::DbCache;
use crate::keepass::keepass::{File, Id, NotFoundError, Protected, SearchTerm};
use crate::server::route::util;
use crate::session::AuthSession;

// 404 for missing entries/groups/icons, 500 for everything else
fn error_response(err: &anyhow::Error, message: &str) -> HttpResponse {
    let resp = json!(
        {
            "success": false,
            "message": message,
        }
    );
    if err.downcast_ref::<NotFoundError>().is_some() {
        return HttpResponse::NotFound().json(resp);
    }
    HttpResponse::InternalServerError().json(resp)
}

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
            return error_response(&err, "failed to get group entries");
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
            return error_response(&err, "failed to get entry");
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
            return error_response(&err, "failed to get protected field");
        }
    };

    HttpResponse::Ok().json(json!(
        {
            "success": true,
            "data": protected.expose_secret(),
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
            return error_response(&err, "failed to get icon");
        }
    };

    HttpResponse::Ok()
        // UUID is unique, cache this forever
        .append_header(("Cache-Control", "public, max-age=31536000, s-maxage=31536000, immutable"))
        .append_header(("ETag", icon.uuid.to_string()))
        .content_type(icon_mime(&icon.data))
        .body(icon.data.clone())
}

fn icon_mime(data: &[u8]) -> &'static str {
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        "image/png"
    } else if data.starts_with(b"\xff\xd8\xff") {
        "image/jpeg"
    } else if data.starts_with(b"GIF8") {
        "image/gif"
    } else if data.starts_with(b"<svg") || data.starts_with(b"<?xml") {
        "image/svg+xml"
    } else {
        // previous behavior, custom icons are usually png
        "image/png"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_mime_detection() {
        assert_eq!(icon_mime(b"\x89PNG\r\n\x1a\nrest"), "image/png");
        assert_eq!(icon_mime(b"\xff\xd8\xff\xe0rest"), "image/jpeg");
        assert_eq!(icon_mime(b"GIF89a"), "image/gif");
        assert_eq!(icon_mime(b"<svg xmlns="), "image/svg+xml");
        assert_eq!(icon_mime(b"unknown"), "image/png");
    }

    #[test]
    fn not_found_maps_to_404() {
        let err: anyhow::Error = NotFoundError("entry").into();
        assert_eq!(error_response(&err, "msg").status(), 404);

        let err = anyhow::anyhow!("other");
        assert_eq!(error_response(&err, "msg").status(), 500);
    }
}

