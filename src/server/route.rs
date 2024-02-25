use actix_files as fs;
use actix_files::NamedFile;
use actix_web::{Responder, web};

use crate::server::route::auth::{
    authenticated,
    backend_login,
    callback_user_auth,
    close_db,
    db_login,
    logout,
    user_login,
};
use crate::server::route::keepass::{get_entry, get_file, get_group_entries, get_groups, get_icon, get_otp, get_protected, search_entries};

pub mod auth;
pub mod keepass;
pub mod util;

pub const API_PATH: &str = "/api/v1";
pub const STATIC_PATH: &str = "/assets";
pub const INDEX_FILE: &str = "public/index.html";

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

            // keepass
            .service(get_groups)
            .service(get_group_entries)
            .service(get_entry)
            .service(get_protected)
            .service(get_otp)
            .service(get_file)
            .service(search_entries)
            .service(get_icon)
        )

        .service(callback_user_auth)

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

