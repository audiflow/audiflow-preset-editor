use axum::Json;
use preset_core::services::derive_preset_id;
use serde_json::Value;

use crate::app::AppError;

/// POST /api/configs/derive-pattern-id
///
/// Derives a deterministic pattern ID from the given podcast identity.
/// Returns the derived ID and which source field was used.
///
/// Request:  `{ "podcastGuid": "...", "feedUrls": ["..."] }`
/// Response: `{ "id": "a1b2c3d4e5f6", "source": "podcastGuid" | "feedUrls" }`
pub async fn derive_preset_id_handler(Json(body): Json<Value>) -> Result<Json<Value>, AppError> {
    if !body.is_object() {
        return Err(AppError::bad_request("Request body must be a JSON object"));
    }

    let guid = match body.get("podcastGuid") {
        None => None,
        Some(value) => {
            let s = value
                .as_str()
                .ok_or_else(|| AppError::bad_request("\"podcastGuid\" must be a string"))?;
            let s = s.trim();
            if s.is_empty() { None } else { Some(s) }
        }
    };

    let feed_urls: Vec<String> = match body.get("feedUrls") {
        None => Vec::new(),
        Some(value) => {
            let arr = value
                .as_array()
                .ok_or_else(|| AppError::bad_request("\"feedUrls\" must be an array of strings"))?;

            let mut urls = Vec::with_capacity(arr.len());
            for entry in arr {
                let url = entry.as_str().ok_or_else(|| {
                    AppError::bad_request("\"feedUrls\" must be an array of strings")
                })?;
                let url = url.trim();
                if !url.is_empty() {
                    urls.push(url.to_string());
                }
            }

            urls
        }
    };

    let has_guid = guid.is_some();
    let id = derive_preset_id(guid, &feed_urls).ok_or_else(|| {
        AppError::bad_request(
            "Cannot derive pattern ID: provide a non-empty podcastGuid or at least one feedUrls entry",
        )
    })?;

    let source = if has_guid { "podcastGuid" } else { "feedUrls" };

    Ok(Json(serde_json::json!({
        "id": id,
        "source": source,
    })))
}
