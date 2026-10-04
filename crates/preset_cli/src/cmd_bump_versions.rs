use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use preset_core::models::PresetMeta;

/// Validates that a string is safe to use as a single path segment.
/// Rejects empty strings, `.`, `..`, and strings containing `/`, `\`, or null bytes.
fn validate_path_segment(segment: &str) -> anyhow::Result<()> {
    if segment.is_empty()
        || !segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        anyhow::bail!("Invalid preset ID: {segment:?}");
    }
    Ok(())
}

/// Computes the repo-relative path for a given directory by querying
/// `git rev-parse --show-toplevel` and stripping the prefix.
/// Returns `(relative_path, toplevel)`.
fn repo_relative_path(path: &Path) -> anyhow::Result<(String, PathBuf)> {
    let git_cwd = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
    };
    let output = Command::new("git")
        .current_dir(&git_cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run git rev-parse: {e}"))?;
    if !output.status.success() {
        anyhow::bail!("Not a git repository");
    }
    let toplevel = std::fs::canonicalize(PathBuf::from(
        String::from_utf8_lossy(&output.stdout).trim(),
    ))
    .map_err(|e| anyhow::anyhow!("Failed to resolve git toplevel: {e}"))?;
    let abs_path = std::fs::canonicalize(path)
        .map_err(|e| anyhow::anyhow!("Failed to resolve {}: {e}", path.display()))?;
    let relative = abs_path
        .strip_prefix(&toplevel)
        .map_err(|_| anyhow::anyhow!("{} is not inside the git repository", path.display()))?;
    Ok((relative.to_string_lossy().replace('\\', "/"), toplevel))
}

/// Runs `git diff --name-only` against a previous ref, scoped to the presets directory.
fn git_diff_names(
    previous_ref: &str,
    presets_dir: &Path,
    repo_root: &Path,
) -> anyhow::Result<String> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["diff", previous_ref, "--name-only", "--"])
        .arg(presets_dir)
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run git diff: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("git diff failed: {stderr}");
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Retrieves file content at a previous git ref. Returns `None` if the file
/// did not exist at that ref.
fn git_show_file(
    previous_ref: &str,
    file_path: &str,
    repo_root: &Path,
) -> anyhow::Result<Option<String>> {
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(["show", &format!("{previous_ref}:{file_path}")])
        .output()
        .map_err(|e| anyhow::anyhow!("Failed to run git show: {e}"))?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}

/// Loads the previous `dataVersion` for each changed preset by reading
/// its `meta.json` from the given git ref. Returns an error if the file
/// exists at the ref but cannot be parsed.
fn load_previous_versions(
    previous_ref: &str,
    changed_ids: &[String],
    presets_dir_name: &str,
    repo_root: &Path,
) -> anyhow::Result<HashMap<String, i32>> {
    let mut versions = HashMap::new();
    for id in changed_ids {
        let path = format!("{presets_dir_name}/{id}/meta.json");
        if let Some(content) = git_show_file(previous_ref, &path, repo_root)? {
            let meta: PresetMeta = serde_json::from_str(&content)
                .map_err(|e| anyhow::anyhow!("Failed to parse {previous_ref}:{path}: {e}"))?;
            versions.insert(id.clone(), meta.data_version);
        }
    }
    Ok(versions)
}

/// Extracts unique preset IDs from a git diff output.
///
/// Parses lines like `presets/coten_radio/meta.json` and extracts
/// the preset ID (`coten_radio`). Skips root-level files (e.g.,
/// `presets/meta.json`) that have no subdirectory.
fn extract_changed_preset_ids(diff_output: &str, presets_dir_name: &str) -> Vec<String> {
    let prefix = format!("{presets_dir_name}/");
    let mut ids: Vec<String> = diff_output
        .lines()
        .filter_map(|line| line.strip_prefix(&prefix))
        .filter_map(|rest| {
            let slash_pos = rest.find('/')?;
            Some(rest[..slash_pos].to_string())
        })
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    ids.sort();
    ids
}

