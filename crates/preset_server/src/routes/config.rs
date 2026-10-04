use axum::Json;
use axum::extract::{Path, Query, State};
use serde_json::Value;

use crate::app::{AppError, SharedState};
use crate::services::local_config_repository::Error as RepoError;
use preset_core::models::{PlaylistDefinition, PresetMeta};
use preset_core::schema::SchemaType;
use preset_core::services::{check_uniqueness, derive_preset_id};

/// GET /api/configs/presets -- returns preset summaries.
pub async fn list_presets(State(state): State<SharedState>) -> Result<Json<Value>, AppError> {
    let presets = state.config_repo.list_presets()?;
    let json: Vec<Value> = presets
        .iter()
        .map(|p| serde_json::to_value(p).unwrap_or(Value::Null))
        .collect();
    Ok(Json(Value::Array(json)))
}

/// POST /api/configs/presets -- creates a new preset.
pub async fn create_preset(
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Result<(axum::http::StatusCode, Json<Value>), AppError> {
    let obj = body
        .as_object()
        .ok_or_else(|| AppError::bad_request("Request body must be a JSON object"))?;

    let id = obj
        .get("id")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::bad_request("Missing or invalid \"id\" field"))?
        .to_string();

    let meta = obj
        .get("meta")
        .and_then(|v| v.as_object())
        .ok_or_else(|| AppError::bad_request("Missing or invalid \"meta\" field"))?;

    // Validate and extract identity fields for deterministic ID derivation.
    let guid = match meta.get("podcastGuid") {
        None => None,
        Some(value) => {
            let s = value
                .as_str()
                .ok_or_else(|| AppError::bad_request("\"podcastGuid\" must be a string"))?;
            let s = s.trim();
            if s.is_empty() { None } else { Some(s) }
        }
    };
    let feed_urls: Vec<String> = match meta.get("feedUrls") {
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

    match derive_preset_id(guid, &feed_urls) {
        Some(expected_id) if id != expected_id => {
            return Err(AppError::bad_request(format!(
                "Preset ID must be \"{expected_id}\" (derived from {}). Got \"{id}\".",
                if guid.is_some() {
                    "podcastGuid"
                } else {
                    "feedUrls"
                },
            )));
        }
        None => {
            return Err(AppError::bad_request(
                "Cannot create preset: at least one of podcastGuid or feedUrls is required to derive an ID",
            ));
        }
        _ => {}
    }

    let display_name = obj
        .get("displayName")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(&id);

    if state.config_repo.preset_exists(&id) {
        return Err(AppError::conflict(format!(
            "Preset \"{id}\" already exists"
        )));
    }

    let mut normalized_meta = meta.clone();

    // Normalize podcastGuid: trim whitespace.
    if let Some(v) = normalized_meta.get_mut("podcastGuid")
        && let Some(s) = v.as_str()
    {
        *v = Value::String(s.trim().to_string());
    }

    // Normalize feedUrls: validate types, trim each entry, drop whitespace-only values.
    if let Some(v) = normalized_meta.get_mut("feedUrls") {
        let arr = v
            .as_array()
            .ok_or_else(|| AppError::bad_request("\"feedUrls\" must be an array of strings"))?;

        let mut cleaned = Vec::with_capacity(arr.len());
        for entry in arr {
            let url = entry
                .as_str()
                .ok_or_else(|| AppError::bad_request("\"feedUrls\" must be an array of strings"))?;
            let url = url.trim();
            if !url.is_empty() {
                cleaned.push(Value::String(url.to_string()));
            }
        }
        *v = Value::Array(cleaned);
    }

    let mut meta_with_version = Value::Object(normalized_meta);
    meta_with_version["dataVersion"] = Value::from(1);
    meta_with_version["id"] = Value::String(id.clone());

    // Validate against preset-meta schema before writing.
    let errors = state
        .validator
        .validate(SchemaType::PresetMeta, &meta_with_version);
    if !errors.is_empty() {
        return Err(AppError::bad_request(format!(
            "Validation failed: {}",
            errors.join("; ")
        )));
    }

    // Check cross-preset uniqueness (podcastGuid, feedUrls).
    let candidate: PresetMeta = serde_json::from_value(meta_with_version.clone())
        .map_err(|e| AppError::bad_request(format!("Invalid preset meta: {e}")))?;
    check_cross_preset_uniqueness(&state, &candidate, None)?;

    state.config_repo.create_preset(&id, &meta_with_version)?;

    // Add the new preset to root meta.json
    let mut root_meta = state.config_repo.get_root_meta_json()?;
    let presets = root_meta.get_mut("presets").and_then(|v| v.as_array_mut());

    let playlists = meta
        .get("playlists")
        .and_then(|v| v.as_array())
        .map_or(0, |a| a.len());

    let feed_url_hint = meta_with_version
        .get("feedUrls")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let summary = serde_json::json!({
        "id": id,
        "dataVersion": 1,
        "displayName": display_name,
        "feedUrlHint": feed_url_hint,
        "playlistCount": playlists,
    });

    match presets {
        Some(arr) => arr.push(summary),
        None => {
            root_meta["presets"] = Value::Array(vec![summary]);
        }
    }

    state.config_repo.save_root_meta(&root_meta)?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(serde_json::json!({ "ok": true, "id": id })),
    ))
}

