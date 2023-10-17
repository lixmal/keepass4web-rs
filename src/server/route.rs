use actix_files as fs;
use actix_files::NamedFile;
use actix_web::{get, Responder, web};

use crate::server::route::auth::{authenticated, backend_login, close_db, db_login, logout, callback_user_auth, user_login};
use crate::server::route::keepass::{
    get_entry,
    get_file,
    get_group_entries,
    get_groups,
    get_icon,
    get_protected,
    search_entries,
};

pub mod auth;
pub mod keepass;
pub mod util;

pub const STATIC_DIR: &str = "/assets";

pub fn setup_routes(cfg: &mut web::ServiceConfig) {
    cfg
        // index
        .service(index)

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
        .service(get_file)
        .service(search_entries)
        .service(get_icon)

        // static
        .service(fs::Files::new(
            STATIC_DIR, "public")
        );
}

#[get("/")]
async fn index() -> impl Responder {
    NamedFile::open_async("public/index.html").await
}
