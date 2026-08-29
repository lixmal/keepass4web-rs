use serde::Deserialize;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Field {
    Title,
    Username,
    Tags,
    Notes,
    Url,
}

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct Search {
    #[serde(deserialize_with = "crate::config::env::list")]
    pub fields: Vec<Field>,
    #[serde(deserialize_with = "crate::config::env::scalar")]
    pub extra_fields: bool,
    #[serde(deserialize_with = "crate::config::env::scalar")]
    pub allow_regex: bool,
}

impl Default for Search {
    fn default() -> Self {
        Search {
            fields: vec![
                Field::Title,
                Field::Username,
                Field::Tags,
                Field::Notes,
                Field::Url,
            ],
            extra_fields: true,
            allow_regex: false,
        }
    }
}
