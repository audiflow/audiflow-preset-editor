use std::collections::HashSet;
use std::path::{Path, PathBuf};

use preset_core::models::PresetMeta;
use preset_core::schema::{SchemaType, Validator};
use preset_core::services::{check_uniqueness, derive_preset_id, is_deterministic_id};

use crate::config_walker;

/// Validates config files against JSON schemas.
/// Returns exit code: 0 = all valid, 1 = validation errors, 2 = file not found.
pub fn run(data_dir: &str, files: &[String]) -> anyhow::Result<i32> {
    let validator = Validator::from_embedded()
        .map_err(|e| anyhow::anyhow!("Failed to load embedded schemas: {e}"))?;
    let data_path = PathBuf::from(data_dir);
    let presets_dir = config_walker::resolve_root_data_dir(&data_path)
        .unwrap_or_else(|| data_path.join("presets"));

    if files.is_empty() {
        validate_all(&presets_dir, &validator)
    } else {
        validate_files(data_dir, files, &validator)
    }
}

/// Validates all config files found under the patterns directory.
fn validate_all(patterns_dir: &Path, validator: &Validator) -> anyhow::Result<i32> {
    let root_meta = patterns_dir.join("meta.json");
    if !root_meta.exists() {
        eprintln!("File not found: {}", root_meta.display());
        return Ok(2);
    }

    let mut error_count = 0u32;
    let mut file_count = 0u32;

    config_walker::walk_config_files(patterns_dir, |path, schema_type| {
        file_count += 1;
        error_count += validate_file(path, schema_type, validator)?;
        Ok(())
    })?;

    error_count += validate_root_index_uniqueness(&patterns_dir.join("meta.json"))?;
    error_count += validate_cross_pattern_uniqueness(patterns_dir)?;
    error_count += validate_preset_id_integrity(patterns_dir)?;

    println!("Validated {file_count} file(s).");
    if 0 < error_count {
        println!("{error_count} error(s) found.");
        Ok(1)
    } else {
        println!("All files are valid.");
        Ok(0)
    }
}

/// Validates specific files or directories, auto-detecting schema type from path.
/// When a directory is given, walks it as a patterns directory.
fn validate_files(data_dir: &str, files: &[String], validator: &Validator) -> anyhow::Result<i32> {
    let mut error_count = 0u32;
    let mut not_found = false;

    for file in files {
        let path = config_walker::resolve_file_path(data_dir, file);
        if !path.exists() {
            eprintln!("File not found: {}", path.display());
            not_found = true;
            continue;
        }

        if path.is_dir() {
            let presets_dir = if path.join("meta.json").exists() {
                path.clone()
            } else {
                config_walker::resolve_root_data_dir(&path).unwrap_or_else(|| path.join("presets"))
            };
            config_walker::walk_config_files(&presets_dir, |p, schema_type| {
                error_count += validate_file(p, schema_type, validator)?;
                Ok(())
            })?;
            error_count += validate_root_index_uniqueness(&presets_dir.join("meta.json"))?;
            error_count += validate_cross_pattern_uniqueness(&presets_dir)?;
            error_count += validate_preset_id_integrity(&presets_dir)?;
        } else {
            let schema_type = config_walker::detect_schema_type(&path);
            let is_root_index = matches!(schema_type, SchemaType::PresetIndex);
            error_count += validate_file(&path, schema_type, validator)?;
            if is_root_index {
                error_count += validate_root_index_uniqueness(&path)?;
            }
        }
    }

    if not_found {
        return Ok(2);
    }
    if 0 < error_count {
        Ok(1)
    } else {
        println!("All files are valid.");
        Ok(0)
    }
}

/// Validates a single file and prints errors.
/// Returns the number of validation errors found.
///
/// Human-readable status goes to stdout (OK/FAIL lines).
/// Structured JSON error objects go to stderr for machine consumption.
fn validate_file(
    path: &Path,
    schema_type: SchemaType,
    validator: &Validator,
) -> anyhow::Result<u32> {
    let content = std::fs::read_to_string(path)?;
    let value: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Invalid JSON in {}: {e}", path.display()))?;

    let schema_type_str = match schema_type {
        SchemaType::PresetIndex => "presetIndex",
        SchemaType::PresetMeta => "presetMeta",
        SchemaType::PlaylistDefinition => "playlistDefinition",
    };

    let errors = validator.validate(schema_type, &value);
    if errors.is_empty() {
        println!("  OK: {}", path.display());
        return Ok(0);
    }

    println!("  FAIL: {}", path.display());
    let error_json = serde_json::to_string_pretty(&serde_json::json!({
        "file": path.display().to_string(),
        "schemaType": schema_type_str,
        "errors": errors,
    }))?;
    eprintln!("{error_json}");

    let count = errors.len() as u32;
    Ok(count)
}

/// Fails when the root index lists a preset more than once. JSON Schema can only
/// compare whole items, so it cannot catch two entries sharing just an `id`, and
/// consumers keying on `id` would silently keep one of them.
/// Returns the number of duplicated IDs.
fn validate_root_index_uniqueness(root_meta: &Path) -> anyhow::Result<u32> {
    if !root_meta.exists() {
        return Ok(0);
    }
    let content = std::fs::read_to_string(root_meta)?;
    let root: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Invalid JSON in {}: {e}", root_meta.display()))?;

    let duplicates = find_duplicate_preset_ids(&root);
    for id in &duplicates {
        println!(
            "  FAIL: {} -- preset \"{id}\" is listed more than once",
            root_meta.display(),
        );
    }
    Ok(duplicates.len() as u32)
}

