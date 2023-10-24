use actix_web::cookie;
use serde::{Deserialize, Deserializer};

#[derive(Clone)]
pub struct Key(pub(crate) cookie::Key);

impl<'de> Deserialize<'de> for Key {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: Deserializer<'de> {
        let b = String::deserialize(deserializer)?;
        Ok(
            Key(
                cookie::Key::try_from(b.as_bytes()).map_err(serde::de::Error::custom)?
            )
        )
    }
}
