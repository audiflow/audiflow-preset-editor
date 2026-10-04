use std::path::{Path, PathBuf};

use preset_core::models::{PlaylistDefinition, PresetConfig, PresetMeta, PresetSummary, RootMeta};
use preset_core::services::ConfigAssembler;
use serde_json::Value;

/// Repository that reads and writes split config files from the local
/// filesystem. Files are stored under `$data_dir/presets/`.
pub struct LocalConfigRepository {
    presets_dir: PathBuf,
}

impl LocalConfigRepository {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            presets_dir: data_dir.join("presets"),
        }
    }

    // -- Read methods --

    /// Lists all preset summaries from root meta.json.
    pub fn list_presets(&self) -> Result<Vec<PresetSummary>, Error> {
        let root_meta = self.get_root_meta()?;
        Ok(root_meta.presets)
    }

    /// Gets the root meta.json as a typed struct.
    pub fn get_root_meta(&self) -> Result<RootMeta, Error> {
        let raw = self.read_file(&self.presets_dir.join("meta.json"))?;
        let root_meta: RootMeta =
            serde_json::from_str(&raw).map_err(|e| Error::ParseError(e.to_string()))?;
        Ok(root_meta)
    }

    /// Returns the root meta.json as a raw JSON value for
    /// read-modify-write cycles.
    pub fn get_root_meta_json(&self) -> Result<Value, Error> {
        let raw = self.read_file(&self.presets_dir.join("meta.json"))?;
        serde_json::from_str(&raw).map_err(|e| Error::ParseError(e.to_string()))
    }

    /// Gets preset metadata for a specific preset.
    pub fn get_preset_meta(&self, preset_id: &str) -> Result<PresetMeta, Error> {
        validate_path_segment(preset_id, "presetId")?;
        let path = self.presets_dir.join(preset_id).join("meta.json");
        let raw = self.read_file(&path)?;
        serde_json::from_str(&raw).map_err(|e| Error::ParseError(e.to_string()))
    }

    /// Returns a preset's meta.json as a raw JSON value for
    /// read-modify-write cycles.
    pub fn get_preset_meta_json(&self, preset_id: &str) -> Result<Value, Error> {
        validate_path_segment(preset_id, "presetId")?;
        let path = self.presets_dir.join(preset_id).join("meta.json");
        let raw = self.read_file(&path)?;
        serde_json::from_str(&raw).map_err(|e| Error::ParseError(e.to_string()))
    }

    /// Gets a single playlist definition by preset and playlist ID.
    pub fn get_playlist(
        &self,
        preset_id: &str,
        playlist_id: &str,
    ) -> Result<PlaylistDefinition, Error> {
        validate_path_segment(preset_id, "presetId")?;
        validate_path_segment(playlist_id, "playlistId")?;
        let path = self
            .presets_dir
            .join(preset_id)
            .join("playlists")
            .join(format!("{playlist_id}.json"));
        let raw = self.read_file(&path)?;
        serde_json::from_str(&raw).map_err(|e| Error::ParseError(e.to_string()))
    }

    /// Returns a playlist definition as a raw JSON value.
    pub fn get_playlist_json(&self, preset_id: &str, playlist_id: &str) -> Result<Value, Error> {
        validate_path_segment(preset_id, "presetId")?;
        validate_path_segment(playlist_id, "playlistId")?;
        let path = self
            .presets_dir
            .join(preset_id)
            .join("playlists")
            .join(format!("{playlist_id}.json"));
        let raw = self.read_file(&path)?;
        serde_json::from_str(&raw).map_err(|e| Error::ParseError(e.to_string()))
    }

    /// Assembles a full config from preset meta and all playlists.
    pub fn assemble_config(&self, preset_id: &str) -> Result<PresetConfig, Error> {
        let meta = self.get_preset_meta(preset_id)?;
        let mut playlists = Vec::new();
        for playlist_id in &meta.playlists {
            playlists.push(self.get_playlist(preset_id, playlist_id)?);
        }
        Ok(ConfigAssembler::assemble(&meta, &playlists))
    }

    // -- Write methods --

    /// Writes playlist JSON to disk using atomic write.
    pub fn save_playlist(
        &self,
        preset_id: &str,
        playlist_id: &str,
        json: &Value,
    ) -> Result<(), Error> {
        validate_path_segment(preset_id, "presetId")?;
        validate_path_segment(playlist_id, "playlistId")?;
        let path = self
            .presets_dir
            .join(preset_id)
            .join("playlists")
            .join(format!("{playlist_id}.json"));
        atomic_write_json(&path, json)
    }

    /// Writes preset meta JSON to disk using atomic write.
    pub fn save_preset_meta(&self, preset_id: &str, json: &Value) -> Result<(), Error> {
        validate_path_segment(preset_id, "presetId")?;
        let path = self.presets_dir.join(preset_id).join("meta.json");
        atomic_write_json(&path, json)
    }

    /// Writes the root meta.json from a raw JSON value using atomic write.
    pub fn save_root_meta(&self, json: &Value) -> Result<(), Error> {
        let path = self.presets_dir.join("meta.json");
        atomic_write_json(&path, json)
    }

    /// Returns true if a preset directory already exists on disk.
    pub fn preset_exists(&self, preset_id: &str) -> bool {
        self.presets_dir.join(preset_id).join("meta.json").exists()
    }

    /// Creates a new preset directory with playlists/ subdir and
    /// writes the initial meta.json.
    pub fn create_preset(&self, preset_id: &str, meta_json: &Value) -> Result<(), Error> {
        validate_path_segment(preset_id, "presetId")?;
        let preset_dir = self.presets_dir.join(preset_id);
        std::fs::create_dir_all(&preset_dir).map_err(Error::Io)?;

        let playlists_dir = preset_dir.join("playlists");
        std::fs::create_dir_all(&playlists_dir).map_err(Error::Io)?;

        let meta_path = preset_dir.join("meta.json");
        atomic_write_json(&meta_path, meta_json)
    }

    // -- Delete methods --

    /// Deletes a playlist file from disk.
    pub fn delete_playlist(&self, preset_id: &str, playlist_id: &str) -> Result<(), Error> {
        validate_path_segment(preset_id, "presetId")?;
        validate_path_segment(playlist_id, "playlistId")?;
        let path = self
            .presets_dir
            .join(preset_id)
            .join("playlists")
            .join(format!("{playlist_id}.json"));
        std::fs::remove_file(&path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Error::NotFound(path.display().to_string()),
            _ => Error::Io(e),
        })
    }

    /// Deletes an entire preset directory recursively.
    pub fn delete_preset(&self, preset_id: &str) -> Result<(), Error> {
        validate_path_segment(preset_id, "presetId")?;
        let dir = self.presets_dir.join(preset_id);
        std::fs::remove_dir_all(&dir).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Error::NotFound(dir.display().to_string()),
            _ => Error::Io(e),
        })
    }

    // -- Private helpers --

    fn read_file(&self, path: &Path) -> Result<String, Error> {
        std::fs::read_to_string(path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => Error::NotFound(path.display().to_string()),
            _ => Error::Io(e),
        })
    }
}

