use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
#[serde(remote = "actix_web::cookie::SameSite")]
pub enum SameSiteDef {
    None,
    Lax,
    Strict,
}