/// Computes new version numbers for changed presets.
///
/// For each changed preset ID, looks up the previous version and
/// increments by one. Presets not found in the previous map start
/// at version 1.
fn compute_version_bumps(
    changed_ids: &[String],
    previous_versions: &HashMap<String, i32>,
) -> HashMap<String, i32> {
    changed_ids
        .iter()
        .map(|id| {
            let new_version = previous_versions.get(id.as_str()).unwrap_or(&0) + 1;
            (id.clone(), new_version)
        })
        .collect()
}

/// Writes updated `dataVersion` into each changed preset's `meta.json`.
/// Skips presets whose `meta.json` no longer exists on disk (deleted presets).
fn apply_preset_bumps(presets_dir: &Path, bumps: &HashMap<String, i32>) -> anyhow::Result<()> {
    for (id, new_version) in bumps {
        let meta_path = presets_dir.join(id).join("meta.json");
        if !meta_path.exists() {
            continue;
        }
        let content = std::fs::read_to_string(&meta_path)
            .map_err(|e| anyhow::anyhow!("Failed to read {}: {e}", meta_path.display()))?;
        let mut value: serde_json::Value = serde_json::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse {}: {e}", meta_path.display()))?;
        value["dataVersion"] = serde_json::json!(new_version);
        let formatted = serde_json::to_string_pretty(&value)? + "\n";
        preset_server::services::atomic_write_str(&meta_path, &formatted)?;
    }
    Ok(())
}

/// Returns the name of the array field in root meta.json that holds preset
/// summaries. Supports both v7 `presets` and legacy v6 `patterns` for the
/// transition period.
fn detect_presets_array_key(root: &serde_json::Value) -> &'static str {
    if root.get("presets").and_then(|v| v.as_array()).is_some() {
        "presets"
    } else {
        "patterns"
    }
}

/// Bumps the root `meta.json` version, updates preset entries with new
/// `dataVersion` and `playlistCount` values, and writes the file atomically.
fn apply_root_bump(
    presets_dir: &Path,
    bumps: &HashMap<String, i32>,
    previous_ref: &str,
    presets_dir_name: &str,
    repo_root: &Path,
) -> anyhow::Result<i32> {
    let root_meta_path = presets_dir.join("meta.json");
    let content = std::fs::read_to_string(&root_meta_path)
        .map_err(|e| anyhow::anyhow!("Failed to read {}: {e}", root_meta_path.display()))?;
    let mut root: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| anyhow::anyhow!("Failed to parse {}: {e}", root_meta_path.display()))?;

    let prev_root_path = format!("{presets_dir_name}/meta.json");
    let prev_root_version = match git_show_file(previous_ref, &prev_root_path, repo_root)? {
        Some(content) => {
            let value: serde_json::Value = serde_json::from_str(&content).map_err(|e| {
                anyhow::anyhow!("Failed to parse {previous_ref}:{prev_root_path}: {e}")
            })?;
            value["dataVersion"].as_i64().ok_or_else(|| {
                anyhow::anyhow!("Missing or invalid dataVersion in {previous_ref}:{prev_root_path}")
            })? as i32
        }
        None => 0,
    };
    let new_root_version = prev_root_version + 1;
    root["dataVersion"] = serde_json::json!(new_root_version);

    let array_key = detect_presets_array_key(&root);
    if let Some(entries) = root[array_key].as_array_mut() {
        for entry in entries.iter_mut() {
            let Some(id) = entry["id"].as_str().map(String::from) else {
                continue;
            };
            if let Some(&new_version) = bumps.get(&id) {
                entry["dataVersion"] = serde_json::json!(new_version);
            }
            let meta_path = presets_dir.join(&id).join("meta.json");
            if meta_path.exists() {
                match std::fs::read_to_string(&meta_path)
                    .map_err(anyhow::Error::from)
                    .and_then(|c| serde_json::from_str::<PresetMeta>(&c).map_err(Into::into))
                {
                    Ok(meta) => {
                        entry["playlistCount"] = serde_json::json!(meta.playlists.len());
                    }
                    Err(e) => {
                        eprintln!("Warning: failed to read {}: {e}", meta_path.display());
                    }
                }
            }
        }
    }

    let formatted = serde_json::to_string_pretty(&root)? + "\n";
    preset_server::services::atomic_write_str(&root_meta_path, &formatted)?;
    Ok(new_root_version)
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BumpResult {
    preset_id: String,
    previous_version: i32,
    new_version: i32,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct BumpSummary {
    changed_presets: usize,
    root_version: i32,
    bumps: Vec<BumpResult>,
}

/// Resolves the presets directory. If the caller passed an explicit path,
/// uses it. Otherwise picks `presets/` if present, else legacy `patterns/`.
fn resolve_presets_dir(explicit: Option<&str>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        let path = PathBuf::from(p);
        return Some(path);
    }
    let cwd = std::env::current_dir().ok()?;
    crate::config_walker::resolve_root_data_dir(&cwd)
}

