use preset_core::schema::{SchemaType, Validator};
use serde_json::json;

fn test_validator() -> Validator {
    let schema_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
    Validator::from_dir(&schema_dir).expect("failed to load schemas")
}

#[test]
fn valid_playlist_definition_passes() {
    let v = test_validator();
    let def = json!({
        "id": "main",
        "displayName": "Main",
        "priority": 0,
        "grouping": { "by": "seasonNumber" }
    });
    let errors = v.validate(SchemaType::PlaylistDefinition, &def);
    assert!(errors.is_empty(), "Expected no errors but got: {errors:?}");
}

#[test]
fn missing_required_field_fails() {
    let v = test_validator();
    // missing id (required)
    let def = json!({
        "displayName": "Main",
        "grouping": { "by": "seasonNumber" }
    });
    let errors = v.validate(SchemaType::PlaylistDefinition, &def);
    assert!(!errors.is_empty());
}

#[test]
fn missing_grouping_fails() {
    let v = test_validator();
    let def = json!({
        "id": "main",
        "displayName": "Main"
    });
    let errors = v.validate(SchemaType::PlaylistDefinition, &def);
    assert!(!errors.is_empty());
}

#[test]
fn invalid_grouping_by_value_fails() {
    let v = test_validator();
    let def = json!({
        "id": "main",
        "displayName": "Main",
        "grouping": { "by": "invalidType" }
    });
    let errors = v.validate(SchemaType::PlaylistDefinition, &def);
    assert!(!errors.is_empty());
}

#[test]
fn valid_preset_meta_passes() {
    let v = test_validator();
    let meta = json!({
        "dataVersion": 1,
        "id": "test-podcast",
        "feedUrls": ["https://example.com/feed.xml"],
        "playlists": ["main"]
    });
    let errors = v.validate(SchemaType::PresetMeta, &meta);
    assert!(errors.is_empty(), "Expected no errors but got: {errors:?}");
}

#[test]
fn valid_preset_index_passes() {
    let v = test_validator();
    let index = json!({
        "dataVersion": 1,
        "schemaVersion": 3,
        "presets": [{
            "id": "test",
            "dataVersion": 1,
            "displayName": "Test Podcast",
            "feedUrlHint": "https://example.com",
            "playlistCount": 1
        }]
    });
    let errors = v.validate(SchemaType::PresetIndex, &index);
    assert!(errors.is_empty(), "Expected no errors but got: {errors:?}");
}

#[test]
fn preset_meta_missing_feed_urls_fails() {
    let v = test_validator();
    let meta = json!({
        "dataVersion": 1,
        "id": "test-podcast",
        "playlists": ["main"]
    });
    let errors = v.validate(SchemaType::PresetMeta, &meta);
    assert!(!errors.is_empty());
}

#[test]
fn preset_index_empty_presets_passes() {
    let v = test_validator();
    let index = json!({
        "dataVersion": 1,
        "schemaVersion": 1,
        "presets": []
    });
    let errors = v.validate(SchemaType::PresetIndex, &index);
    assert!(errors.is_empty(), "Expected no errors but got: {errors:?}");
}

#[test]
fn playlist_definition_with_additional_properties_fails() {
    let v = test_validator();
    let def = json!({
        "id": "main",
        "displayName": "Main",
        "priority": 0,
        "grouping": { "by": "seasonNumber" },
        "unknownField": true
    });
    let errors = v.validate(SchemaType::PlaylistDefinition, &def);
    assert!(!errors.is_empty());
}

#[test]
fn preset_meta_accepts_show_episode_thumbnail() {
    let validator = Validator::from_embedded().unwrap();
    let base = serde_json::json!({
        "dataVersion": 1,
        "id": "abc",
        "feedUrls": ["https://example.com/rss"],
        "playlists": ["p1"]
    });

    let mut with_true = base.clone();
    with_true["showEpisodeThumbnail"] = serde_json::json!(true);
    let errs = validator.validate(SchemaType::PresetMeta, &with_true);
    assert!(errs.is_empty(), "true should be accepted: {:?}", errs);

    let mut with_false = base.clone();
    with_false["showEpisodeThumbnail"] = serde_json::json!(false);
    let errs = validator.validate(SchemaType::PresetMeta, &with_false);
    assert!(errs.is_empty(), "false should be accepted: {:?}", errs);

    let mut with_string = base.clone();
    with_string["showEpisodeThumbnail"] = serde_json::json!("yes");
    let errs = validator.validate(SchemaType::PresetMeta, &with_string);
    assert!(!errs.is_empty(), "string should be rejected");
}

#[test]
fn playlist_definition_accepts_show_thumbnail_everywhere() {
    let validator = Validator::from_embedded().unwrap();
    let doc = serde_json::json!({
        "id": "test",
        "displayName": "Test",
        "priority": 0,
        "grouping": {
            "by": "titleClassifier",
            "staticClassifiers": [
                {
                    "id": "g1",
                    "displayName": "G1",
                    "pattern": { "source": "title", "pattern": ".*" },
                    "groupItem": { "showThumbnail": false },
                    "episodeItem": { "showThumbnail": true }
                }
            ]
        },
        "groupItem": { "showThumbnail": false },
        "episodeItem": { "showThumbnail": false }
    });
    let errs = validator.validate(SchemaType::PlaylistDefinition, &doc);
    assert!(errs.is_empty(), "expected accept, got: {:?}", errs);

    let mut bad = doc.clone();
    bad["groupItem"]["showThumbnail"] = serde_json::json!("nope");
    let errs = validator.validate(SchemaType::PlaylistDefinition, &bad);
    assert!(!errs.is_empty(), "non-boolean must be rejected");
}
