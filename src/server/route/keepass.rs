use actix_session::Session;
use actix_web::{delete, get, post, put, HttpResponse, Responder, web};
use actix_web::web::Data;
use log::info;
use serde::Deserialize;
use serde_json::json;
use secrecy::ExposeSecret;
use uuid::Uuid;

use crate::config::config::Config;
use crate::keepass::db_cache::DbCache;
use crate::keepass::keepass::{CustomField, File, Id, NotFoundError, Protected, SearchTerm};
use crate::server::route::util;
use crate::session::AuthSession;

// what to move, and where it should end up
#[derive(Deserialize)]
struct Move {
    id: Uuid,
    group_id: Uuid,
}

#[derive(Deserialize)]
struct NewEntry {
    group_id: Uuid,
    title: String,
    username: String,
    password: String,
    url: String,
    notes: String,
    icon: Option<usize>,
    #[serde(default)]
    tags: String,
    #[serde(default)]
    fields: String,
}

#[derive(Deserialize)]
struct UpdateEntry {
    id: Uuid,
    title: String,
    username: String,
    password: String,
    url: String,
    notes: String,
    icon: Option<usize>,
    #[serde(default)]
    tags: String,
    #[serde(default)]
    fields: String,
}

// tags arrive as one comma separated field, the way the entry shows them
fn parse_tags(tags: &str) -> Vec<String> {
    tags.split(',')
        .map(|tag| tag.trim())
        .filter(|tag| !tag.is_empty())
        .map(|tag| tag.to_string())
        .collect()
}

// the custom fields of an entry are a list, which a form encoded body cannot
// carry on its own, so they travel as json in one field
fn parse_custom_fields(fields: &str) -> Result<Vec<CustomField>, serde_json::Error> {
    if fields.trim().is_empty() {
        return Ok(vec![]);
    }

    serde_json::from_str(fields)
}

#[derive(Deserialize)]
struct NewGroup {
    parent_id: Uuid,
    title: String,
}

#[derive(Deserialize)]
struct RenameGroup {
    id: Uuid,
    title: String,
}

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
    let data = match keepass.get_file(&params) {
        Ok(v) => v,
        Err(err) => {
            info!("{}: failed to get file '{}' of entry '{}': {}", username, params.filename, params.entry_id, err);
            return error_response(&err, "failed to get file");
        }
    };

    // the name is the user's, so it goes in quoted and stripped of anything
    // that would let it break out of the header
    let filename = params.filename.replace(['"', '\\', '\r', '\n'], "_");

    HttpResponse::Ok()
        // the bytes come out of the user's vault, so no cache along the way
        // gets to keep a copy
        .append_header(("Cache-Control", "no-store"))
        .append_header(("Content-Disposition", format!("attachment; filename=\"{}\"", filename)))
        .content_type("application/octet-stream")
        .body(data)
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
        .append_header(("ETag", icon.id().uuid().to_string()))
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

#[post("/entry")]
async fn create_entry(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Form<NewEntry>) -> impl Responder {
    let username = session.get_user_id();
    let mut new_id = Uuid::nil();

    let tags = parse_tags(&params.tags);
    let custom_fields = match parse_custom_fields(&params.fields) {
        Ok(fields) => fields,
        Err(err) => {
            info!("create_entry from '{}': {}", username, err);
            return HttpResponse::BadRequest().json(json!({
                "success": false,
                "message": "failed to read the custom fields",
            }));
        }
    };

    if let Err(err) = util::modify_db(&session, &config, &db_cache, |kp| {
        new_id = kp.create_entry(
            &params.group_id,
            &params.title,
            &params.username,
            &params.password,
            &params.url,
            &params.notes,
            params.icon,
            &tags,
            &custom_fields,
        )?;
        Ok(())
    }).await {
        return err;
    }

    info!("create_entry from '{}': {}", username, new_id);
    HttpResponse::Ok().json(json!({ "success": true, "data": { "id": new_id } }))
}

#[put("/entry")]
async fn update_entry(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Form<UpdateEntry>) -> impl Responder {
    let username = session.get_user_id();

    let tags = parse_tags(&params.tags);
    let custom_fields = match parse_custom_fields(&params.fields) {
        Ok(fields) => fields,
        Err(err) => {
            info!("update_entry from '{}': {}", username, err);
            return HttpResponse::BadRequest().json(json!({
                "success": false,
                "message": "failed to read the custom fields",
            }));
        }
    };

    if let Err(err) = util::modify_db(&session, &config, &db_cache, |kp| {
        kp.update_entry(
            &params.id,
            &params.title,
            &params.username,
            &params.password,
            &params.url,
            &params.notes,
            params.icon,
            &tags,
            &custom_fields,
        )
    }).await {
        return err;
    }

    info!("update_entry from '{}': {}", username, params.id);
    HttpResponse::Ok().json(json!({ "success": true }))
}

#[post("/group")]
async fn create_group(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Form<NewGroup>) -> impl Responder {
    let username = session.get_user_id();
    let mut new_id = Uuid::nil();

    if let Err(err) = util::modify_db(&session, &config, &db_cache, |kp| {
        new_id = kp.create_group(&params.parent_id, &params.title)?;
        Ok(())
    }).await {
        return err;
    }

    info!("create_group from '{}': {} under {}", username, new_id, params.parent_id);
    HttpResponse::Ok().json(json!({ "success": true, "data": { "id": new_id } }))
}

#[put("/group")]
async fn rename_group(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Form<RenameGroup>) -> impl Responder {
    let username = session.get_user_id();

    if let Err(err) = util::modify_db(&session, &config, &db_cache, |kp| {
        kp.rename_group(&params.id, &params.title)
    }).await {
        return err;
    }

    info!("rename_group from '{}': {}", username, params.id);
    HttpResponse::Ok().json(json!({ "success": true }))
}

#[delete("/entry")]
async fn delete_entry(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Form<Id>) -> impl Responder {
    let username = session.get_user_id();

    if let Err(err) = util::modify_db(&session, &config, &db_cache, |kp| {
        kp.delete_entry(&params.id)
    }).await {
        return err;
    }

    info!("delete_entry from '{}': {}", username, params.id);
    HttpResponse::Ok().json(json!({ "success": true }))
}

#[delete("/group")]
async fn delete_group(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Form<Id>) -> impl Responder {
    let username = session.get_user_id();

    if let Err(err) = util::modify_db(&session, &config, &db_cache, |kp| {
        kp.delete_group(&params.id)
    }).await {
        return err;
    }

    info!("delete_group from '{}': {}", username, params.id);
    HttpResponse::Ok().json(json!({ "success": true }))
}

#[put("/move_entry")]
async fn move_entry(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Form<Move>) -> impl Responder {
    let username = session.get_user_id();

    if let Err(err) = util::modify_db(&session, &config, &db_cache, |kp| {
        kp.move_entry(&params.id, &params.group_id)
    }).await {
        return err;
    }

    info!("move_entry from '{}': {} to {}", username, params.id, params.group_id);
    HttpResponse::Ok().json(json!({ "success": true }))
}

#[put("/move_group")]
async fn move_group(session: Session, config: Data<Config>, db_cache: Data<DbCache>, params: web::Form<Move>) -> impl Responder {
    let username = session.get_user_id();

    if let Err(err) = util::modify_db(&session, &config, &db_cache, |kp| {
        kp.move_group(&params.id, &params.group_id)
    }).await {
        return err;
    }

    info!("move_group from '{}': {} to {}", username, params.id, params.group_id);
    HttpResponse::Ok().json(json!({ "success": true }))
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