/// GET /api/configs/presets/{id} -- returns preset metadata.
pub async fn get_preset(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let meta = state.config_repo.get_preset_meta(&id)?;
    let json = serde_json::to_value(meta)
        .map_err(|e| AppError::internal(format!("Serialization error: {e}")))?;
    Ok(Json(json))
}

/// DELETE /api/configs/presets/{id} -- deletes a preset and all playlists.
///
/// Also removes the preset's entry from root meta.json to keep the
/// browse listing consistent.
pub async fn delete_preset(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    state.config_repo.delete_preset(&id)?;

    // Remove from root meta.json
    let mut root_meta = state.config_repo.get_root_meta_json()?;
    if let Some(presets) = root_meta.get_mut("presets").and_then(|v| v.as_array_mut()) {
        presets.retain(|entry| entry.get("id").and_then(|v| v.as_str()) != Some(&id));
        state.config_repo.save_root_meta(&root_meta)?;
    }

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// PUT /api/configs/presets/{id}/meta -- saves preset meta to disk.
pub async fn update_preset_meta(
    State(state): State<SharedState>,
    Path(id): Path<String>,
    Json(mut body): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let obj = body
        .as_object_mut()
        .ok_or_else(|| AppError::bad_request("Request body must be a JSON object"))?;

    // Extract displayName before merging (lives in root meta only).
    // Empty string is rejected since the root index schema requires it.
    let display_name = obj.remove("displayName").and_then(|v| {
        match v {
            Value::String(s) if !s.is_empty() => Some(s),
            Value::String(_) => None, // empty string ignored, keeps existing
            Value::Null => None,
            _ => None,
        }
    });

    // Read-modify-write: preserve existing dataVersion.
    let mut existing = state.config_repo.get_preset_meta_json(&id)?;
    let existing_obj = existing
        .as_object_mut()
        .ok_or_else(|| AppError::internal("Existing meta is not a JSON object"))?;

    let preserved_data_version = existing_obj.get("dataVersion").cloned();

    // Merge body into existing: Value::Null means "remove key from existing"
    // so the client can revert tri-state fields back to schema-default.
    merge_meta_body(existing_obj, std::mem::take(obj));
    // Force canonical ID from route parameter
    existing_obj.insert("id".to_string(), Value::String(id.clone()));
    // Restore preserved dataVersion
    if let Some(dv) = preserved_data_version {
        existing_obj.insert("dataVersion".to_string(), dv);
    }

    let errors = state.validator.validate(SchemaType::PresetMeta, &existing);
    if !errors.is_empty() {
        return Err(AppError::bad_request(format!(
            "Validation failed: {}",
            errors.join("; ")
        )));
    }

    // Check cross-preset uniqueness (podcastGuid, feedUrls).
    let candidate: PresetMeta = serde_json::from_value(existing.clone())
        .map_err(|e| AppError::internal(format!("Preset meta deserialization error: {e}")))?;
    check_cross_preset_uniqueness(&state, &candidate, Some(&id))?;

    state.config_repo.save_preset_meta(&id, &existing)?;

    // Sync root meta.json
    sync_root_meta(&state, &id, &existing, display_name.as_deref())?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// GET /api/configs/presets/{id}/assembled -- assembles full config.
pub async fn get_assembled(
    State(state): State<SharedState>,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    let config = state.config_repo.assemble_config(&id)?;
    let mut json = serde_json::to_value(config)
        .map_err(|e| AppError::internal(format!("Serialization error: {e}")))?;

    // Enrich with displayName from root meta.json
    if let Ok(root_meta) = state.config_repo.get_root_meta_json()
        && let Some(presets) = root_meta.get("presets").and_then(|v| v.as_array())
    {
        for entry in presets {
            if entry.get("id").and_then(|v| v.as_str()) == Some(&id) {
                if let Some(name) = entry.get("displayName").and_then(|v| v.as_str())
                    && !name.is_empty()
                {
                    json["displayName"] = Value::String(name.to_string());
                }
                break;
            }
        }
    }

    Ok(Json(json))
}

/// GET /api/configs/presets/{id}/playlists/{pid} -- returns a playlist definition.
pub async fn get_playlist(
    State(state): State<SharedState>,
    Path((id, pid)): Path<(String, String)>,
) -> Result<Json<Value>, AppError> {
    let playlist = state.config_repo.get_playlist(&id, &pid)?;
    let json = serde_json::to_value(playlist)
        .map_err(|e| AppError::internal(format!("Serialization error: {e}")))?;
    Ok(Json(json))
}

/// PUT /api/configs/presets/{id}/playlists/{pid} -- saves a playlist definition.
pub async fn save_playlist(
    State(state): State<SharedState>,
    Path((id, pid)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, AppError> {
    if !body.is_object() {
        return Err(AppError::bad_request("Request body must be a JSON object"));
    }

    // Strip null values for schema validation
    let sanitized = strip_nulls(&body);

    let errors = state
        .validator
        .validate(SchemaType::PlaylistDefinition, &sanitized);
    if !errors.is_empty() {
        return Err(AppError::bad_request(format!(
            "Validation failed: {}",
            errors.join("; ")
        )));
    }

    // Round-trip through the typed model for canonical field ordering,
    // forcing the id to match the route parameter.
    let mut definition: PlaylistDefinition = serde_json::from_value(sanitized)
        .map_err(|e| AppError::bad_request(format!("Invalid playlist definition: {e}")))?;
    definition.id = pid.clone();
    definition.strip_conditional_fields();
    let normalized = serde_json::to_value(&definition)
        .map_err(|e| AppError::internal(format!("Serialization error: {e}")))?;

    state.config_repo.save_playlist(&id, &pid, &normalized)?;

    // Ensure playlist ID is listed in preset meta so assemble_config includes it
    add_playlist_to_meta(&state, &id, &pid)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

/// DELETE /api/configs/presets/{id}/playlists/{pid}
///
/// Removes the playlist file and its ID from the preset meta's
/// playlists array to keep assemble_config consistent.
pub async fn delete_playlist(
    State(state): State<SharedState>,
    Path((id, pid)): Path<(String, String)>,
) -> Result<Json<Value>, AppError> {
    // Reject if this is the last playlist (schema requires minItems: 1).
    let meta = state.config_repo.get_preset_meta_json(&id)?;
    let playlist_count = meta
        .get("playlists")
        .and_then(|v| v.as_array())
        .map_or(0, |a| a.len());
    if 2 > playlist_count {
        return Err(AppError::bad_request(
            "Cannot delete the last playlist; a preset must have at least one",
        ));
    }

    state.config_repo.delete_playlist(&id, &pid)?;

    // Remove pid from preset meta
    remove_playlist_from_meta(&state, &id, &pid)?;

    Ok(Json(serde_json::json!({ "ok": true })))
}

#[derive(serde::Deserialize)]
pub struct ValidateQuery {
    #[serde(rename = "type")]
    pub schema_type: Option<String>,
}

/// POST /api/configs/validate -- validates JSON against a schema.
pub async fn validate_config(
    State(state): State<SharedState>,
    Query(query): Query<ValidateQuery>,
    body: String,
) -> Result<Json<Value>, AppError> {
    if body.is_empty() {
        return Err(AppError::bad_request("Request body is empty"));
    }

    let value: Value =
        serde_json::from_str(&body).map_err(|_| AppError::bad_request("Invalid JSON"))?;

    let schema_type_str = query.schema_type.as_deref().unwrap_or("playlistDefinition");

    let schema_type = match schema_type_str {
        "playlistDefinition" => SchemaType::PlaylistDefinition,
        "presetMeta" => SchemaType::PresetMeta,
        "presetIndex" => SchemaType::PresetIndex,
        other => return Err(AppError::bad_request(format!("Invalid type: {other}"))),
    };

    let errors = state.validator.validate(schema_type, &value);
    Ok(Json(serde_json::json!({
        "valid": errors.is_empty(),
        "errors": errors,
    })))
}

/// Adds a playlist ID to the preset meta's playlists array if not already present,
/// and updates the playlistCount in root meta.
fn add_playlist_to_meta(
    state: &SharedState,
    preset_id: &str,
    playlist_id: &str,
) -> Result<(), AppError> {
    let mut meta = state.config_repo.get_preset_meta_json(preset_id)?;
    let playlists = meta.get_mut("playlists").and_then(|v| v.as_array_mut());

    let already_listed = playlists
        .as_ref()
        .is_some_and(|arr| arr.iter().any(|v| v.as_str() == Some(playlist_id)));

    if already_listed {
        return Ok(());
    }

    match playlists {
        Some(arr) => arr.push(Value::String(playlist_id.to_string())),
        None => {
            meta["playlists"] = Value::Array(vec![Value::String(playlist_id.to_string())]);
        }
    }

    state.config_repo.save_preset_meta(preset_id, &meta)?;
    sync_root_meta(state, preset_id, &meta, None)?;
    Ok(())
}

/// Removes a playlist ID from the preset meta's playlists array
/// and updates the playlistCount in root meta.
fn remove_playlist_from_meta(
    state: &SharedState,
    preset_id: &str,
    playlist_id: &str,
) -> Result<(), AppError> {
    let mut meta = state.config_repo.get_preset_meta_json(preset_id)?;
    if let Some(playlists) = meta.get_mut("playlists").and_then(|v| v.as_array_mut()) {
        playlists.retain(|v| v.as_str() != Some(playlist_id));
        state.config_repo.save_preset_meta(preset_id, &meta)?;
        sync_root_meta(state, preset_id, &meta, None)?;
    }
    Ok(())
}

/// Recursively removes null-valued keys from JSON maps.
fn strip_nulls(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut result = serde_json::Map::new();
            for (key, val) in map {
                if val.is_null() {
                    continue;
                }
                result.insert(key.clone(), strip_nulls(val));
            }
            Value::Object(result)
        }
        Value::Array(arr) => Value::Array(arr.iter().map(strip_nulls).collect()),
        other => other.clone(),
    }
}

/// Updates the root meta.json entry for a preset to keep browse
/// listing in sync with preset meta changes.
fn sync_root_meta(
    state: &SharedState,
    id: &str,
    meta: &Value,
    display_name: Option<&str>,
) -> Result<(), AppError> {
    let mut root_meta = state.config_repo.get_root_meta_json()?;
    let presets = match root_meta.get_mut("presets").and_then(|v| v.as_array_mut()) {
        Some(arr) => arr,
        None => return Ok(()),
    };

    let mut found = false;
    for entry in presets.iter_mut() {
        if entry.get("id").and_then(|v| v.as_str()) == Some(id) {
            let playlists_count = meta
                .get("playlists")
                .and_then(|v| v.as_array())
                .map_or(0, |a| a.len());
            entry["playlistCount"] = Value::from(playlists_count);
            entry["feedUrlHint"] = Value::String(feed_url_hint(meta));

            if let Some(name) = display_name {
                entry["displayName"] = Value::String(name.to_string());
            }
            found = true;
            break;
        }
    }

    if !found {
        return Ok(());
    }

    state.config_repo.save_root_meta(&root_meta)?;
    Ok(())
}

/// Loads all preset metas except `exclude_id` and checks for uniqueness conflicts.
fn check_cross_preset_uniqueness(
    state: &SharedState,
    candidate: &PresetMeta,
    exclude_id: Option<&str>,
) -> Result<(), AppError> {
    let summaries = state.config_repo.list_presets()?;
    let mut others = Vec::new();
    for summary in &summaries {
        if exclude_id == Some(summary.id.as_str()) {
            continue;
        }
        match state.config_repo.get_preset_meta(&summary.id) {
            Ok(meta) => others.push(meta),
            Err(RepoError::NotFound(_)) => continue,
            Err(e) => return Err(e.into()),
        }
    }

    let conflicts = check_uniqueness(candidate, &others);
    if conflicts.is_empty() {
        return Ok(());
    }

    let messages: Vec<String> = conflicts.iter().map(|c| c.to_string()).collect();
    Err(AppError::conflict(messages.join("; ")))
}

fn feed_url_hint(meta: &Value) -> String {
    meta.get("feedUrls")
        .and_then(|v| v.as_array())
        .and_then(|a| a.first())
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

/// Merges `body` into `existing` for `update_preset_meta`.
///
/// Treats `Value::Null` in `body` as "remove this key from `existing`" so a
/// client can express "revert to schema default" for tri-state fields like
/// `showEpisodeThumbnail`. Non-null values overwrite existing entries.
fn merge_meta_body(
    existing: &mut serde_json::Map<String, Value>,
    body: serde_json::Map<String, Value>,
) {
    for (key, value) in body {
        if value.is_null() {
            existing.remove(&key);
        } else {
            existing.insert(key, value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Map, json};

    #[test]
    fn null_value_deletes_existing_key() {
        let mut existing: Map<String, Value> = serde_json::from_value(json!({
            "showEpisodeThumbnail": false,
            "yearGroupedEpisodes": true,
        }))
        .unwrap();
        let body: Map<String, Value> = serde_json::from_value(json!({
            "showEpisodeThumbnail": null,
        }))
        .unwrap();
        merge_meta_body(&mut existing, body);
        assert!(!existing.contains_key("showEpisodeThumbnail"));
        assert_eq!(existing.get("yearGroupedEpisodes"), Some(&json!(true)));
    }

    #[test]
    fn non_null_value_replaces_existing_key() {
        let mut existing: Map<String, Value> = serde_json::from_value(json!({
            "showEpisodeThumbnail": true,
        }))
        .unwrap();
        let body: Map<String, Value> = serde_json::from_value(json!({
            "showEpisodeThumbnail": false,
        }))
        .unwrap();
        merge_meta_body(&mut existing, body);
        assert_eq!(existing.get("showEpisodeThumbnail"), Some(&json!(false)));
    }

    #[test]
    fn null_value_on_absent_key_is_a_noop() {
        let mut existing: Map<String, Value> = serde_json::from_value(json!({})).unwrap();
        let body: Map<String, Value> = serde_json::from_value(json!({
            "showEpisodeThumbnail": null,
        }))
        .unwrap();
        merge_meta_body(&mut existing, body);
        assert!(!existing.contains_key("showEpisodeThumbnail"));
    }
}