/// Validates that a path segment contains only safe characters.
fn validate_path_segment(segment: &str, label: &str) -> Result<(), Error> {
    if segment.is_empty()
        || !segment
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    {
        return Err(Error::InvalidPathSegment {
            label: label.to_string(),
            value: segment.to_string(),
        });
    }
    Ok(())
}

/// Writes JSON to a file atomically: write to .tmp first, then rename.
/// Output is pretty-printed with 2-space indent and a trailing newline.
fn atomic_write_json(path: &Path, json: &Value) -> Result<(), Error> {
    let content = format!(
        "{}\n",
        serde_json::to_string_pretty(json).map_err(|e| Error::ParseError(e.to_string()))?
    );
    super::atomic_write::atomic_write_str(path, &content).map_err(Error::Io)
}

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    ParseError(String),
    NotFound(String),
    InvalidPathSegment { label: String, value: String },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Io(e) => write!(f, "IO error: {e}"),
            Error::ParseError(msg) => write!(f, "Parse error: {msg}"),
            Error::NotFound(path) => write!(f, "Not found: {path}"),
            Error::InvalidPathSegment { label, value } => {
                write!(
                    f,
                    "{label} '{value}' must contain only alphanumeric characters, hyphens, or underscores"
                )
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Io(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    /// Creates a temporary data directory with root meta.json and a
    /// preset directory containing meta.json and a playlist file.
    fn setup_test_dir() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let presets_dir = tmp.path().join("presets");
        std::fs::create_dir_all(&presets_dir).unwrap();

        let root_meta = json!({
            "dataVersion": 1,
            "schemaVersion": 3,
            "presets": [
                {
                    "id": "test-pattern",
                    "dataVersion": 1,
                    "displayName": "Test Pattern",
                    "feedUrlHint": "https://example.com/feed.xml",
                    "playlistCount": 1
                }
            ]
        });
        std::fs::write(
            presets_dir.join("meta.json"),
            format!("{}\n", serde_json::to_string_pretty(&root_meta).unwrap()),
        )
        .unwrap();

        // Create preset directory structure
        let preset_dir = presets_dir.join("test-pattern");
        std::fs::create_dir_all(preset_dir.join("playlists")).unwrap();

        let preset_meta = json!({
            "dataVersion": 1,
            "id": "test-pattern",
            "feedUrls": ["https://example.com/feed.xml"],
            "playlists": ["playlist-1"]
        });
        std::fs::write(
            preset_dir.join("meta.json"),
            format!("{}\n", serde_json::to_string_pretty(&preset_meta).unwrap()),
        )
        .unwrap();

        let playlist = json!({
            "id": "playlist-1",
            "displayName": "Playlist One",
            "priority": 0,
            "grouping": { "by": "seasonNumber" }
        });
        std::fs::write(
            preset_dir.join("playlists").join("playlist-1.json"),
            format!("{}\n", serde_json::to_string_pretty(&playlist).unwrap()),
        )
        .unwrap();

        tmp
    }

    #[test]
    fn list_presets_reads_root_meta() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        let presets = repo.list_presets().unwrap();
        assert_eq!(presets.len(), 1);
        assert_eq!(presets[0].id, "test-pattern");
        assert_eq!(presets[0].display_name, "Test Pattern");
    }

    #[test]
    fn get_preset_meta_reads_correct_file() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        let meta = repo.get_preset_meta("test-pattern").unwrap();
        assert_eq!(meta.id, "test-pattern");
        assert_eq!(meta.feed_urls, vec!["https://example.com/feed.xml"]);
        assert_eq!(meta.playlists, vec!["playlist-1"]);
    }

    #[test]
    fn get_playlist_reads_correct_file() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        let playlist = repo.get_playlist("test-pattern", "playlist-1").unwrap();
        assert_eq!(playlist.id, "playlist-1");
        assert_eq!(playlist.display_name, "Playlist One");
        assert_eq!(playlist.grouping.by, "seasonNumber");
    }

    #[test]
    fn assemble_config_combines_meta_and_playlists() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        let config = repo.assemble_config("test-pattern").unwrap();
        assert_eq!(config.id, "test-pattern");
        assert_eq!(config.playlists.len(), 1);
        assert_eq!(config.playlists[0].id, "playlist-1");
        assert_eq!(
            config.feed_urls,
            Some(vec!["https://example.com/feed.xml".to_string()])
        );
    }

    #[test]
    fn save_playlist_writes_atomically() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        let new_playlist = json!({
            "id": "playlist-2",
            "displayName": "Playlist Two",
            "priority": 1,
            "grouping": { "by": "titleClassifier" }
        });

        repo.save_playlist("test-pattern", "playlist-2", &new_playlist)
            .unwrap();

        // Verify the file was written correctly
        let loaded = repo.get_playlist("test-pattern", "playlist-2").unwrap();
        assert_eq!(loaded.id, "playlist-2");
        assert_eq!(loaded.display_name, "Playlist Two");

        // Verify no .tmp file remains
        let tmp_path = tmp
            .path()
            .join("presets/test-pattern/playlists/playlist-2.json.tmp");
        assert!(!tmp_path.exists());
    }

    #[test]
    fn save_playlist_uses_pretty_json_with_trailing_newline() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        let playlist =
            json!({"id": "fmt-test", "displayName": "Fmt", "grouping": {"by": "seasonNumber"}});
        repo.save_playlist("test-pattern", "fmt-test", &playlist)
            .unwrap();

        let raw = std::fs::read_to_string(
            tmp.path()
                .join("presets/test-pattern/playlists/fmt-test.json"),
        )
        .unwrap();

        // Should have 2-space indentation
        assert!(raw.contains("  \""));
        // Should end with trailing newline
        assert!(raw.ends_with("}\n"));
    }

    #[test]
    fn delete_playlist_removes_file() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        let path = tmp
            .path()
            .join("presets/test-pattern/playlists/playlist-1.json");
        assert!(path.exists());

        repo.delete_playlist("test-pattern", "playlist-1").unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn delete_playlist_returns_error_for_missing_file() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        let result = repo.delete_playlist("test-pattern", "nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn create_preset_creates_directory_structure() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        let meta = json!({
            "dataVersion": 1,
            "id": "new-pattern",
            "feedUrls": ["https://example.com/new.xml"],
            "playlists": []
        });

        repo.create_preset("new-pattern", &meta).unwrap();

        let preset_dir = tmp.path().join("presets/new-pattern");
        assert!(preset_dir.exists());
        assert!(preset_dir.join("playlists").exists());
        assert!(preset_dir.join("meta.json").exists());

        let loaded = repo.get_preset_meta("new-pattern").unwrap();
        assert_eq!(loaded.id, "new-pattern");
    }

    #[test]
    fn delete_preset_removes_directory() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        let dir = tmp.path().join("presets/test-pattern");
        assert!(dir.exists());

        repo.delete_preset("test-pattern").unwrap();
        assert!(!dir.exists());
    }

    #[test]
    fn path_validation_rejects_traversal() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        let result = repo.get_preset_meta("../etc");
        assert!(result.is_err());
        match result.unwrap_err() {
            Error::InvalidPathSegment { label, value } => {
                assert_eq!(label, "presetId");
                assert_eq!(value, "../etc");
            }
            other => panic!("Expected InvalidPathSegment, got: {other:?}"),
        }
    }

    #[test]
    fn path_validation_rejects_slashes() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        let result = repo.get_playlist("test-pattern", "foo/bar");
        assert!(result.is_err());
    }

    #[test]
    fn path_validation_allows_hyphens_and_underscores() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        // This should not error on validation (may error on file not found)
        let result = repo.get_preset_meta("valid-id_123");
        // Should fail with NotFound, not validation error
        assert!(matches!(result.unwrap_err(), Error::NotFound(_)));
    }

    #[test]
    fn get_root_meta_json_returns_raw_value() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        let json = repo.get_root_meta_json().unwrap();
        assert_eq!(json["schemaVersion"], 3);
        assert!(json["presets"].is_array());
    }

    #[test]
    fn save_root_meta_writes_and_reads_back() {
        let tmp = setup_test_dir();
        let repo = LocalConfigRepository::new(tmp.path());

        let mut json = repo.get_root_meta_json().unwrap();
        json["schemaVersion"] = json!(4);
        repo.save_root_meta(&json).unwrap();

        let reloaded = repo.get_root_meta_json().unwrap();
        assert_eq!(reloaded["schemaVersion"], 4);
    }
}