/// Entry point for the `bump-versions` subcommand.
///
/// Accepts an optional `--presets-dir <path>` (auto-detects `presets/` then
/// legacy `patterns/` when omitted) and a positional `<previous-ref>`. Detects
/// changed presets via `git diff`, loads previous versions, computes bumps,
/// and writes updated `dataVersion` fields to disk.
pub fn run(presets_dir: Option<&str>, previous_ref: &str, json: bool) -> anyhow::Result<i32> {
    let presets_path = resolve_presets_dir(presets_dir).ok_or_else(|| {
        anyhow::anyhow!(
            "No presets/ or patterns/ directory found in current working directory. \
             Pass --presets-dir <path> explicitly."
        )
    })?;
    if !presets_path.join("meta.json").exists() {
        anyhow::bail!("No meta.json found in {}", presets_path.display());
    }

    let (presets_dir_name, repo_root) = repo_relative_path(&presets_path)?;

    let diff_output = git_diff_names(previous_ref, Path::new(&presets_dir_name), &repo_root)?;

    if diff_output.trim().is_empty() {
        if json {
            println!(
                "{}",
                serde_json::to_string_pretty(&BumpSummary {
                    changed_presets: 0,
                    root_version: 0,
                    bumps: vec![],
                })?
            );
        } else {
            eprintln!("No preset changes detected.");
        }
        return Ok(0);
    }

    let changed_ids = extract_changed_preset_ids(&diff_output, &presets_dir_name);

    // Validate path segments to prevent directory traversal
    for id in &changed_ids {
        validate_path_segment(id)?;
    }

    // Filter out deleted presets whose meta.json no longer exists on disk
    let changed_ids: Vec<String> = changed_ids
        .into_iter()
        .filter(|id| presets_path.join(id).join("meta.json").exists())
        .collect();

    let previous_versions =
        load_previous_versions(previous_ref, &changed_ids, &presets_dir_name, &repo_root)?;
    let bumps = compute_version_bumps(&changed_ids, &previous_versions);

    apply_preset_bumps(&presets_path, &bumps)?;
    let new_root_version = apply_root_bump(
        &presets_path,
        &bumps,
        previous_ref,
        &presets_dir_name,
        &repo_root,
    )?;

    let results: Vec<BumpResult> = changed_ids
        .iter()
        .map(|id| BumpResult {
            preset_id: id.clone(),
            previous_version: *previous_versions.get(id).unwrap_or(&0),
            new_version: *bumps.get(id).unwrap_or(&1),
        })
        .collect();

    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&BumpSummary {
                changed_presets: results.len(),
                root_version: new_root_version,
                bumps: results,
            })?
        );
    } else {
        eprintln!("Detecting changes from {previous_ref}...");
        for r in &results {
            eprintln!(
                "  {}: version {} -> {}",
                r.preset_id, r.previous_version, r.new_version
            );
        }
        eprintln!("  Root meta: version -> {new_root_version}");
        eprintln!("Version bump complete.");
    }

    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_unique_preset_ids_from_diff() {
        let diff = "presets/coten_radio/meta.json\n\
                     presets/coten_radio/playlists/main.json\n\
                     presets/rebuild/meta.json\n";
        let result = extract_changed_preset_ids(diff, "presets");
        assert_eq!(result, vec!["coten_radio", "rebuild"]);
    }

    #[test]
    fn skips_root_meta_json() {
        let diff = "presets/meta.json\npresets/coten_radio/meta.json\n";
        let result = extract_changed_preset_ids(diff, "presets");
        assert_eq!(result, vec!["coten_radio"]);
    }

    #[test]
    fn returns_empty_for_no_preset_changes() {
        let diff = "README.md\nsrc/main.rs\n";
        let result = extract_changed_preset_ids(diff, "presets");
        assert!(result.is_empty());
    }

    #[test]
    fn handles_empty_diff() {
        let result = extract_changed_preset_ids("", "presets");
        assert!(result.is_empty());
    }

    #[test]
    fn bumps_existing_preset_version() {
        let mut previous: HashMap<String, i32> = HashMap::new();
        previous.insert("coten_radio".into(), 3);
        let changed = vec!["coten_radio".into()];
        let result = compute_version_bumps(&changed, &previous);
        assert_eq!(result.get("coten_radio"), Some(&4));
    }

    #[test]
    fn new_preset_starts_at_one() {
        let previous: HashMap<String, i32> = HashMap::new();
        let changed = vec!["new_preset".into()];
        let result = compute_version_bumps(&changed, &previous);
        assert_eq!(result.get("new_preset"), Some(&1));
    }

    #[test]
    fn validates_safe_path_segments() {
        assert!(validate_path_segment("coten_radio").is_ok());
        assert!(validate_path_segment("my-show").is_ok());
    }

    #[test]
    fn rejects_path_traversal_segments() {
        assert!(validate_path_segment("..").is_err());
        assert!(validate_path_segment(".").is_err());
        assert!(validate_path_segment("").is_err());
        assert!(validate_path_segment("foo/bar").is_err());
        assert!(validate_path_segment("foo\\bar").is_err());
        assert!(validate_path_segment("foo\0bar").is_err());
    }

    #[test]
    fn detect_array_key_prefers_presets() {
        let v = serde_json::json!({"presets": []});
        assert_eq!(detect_presets_array_key(&v), "presets");
    }

    #[test]
    fn detect_array_key_falls_back_to_patterns() {
        let v = serde_json::json!({"patterns": []});
        assert_eq!(detect_presets_array_key(&v), "patterns");
    }

    #[test]
    fn integration_bump_versions_in_temp_repo() {
        use std::process::Command;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let repo = tmp.path();

        // Init git repo
        Command::new("git")
            .args(["init"])
            .current_dir(repo)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo)
            .output()
            .unwrap();

        let presets = repo.join("presets");
        std::fs::create_dir_all(presets.join("show_a/playlists")).unwrap();

        // Root meta
        std::fs::write(
            presets.join("meta.json"),
            r#"{"dataVersion": 1, "schemaVersion": 7, "presets": [{"id": "show_a", "dataVersion": 1, "displayName": "Show A", "feedUrlHint": "https://example.com", "playlistCount": 1}]}"#,
        )
        .unwrap();

        // Preset meta
        std::fs::write(
            presets.join("show_a/meta.json"),
            r#"{"dataVersion": 1, "id": "show_a", "feedUrls": ["https://example.com"], "playlists": ["main", "bonus"]}"#,
        )
        .unwrap();

        // Playlist
        std::fs::write(
            presets.join("show_a/playlists/main.json"),
            r#"{"resolverType": "titleClassifier"}"#,
        )
        .unwrap();

        // Initial commit
        Command::new("git")
            .args(["add", "."])
            .current_dir(repo)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(repo)
            .output()
            .unwrap();

        // Make a change
        std::fs::write(
            presets.join("show_a/playlists/main.json"),
            r#"{"resolverType": "seasonNumber"}"#,
        )
        .unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(repo)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "update"])
            .current_dir(repo)
            .output()
            .unwrap();

        // Test pure functions against real git diff output
        let diff = Command::new("git")
            .args(["diff", "HEAD~1", "--name-only", "--", "presets"])
            .current_dir(repo)
            .output()
            .unwrap();
        let diff_output = String::from_utf8_lossy(&diff.stdout);
        let changed = extract_changed_preset_ids(&diff_output, "presets");
        assert_eq!(changed, vec!["show_a"]);

        let mut prev: HashMap<String, i32> = HashMap::new();
        prev.insert("show_a".into(), 1);
        let bumps = compute_version_bumps(&changed, &prev);
        assert_eq!(bumps.get("show_a"), Some(&2));
    }

    #[test]
    fn integration_run_bumps_versions_on_disk() {
        use std::process::Command;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let repo = tmp.path();

        // Init git repo
        Command::new("git")
            .args(["init"])
            .current_dir(repo)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo)
            .output()
            .unwrap();

        let presets = repo.join("presets");
        std::fs::create_dir_all(presets.join("show_a/playlists")).unwrap();

        std::fs::write(
            presets.join("meta.json"),
            r#"{"dataVersion": 1, "schemaVersion": 7, "presets": [{"id": "show_a", "dataVersion": 1, "displayName": "Show A", "feedUrlHint": "https://example.com", "playlistCount": 1}]}"#,
        )
        .unwrap();

        std::fs::write(
            presets.join("show_a/meta.json"),
            r#"{"dataVersion": 1, "id": "show_a", "feedUrls": ["https://example.com"], "playlists": ["main", "bonus"]}"#,
        )
        .unwrap();

        std::fs::write(
            presets.join("show_a/playlists/main.json"),
            r#"{"resolverType": "titleClassifier"}"#,
        )
        .unwrap();

        Command::new("git")
            .args(["add", "."])
            .current_dir(repo)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(repo)
            .output()
            .unwrap();

        // Make a change
        std::fs::write(
            presets.join("show_a/playlists/main.json"),
            r#"{"resolverType": "seasonNumber"}"#,
        )
        .unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(repo)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "update"])
            .current_dir(repo)
            .output()
            .unwrap();

        let result = run(Some(presets.to_str().unwrap()), "HEAD~1", false);

        assert!(result.is_ok(), "run() failed: {:?}", result.err());
        assert_eq!(result.unwrap(), 0);

        // Verify preset meta was bumped
        let meta: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(presets.join("show_a/meta.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(meta["dataVersion"], 2);

        // Verify root meta was bumped
        let root: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(presets.join("meta.json")).unwrap())
                .unwrap();
        assert_eq!(root["dataVersion"], 2);
        assert_eq!(root["presets"][0]["dataVersion"], 2);
        // "main" and "bonus" in playlists array
        assert_eq!(root["presets"][0]["playlistCount"], 2);
    }

    #[test]
    fn integration_run_bumps_legacy_patterns_layout() {
        use std::process::Command;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let repo = tmp.path();

        Command::new("git")
            .args(["init"])
            .current_dir(repo)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo)
            .output()
            .unwrap();

        let patterns = repo.join("patterns");
        std::fs::create_dir_all(patterns.join("show_a/playlists")).unwrap();

        std::fs::write(
            patterns.join("meta.json"),
            r#"{"dataVersion": 1, "schemaVersion": 6, "patterns": [{"id": "show_a", "dataVersion": 1, "displayName": "Show A", "feedUrlHint": "https://example.com", "playlistCount": 1}]}"#,
        )
        .unwrap();

        std::fs::write(
            patterns.join("show_a/meta.json"),
            r#"{"dataVersion": 1, "id": "show_a", "feedUrls": ["https://example.com"], "playlists": ["main"]}"#,
        )
        .unwrap();

        std::fs::write(
            patterns.join("show_a/playlists/main.json"),
            r#"{"resolverType": "titleClassifier"}"#,
        )
        .unwrap();

        Command::new("git")
            .args(["add", "."])
            .current_dir(repo)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "init"])
            .current_dir(repo)
            .output()
            .unwrap();

        std::fs::write(
            patterns.join("show_a/playlists/main.json"),
            r#"{"resolverType": "seasonNumber"}"#,
        )
        .unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(repo)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "update"])
            .current_dir(repo)
            .output()
            .unwrap();

        let result = run(Some(patterns.to_str().unwrap()), "HEAD~1", false);

        assert!(
            result.is_ok(),
            "run() failed on legacy patterns/ layout: {:?}",
            result.err()
        );

        let root: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(patterns.join("meta.json")).unwrap())
                .unwrap();
        assert_eq!(root["dataVersion"], 2);
        assert_eq!(root["patterns"][0]["dataVersion"], 2);
    }
}
