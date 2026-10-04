use std::path::{Path, PathBuf};

use preset_core::models::{PlaylistDefinition, PresetMeta, RootMeta};
use preset_core::schema::SchemaType;

use crate::config_walker;

/// Formats config JSON files with 2-space indent and trailing newline.
/// Returns exit code: 0 = all formatted/already correct, 1 = would change (--check mode).
pub fn run(data_dir: &str, check: bool, files: &[String]) -> anyhow::Result<i32> {
    let data_path = PathBuf::from(data_dir);
    let presets_dir = config_walker::resolve_root_data_dir(&data_path)
        .unwrap_or_else(|| data_path.join("presets"));

    if files.is_empty() {
        format_all(&presets_dir, check)
    } else {
        format_files(data_dir, files, check)
    }
}

/// Formats all JSON files found under the patterns directory.
fn format_all(patterns_dir: &Path, check: bool) -> anyhow::Result<i32> {
    let mut would_change = false;
    let mut formatted_count = 0u32;

    config_walker::walk_config_files(patterns_dir, |path, schema_type| {
        let result = format_file(path, schema_type, check)?;
        if result == FormatResult::Changed {
            would_change = true;
            formatted_count += 1;
        }
        Ok(())
    })?;

    report_format_result(check, would_change, formatted_count)
}

/// Formats specific files or directories.
/// When a directory is given, walks it as a patterns directory.
fn format_files(data_dir: &str, files: &[String], check: bool) -> anyhow::Result<i32> {
    let mut would_change = false;
    let mut formatted_count = 0u32;
    let mut missing_count = 0u32;

    for file in files {
        let path = config_walker::resolve_file_path(data_dir, file);
        if !path.exists() {
            eprintln!("File not found: {}", path.display());
            missing_count += 1;
            continue;
        }

        if path.is_dir() {
            let presets_dir = if path.join("meta.json").exists() {
                path.clone()
            } else {
                config_walker::resolve_root_data_dir(&path).unwrap_or_else(|| path.join("presets"))
            };
            config_walker::walk_config_files(&presets_dir, |p, schema_type| {
                let result = format_file(p, schema_type, check)?;
                if result == FormatResult::Changed {
                    would_change = true;
                    formatted_count += 1;
                }
                Ok(())
            })?;
        } else {
            let schema_type = config_walker::detect_schema_type(&path);
            let result = format_file(&path, schema_type, check)?;
            if result == FormatResult::Changed {
                would_change = true;
                formatted_count += 1;
            }
        }
    }

    if 0 < missing_count {
        eprintln!("{missing_count} file(s) not found.");
        return Ok(2);
    }

    report_format_result(check, would_change, formatted_count)
}

fn report_format_result(check: bool, would_change: bool, count: u32) -> anyhow::Result<i32> {
    if check && would_change {
        println!("{count} file(s) would be reformatted.");
        return Ok(1);
    }
    if !check && 0 < count {
        println!("Formatted {count} file(s).");
    } else {
        println!("All files are already formatted.");
    }
    Ok(0)
}

#[derive(PartialEq)]
enum FormatResult {
    Unchanged,
    Changed,
}

/// Formats a single JSON file by round-tripping through the typed
/// model so default-valued fields are stripped, matching the API's
/// save behavior.  In check mode, reports whether it would change.
fn format_file(path: &Path, schema_type: SchemaType, check: bool) -> anyhow::Result<FormatResult> {
    let original = std::fs::read_to_string(path)?;

    let normalized = normalize_json(&original, schema_type)
        .map_err(|e| anyhow::anyhow!("Invalid JSON in {}: {e}", path.display()))?;

    let formatted = format!("{normalized}\n");

    if original == formatted {
        return Ok(FormatResult::Unchanged);
    }

    if check {
        println!("  Would reformat: {}", path.display());
        return Ok(FormatResult::Changed);
    }

    preset_server::services::atomic_write_str(path, &formatted)?;
    println!("  Formatted: {}", path.display());

    Ok(FormatResult::Changed)
}

/// Round-trips JSON through the typed model for the given schema type,
/// stripping default-valued fields via serde skip_serializing_if
/// attributes.
fn normalize_json(raw: &str, schema_type: SchemaType) -> Result<String, serde_json::Error> {
    match schema_type {
        SchemaType::PlaylistDefinition => {
            let mut model: PlaylistDefinition = serde_json::from_str(raw)?;
            model.strip_conditional_fields();
            serde_json::to_string_pretty(&model)
        }
        SchemaType::PresetMeta => {
            let model: PresetMeta = serde_json::from_str(raw)?;
            serde_json::to_string_pretty(&model)
        }
        SchemaType::PresetIndex => {
            let model: RootMeta = serde_json::from_str(raw)?;
            serde_json::to_string_pretty(&model)
        }
    }
}