/// Returns each `id` appearing more than once in `presets`, in first-repeat order.
fn find_duplicate_preset_ids(root: &serde_json::Value) -> Vec<String> {
    let Some(entries) = root.get("presets").and_then(|v| v.as_array()) else {
        return Vec::new();
    };

    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();
    for id in entries
        .iter()
        .filter_map(|e| e.get("id").and_then(|v| v.as_str()))
    {
        if !seen.insert(id) && !duplicates.iter().any(|d| d == id) {
            duplicates.push(id.to_string());
        }
    }
    duplicates
}

/// Loads every pattern meta.json and checks for cross-pattern uniqueness
/// conflicts (duplicate podcastGuid or feedUrls).
/// Returns the number of conflicts found.
fn validate_cross_pattern_uniqueness(patterns_dir: &Path) -> anyhow::Result<u32> {
    let metas = load_preset_metas(patterns_dir)?;

    let mut conflict_count = 0u32;
    for i in 1..metas.len() {
        let candidate = &metas[i];
        let others = &metas[..i];
        let conflicts = check_uniqueness(candidate, others);
        for conflict in &conflicts {
            println!("  FAIL: pattern \"{}\" -- {conflict}", candidate.id);
            conflict_count += 1;
        }
    }

    Ok(conflict_count)
}

/// Checks that deterministic pattern IDs match their expected derivation.
/// Grandfathered (non-12-hex) IDs are skipped.
fn validate_preset_id_integrity(patterns_dir: &Path) -> anyhow::Result<u32> {
    let metas = load_preset_metas(patterns_dir)?;
    let mut error_count = 0u32;

    for meta in &metas {
        if !is_deterministic_id(&meta.id) {
            continue; // grandfathered
        }
        let expected = derive_preset_id(meta.podcast_guid.as_deref(), &meta.feed_urls);
        match expected {
            Some(expected_id) if meta.id != expected_id => {
                println!(
                    "  FAIL: pattern \"{}\" -- ID mismatch: expected \"{expected_id}\"",
                    meta.id,
                );
                error_count += 1;
            }
            None => {
                println!(
                    "  FAIL: pattern \"{}\" -- deterministic ID cannot be re-derived (missing podcastGuid and feedUrls)",
                    meta.id,
                );
                error_count += 1;
            }
            _ => {}
        }
    }

    Ok(error_count)
}

/// Reads pattern meta.json files from subdirectories of patterns_dir.
fn load_preset_metas(patterns_dir: &Path) -> anyhow::Result<Vec<PresetMeta>> {
    let mut metas: Vec<PresetMeta> = Vec::new();

    let mut dirs: Vec<PathBuf> = match std::fs::read_dir(patterns_dir) {
        Ok(entries) => entries
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect(),
        Err(_) => return Ok(metas),
    };
    dirs.sort();

    for dir in &dirs {
        let meta_path = dir.join("meta.json");
        if !meta_path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&meta_path)?;
        if let Ok(meta) = serde_json::from_str::<PresetMeta>(&content) {
            metas.push(meta);
        }
    }

    Ok(metas)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn duplicate_ids_are_reported_once_each() {
        let root = json!({ "presets": [
            { "id": "a" }, { "id": "b" }, { "id": "a" }, { "id": "a" }, { "id": "c" }, { "id": "c" }
        ]});
        assert_eq!(find_duplicate_preset_ids(&root), vec!["a", "c"]);
    }

    #[test]
    fn unique_ids_report_nothing() {
        let root = json!({ "presets": [{ "id": "a" }, { "id": "b" }] });
        assert!(find_duplicate_preset_ids(&root).is_empty());
    }

    #[test]
    fn missing_presets_array_reports_nothing() {
        assert!(find_duplicate_preset_ids(&json!({})).is_empty());
    }

    /// A root index that passes the schema, so only the duplicate check can fail it.
    fn root_with_duplicate_entries() -> serde_json::Value {
        let summary = json!({
            "id": "show_a",
            "dataVersion": 1,
            "displayName": "Show A",
            "feedUrlHints": ["https://example.com/rss"],
            "playlistCount": 1
        });
        json!({
            "dataVersion": 1,
            "schemaVersion": 8,
            "presets": [summary, summary]
        })
    }

    #[test]
    fn validate_all_fails_on_duplicate_root_entries() {
        let tmp = tempfile::TempDir::new().unwrap();
        let root = root_with_duplicate_entries();
        std::fs::write(tmp.path().join("meta.json"), root.to_string()).unwrap();

        let validator = Validator::from_embedded().unwrap();
        assert_eq!(validate_all(tmp.path(), &validator).unwrap(), 1);
    }

    #[test]
    fn validating_the_root_file_directly_fails_on_duplicate_entries() {
        let tmp = tempfile::TempDir::new().unwrap();
        let presets = tmp.path().join("presets");
        std::fs::create_dir_all(&presets).unwrap();
        let root_path = presets.join("meta.json");
        let root = root_with_duplicate_entries();
        std::fs::write(&root_path, root.to_string()).unwrap();

        let validator = Validator::from_embedded().unwrap();
        let files = [root_path.display().to_string()];
        assert_eq!(validate_files(".", &files, &validator).unwrap(), 1);
    }
}
