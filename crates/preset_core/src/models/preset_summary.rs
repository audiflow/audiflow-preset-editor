use serde::{Deserialize, Serialize};

use super::default_data_version;

/// Summary of a preset from root meta.json.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PresetSummary {
    pub id: String,

    #[serde(default = "default_data_version")]
    pub data_version: i32,

    pub display_name: String,
    pub feed_url_hints: Vec<String>,
    pub playlist_count: i32,
}
