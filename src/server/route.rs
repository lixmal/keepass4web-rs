use actix_files as fs;
use actix_files::NamedFile;
use actix_web::{HttpResponse, Responder, web};
use serde_json::json;

use crate::server::route::auth::{
    authenticated,
    backend_login,
    callback_user_auth,
    close_db,
    db_login,
    logout,
    save_db,
    user_login,
};
use crate::server::route::keepass::{
    create_entry,
    create_group,
    delete_entry,
    get_entry,
    get_file,
    get_group_entries,
    get_groups,
    get_icon,
    get_protected,
    rename_group,
    search_entries,
    update_entry,
};

pub mod auth;
pub mod keepass;
pub mod util;

pub const API_PATH: &str = "/api/v1";
pub const STATIC_PATH: &str = "/assets";
pub const INDEX_FILE: &str = "public/index.html";
pub const ROUTE_HEALTH: &str = "/health";

pub fn setup_routes(cfg: &mut web::ServiceConfig) {
    cfg
        .service(web::scope(API_PATH)
            // auth
            .service(authenticated)
            .service(user_login)
            .service(backend_login)
            .service(db_login)
            .service(close_db)
            .service(logout)
            .service(save_db)

            // keepass
            .service(get_groups)
            .service(get_group_entries)
            .service(get_entry)
            .service(get_protected)
            .service(get_file)
            .service(search_entries)
            .service(get_icon)
            .service(create_entry)
            .service(update_entry)
            .service(delete_entry)
            .service(create_group)
            .service(rename_group)
        )

        .service(callback_user_auth)

        // unauthenticated liveness probe
        .route(ROUTE_HEALTH, web::get().to(health))

        // static
        .route("/", web::get().to(index))
        .route("/keepass", web::get().to(index))
        .route("/user_login", web::get().to(index))
        .route("/backend_login", web::get().to(index))
        .route("/db_login", web::get().to(index))
        .service(fs::Files::new(STATIC_PATH, "public"))
    ;
}

async fn index() -> impl Responder {
    NamedFile::open_async(INDEX_FILE).await
}

async fn health() -> impl Responder {
    HttpResponse::Ok().json(json!(
        {
            "status": "ok",
        }
    ))
}

