use preset_core::models::*;
use serde_json::json;

// --- JSON round-trip for PlaylistDefinition ---

#[test]
fn playlist_definition_full_round_trip() {
    let json_val = json!({
        "id": "main",
        "displayName": "Main Episodes",
        "grouping": {
            "by": "seasonNumber",
            "numberingExtractor": {
                "source": "title",
                "pattern": "\\[(\\d+)-(\\d+)\\]"
            },
            "staticClassifiers": [
                {
                    "id": "main",
                    "displayName": "Main",
                    "pattern": { "source": "title", "pattern": "^\\[\\d+-\\d+\\]" }
                }
            ]
        },
        "priority": 1,
        "episodeFilters": {
            "require": [{"title": "^\\[\\d+"}],
            "exclude": [{"title": "^\\[bonus"}]
        },
        "groupItem": {
            "titleExtractor": {
                "source": "seasonNumber",
                "template": "Season ${0}"
            },
            "prependSeasonNumber": true,
            "showDateRange": true
        },
        "groupListing": {
            "yearBinding": "pinToYear",
            "userSortable": true,
            "sort": {
                "field": "newestEpisodeDate",
                "order": "descending"
            }
        },
        "episodeListing": {
            "showYearHeaders": true,
            "sort": {
                "field": "episodeNumber",
                "order": "ascending"
            }
        }
    });

    let def: PlaylistDefinition = serde_json::from_value(json_val.clone()).unwrap();
    assert_eq!(def.id, "main");
    assert_eq!(def.priority, 1);
    assert!(
        def.group_item
            .as_ref()
            .unwrap()
            .prepend_season_number
            .unwrap()
    );
    assert!(def.grouping.static_classifiers.is_some());
    assert_eq!(def.grouping.static_classifiers.as_ref().unwrap().len(), 1);

    let serialized = serde_json::to_value(&def).unwrap();
    assert_eq!(serialized["id"], "main");
    assert_eq!(serialized["priority"], 1);
    assert_eq!(serialized["groupListing"]["yearBinding"], "pinToYear");
}

#[test]
fn playlist_definition_minimal_round_trip() {
    let json_val = json!({
        "id": "simple",
        "displayName": "Simple Playlist",
        "priority": 0,
        "grouping": { "by": "seasonNumber" }
    });

    let def: PlaylistDefinition = serde_json::from_value(json_val).unwrap();
    assert_eq!(def.id, "simple");
    assert_eq!(def.priority, 0);
    assert!(def.episode_filters.is_none());
    assert!(def.grouping.static_classifiers.is_none());

    let serialized = serde_json::to_value(&def).unwrap();
    // priority is always serialized now (auto-set from array order)
    assert_eq!(serialized["priority"], 0);
    assert!(serialized.get("episodeFilters").is_none());
}

// --- JSON round-trip for sort types ---

#[test]
fn sort_rule_round_trip() {
    let json_val = json!({
        "field": "newestEpisodeDate",
        "order": "descending"
    });

    let rule: SortRule = serde_json::from_value(json_val).unwrap();
    assert_eq!(rule.field, SortField::NewestEpisodeDate);
    assert_eq!(rule.order, SortOrder::Descending);

    let serialized = serde_json::to_value(&rule).unwrap();
    assert_eq!(serialized["field"], "newestEpisodeDate");
    assert_eq!(serialized["order"], "descending");
}

#[test]
fn episode_sort_rule_round_trip() {
    let json_val = json!({
        "field": "publishedAt",
        "order": "ascending"
    });

    let rule: EpisodeSortRule = serde_json::from_value(json_val).unwrap();
    assert_eq!(rule.field, EpisodeSortField::PublishedAt);
    assert_eq!(rule.order, SortOrder::Ascending);

    let serialized = serde_json::to_value(&rule).unwrap();
    assert_eq!(serialized["field"], "publishedAt");
    assert_eq!(serialized["order"], "ascending");
}

#[test]
fn sort_field_all_variants() {
    let cases = vec![
        ("playlistNumber", SortField::PlaylistNumber),
        ("newestEpisodeDate", SortField::NewestEpisodeDate),
        ("alphabetical", SortField::Alphabetical),
    ];
    for (json_str, expected) in cases {
        let val: SortField = serde_json::from_value(json!(json_str)).unwrap();
        assert_eq!(val, expected);
    }
}

// --- TitleExtractor::extract() tests ---

fn make_episode(title: &str, season: Option<i32>, episode: Option<i32>) -> SimpleEpisodeData {
    SimpleEpisodeData {
        id: 1,
        title: title.to_string(),
        description: None,
        season_number: season,
        episode_number: episode,
        published_at: None,
        image_url: None,
    }
}

#[test]
fn title_extractor_pattern_matching() {
    let extractor: TitleExtractor = serde_json::from_value(json!({
        "source": "title",
        "pattern": "\\[(.+?)\\s+\\d+\\]",
        "template": "${1}"
    }))
    .unwrap();

    let ep = make_episode("[History 42] The Roman Empire", None, None);
    let result = extractor.extract(&ep);
    assert_eq!(result, Some("History".to_string()));
}

#[test]
fn title_extractor_template() {
    let extractor: TitleExtractor = serde_json::from_value(json!({
        "source": "seasonNumber",
        "template": "Season ${0}"
    }))
    .unwrap();

    let ep = make_episode("Episode 1", Some(3), None);
    let result = extractor.extract(&ep);
    assert_eq!(result, Some("Season 3".to_string()));
}

#[test]
fn title_extractor_fallback_chain() {
    let extractor: TitleExtractor = serde_json::from_value(json!({
        "source": "title",
        "pattern": "\\[(.+?)\\]",
        "template": "${1}",
        "fallback": {
            "source": "seasonNumber",
            "template": "Season ${0}"
        }
    }))
    .unwrap();

    // Primary matches
    let ep1 = make_episode("[Bonus] Special", Some(1), None);
    assert_eq!(extractor.extract(&ep1), Some("Bonus".to_string()));

    // Primary fails, fallback used
    let ep2 = make_episode("No brackets here", Some(5), None);
    assert_eq!(extractor.extract(&ep2), Some("Season 5".to_string()));
}

#[test]
fn title_extractor_fallback_value_for_null_season() {
    let extractor: TitleExtractor = serde_json::from_value(json!({
        "source": "seasonNumber",
        "template": "Season ${0}",
        "fallbackValue": "Uncategorized"
    }))
    .unwrap();

    // Null season -> fallback value
    let ep1 = make_episode("Episode 1", None, None);
    assert_eq!(extractor.extract(&ep1), Some("Uncategorized".to_string()));

    // Zero season -> fallback value
    let ep2 = make_episode("Episode 2", Some(0), None);
    assert_eq!(extractor.extract(&ep2), Some("Uncategorized".to_string()));

    // Valid season -> template
    let ep3 = make_episode("Episode 3", Some(2), None);
    assert_eq!(extractor.extract(&ep3), Some("Season 2".to_string()));
}

#[test]
fn title_extractor_no_match_no_fallback() {
    let extractor: TitleExtractor = serde_json::from_value(json!({
        "source": "title",
        "pattern": "NOMATCH"
    }))
    .unwrap();

    let ep = make_episode("Regular Title", None, None);
    assert_eq!(extractor.extract(&ep), None);
}

// --- NumberingExtractor::extract() tests ---

#[test]
fn episode_extractor_primary_pattern() {
    let extractor: NumberingExtractor = serde_json::from_value(json!({
        "source": "title",
        "pattern": "\\[(\\d+)-(\\d+)\\]"
    }))
    .unwrap();

    let ep = make_episode("[62-15] The Topic", None, None);
    let result = extractor.extract(&ep);
    assert_eq!(result.season_number, Some(62));
    assert_eq!(result.episode_number, Some(15));
}

#[test]
fn episode_extractor_fallback_pattern() {
    let extractor: NumberingExtractor = serde_json::from_value(json!({
        "source": "title",
        "pattern": "\\[(\\d+)-(\\d+)\\]",
        "fallbackSeasonNumber": 0,
        "fallbackEpisodePattern": "#(\\d+)"
    }))
    .unwrap();

    // Primary fails, fallback matches
    let ep = make_episode("Special #135 - Bonus", None, None);
    let result = extractor.extract(&ep);
    assert_eq!(result.season_number, Some(0));
    assert_eq!(result.episode_number, Some(135));
}

#[test]
fn episode_extractor_rss_fallback() {
    let extractor: NumberingExtractor = serde_json::from_value(json!({
        "source": "title",
        "pattern": "\\[(\\d+)-(\\d+)\\]",
        "fallbackToRss": true
    }))
    .unwrap();

    let ep = make_episode("No pattern here", None, Some(42));
    let result = extractor.extract(&ep);
    assert_eq!(result.season_number, None);
    assert_eq!(result.episode_number, Some(42));
}

#[test]
fn episode_extractor_no_match_no_fallback() {
    let extractor: NumberingExtractor = serde_json::from_value(json!({
        "source": "title",
        "pattern": "\\[(\\d+)-(\\d+)\\]"
    }))
    .unwrap();

    let ep = make_episode("No pattern", None, Some(10));
    let result = extractor.extract(&ep);
    assert_eq!(result.season_number, None);
    assert_eq!(result.episode_number, None);
    assert!(!result.has_values());
}

#[test]
fn episode_extractor_null_season_group() {
    let extractor: NumberingExtractor = serde_json::from_value(json!({
        "source": "title",
        "pattern": "#(\\d+)",
        "seasonGroup": null,
        "episodeGroup": 1
    }))
    .unwrap();

    assert!(extractor.season_group.is_none());

    let ep = make_episode("Episode #42", None, None);
    let result = extractor.extract(&ep);
    assert_eq!(result.season_number, None);
    assert_eq!(result.episode_number, Some(42));
}

// --- PlaylistDefinition::has_filters() ---

#[test]
fn has_filters_with_require() {
    let def: PlaylistDefinition = serde_json::from_value(json!({
        "id": "test",
        "displayName": "Test",
        "priority": 0,
        "grouping": { "by": "seasonNumber" },
        "episodeFilters": {
            "require": [{"title": "pattern"}]
        }
    }))
    .unwrap();
    assert!(def.has_filters());
}

#[test]
fn has_filters_with_exclude() {
    let def: PlaylistDefinition = serde_json::from_value(json!({
        "id": "test",
        "displayName": "Test",
        "priority": 0,
        "grouping": { "by": "seasonNumber" },
        "episodeFilters": {
            "exclude": [{"title": "bonus"}]
        }
    }))
    .unwrap();
    assert!(def.has_filters());
}

#[test]
fn has_filters_empty() {
    let def: PlaylistDefinition = serde_json::from_value(json!({
        "id": "test",
        "displayName": "Test",
        "priority": 0,
        "grouping": { "by": "seasonNumber" },
        "episodeFilters": {}
    }))
    .unwrap();
    assert!(!def.has_filters());
}

#[test]
fn has_filters_none() {
    let def: PlaylistDefinition = serde_json::from_value(json!({
        "id": "test",
        "displayName": "Test",
        "priority": 0,
        "grouping": { "by": "seasonNumber" }
    }))
    .unwrap();
    assert!(!def.has_filters());
}

// --- CompiledFilters OR semantics ---

/// Helper: build a minimal resolver service + config and run filter_episodes
/// to test require/exclude OR semantics end-to-end through ResolverService.
mod filter_semantics {
    use chrono::{TimeZone, Utc};
    use preset_core::models::*;
    use preset_core::services::resolver_service::ResolverService;

    fn ep(id: i64, title: &str) -> SimpleEpisodeData {
        SimpleEpisodeData {
            id,
            title: title.to_string(),
            description: None,
            season_number: None,
            episode_number: None,
            published_at: Some(Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap()),
            image_url: None,
        }
    }

    /// Build a config with one definition that has the given filters, then
    /// resolve and return which episode IDs survived filtering.
    fn filtered_ids(
        require: Option<Vec<EpisodeFilterEntry>>,
        exclude: Option<Vec<EpisodeFilterEntry>>,
        episodes: &[SimpleEpisodeData],
    ) -> Vec<i64> {
        let definition = PlaylistDefinition {
            id: "test".to_string(),
            display_name: "Test".to_string(),
            grouping: GroupingConfig {
                by: "year".to_string(),
                numbering_extractor: None,
                static_classifiers: None,
            },
            selector: None,
            priority: 0,
            episode_filters: Some(EpisodeFilters { require, exclude }),
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
        };

        let config = PresetConfig {
            id: "p".to_string(),
            podcast_guid: None,
            feed_urls: Some(vec!["https://example.com/feed".to_string()]),
            year_grouped_episodes: false,
            show_episode_thumbnail: None,
            playlists: vec![definition],
        };

        let resolvers: Vec<Box<dyn preset_core::resolvers::Resolver>> =
            vec![Box::new(preset_core::resolvers::YearResolver)];
        let service = ResolverService::new(resolvers, vec![config]);

        let ep_refs: Vec<&dyn EpisodeData> =
            episodes.iter().map(|e| e as &dyn EpisodeData).collect();
        match service.resolve_smart_playlists(None, "https://example.com/feed", &ep_refs) {
            Some(grouping) => {
                let mut ids: Vec<i64> = grouping
                    .playlists
                    .iter()
                    .flat_map(|p| &p.episode_ids)
                    .copied()
                    .collect();
                ids.sort();
                ids
            }
            None => vec![],
        }
    }

    #[test]
    fn require_or_semantics_any_entry_matches() {
        // Matches the schema contract: `require` across entries is OR.
        // An episode is included when ANY rule matches; each rule's
        // own fields are still AND internally.
        let episodes = vec![
            ep(1, "alpha episode"),
            ep(2, "beta episode"),
            ep(3, "alpha standalone"),
            ep(4, "gamma"),
        ];

        let require = Some(vec![
            EpisodeFilterEntry {
                title: Some("alpha".to_string()),
                description: None,
            },
            EpisodeFilterEntry {
                title: Some("episode".to_string()),
                description: None,
            },
        ]);

        let ids = filtered_ids(require, None, &episodes);
        assert!(ids.contains(&1), "alpha episode matches both entries");
        assert!(ids.contains(&2), "beta episode matches the second entry");
        assert!(ids.contains(&3), "alpha standalone matches the first entry");
        assert!(!ids.contains(&4), "gamma matches no require entry");
    }

    #[test]
    fn require_no_match_excludes_episode() {
        let episodes = vec![ep(1, "unrelated title")];

        let require = Some(vec![EpisodeFilterEntry {
            title: Some("alpha".to_string()),
            description: None,
        }]);

        let ids = filtered_ids(require, None, &episodes);
        assert!(
            ids.is_empty(),
            "episode matching no require entry should be excluded"
        );
    }

    #[test]
    fn exclude_or_semantics_any_entry_rejects() {
        let episodes = vec![
            ep(1, "alpha episode"),
            ep(2, "beta episode"),
            ep(3, "gamma episode"),
        ];

        let exclude = Some(vec![
            EpisodeFilterEntry {
                title: Some("alpha".to_string()),
                description: None,
            },
            EpisodeFilterEntry {
                title: Some("beta".to_string()),
                description: None,
            },
        ]);

        let ids = filtered_ids(None, exclude, &episodes);
        assert!(!ids.contains(&1), "alpha should be excluded");
        assert!(!ids.contains(&2), "beta should be excluded");
        assert!(ids.contains(&3), "gamma should survive");
    }

    #[test]
    fn require_and_exclude_combined() {
        let episodes = vec![
            ep(1, "special alpha episode"),
            ep(2, "special beta episode"),
            ep(3, "gamma episode"),
            ep(4, "nothing relevant"),
        ];

        let require = Some(vec![
            EpisodeFilterEntry {
                title: Some("special".to_string()),
                description: None,
            },
            EpisodeFilterEntry {
                title: Some("episode".to_string()),
                description: None,
            },
        ]);
        let exclude = Some(vec![EpisodeFilterEntry {
            title: Some("beta".to_string()),
            description: None,
        }]);

        let ids = filtered_ids(require, exclude, &episodes);
        assert!(
            ids.contains(&1),
            "special alpha episode matches both require entries and is not excluded"
        );
        assert!(
            !ids.contains(&2),
            "special beta episode matches require but is excluded by beta"
        );
        assert!(
            ids.contains(&3),
            "gamma episode matches the second require entry (OR) and is not excluded"
        );
        assert!(
            !ids.contains(&4),
            "nothing relevant matches no require entry"
        );
    }
}

// --- PresetConfig::matches_podcast() ---

#[test]
fn preset_config_matches_by_guid() {
    let config: PresetConfig = serde_json::from_value(json!({
        "id": "test",
        "podcastGuid": "abc-123",
        "feedUrls": ["https://example.com/feed"],
        "playlists": []
    }))
    .unwrap();

    assert!(config.matches_podcast(Some("abc-123"), "https://other.com/feed"));
    assert!(!config.matches_podcast(Some("wrong-guid"), "https://other.com/feed"));
}

#[test]
fn preset_config_matches_by_url() {
    let config: PresetConfig = serde_json::from_value(json!({
        "id": "test",
        "feedUrls": ["https://example.com/feed"],
        "playlists": []
    }))
    .unwrap();

    assert!(config.matches_podcast(None, "https://example.com/feed"));
    assert!(!config.matches_podcast(None, "https://other.com/feed"));
}

#[test]
fn preset_config_no_match() {
    let config: PresetConfig = serde_json::from_value(json!({
        "id": "test",
        "playlists": []
    }))
    .unwrap();

    assert!(!config.matches_podcast(Some("any-guid"), "https://any.com/feed"));
}

#[test]
fn preset_config_empty_guid_treated_as_none() {
    let config: PresetConfig = serde_json::from_value(json!({
        "id": "test",
        "podcastGuid": "",
        "playlists": []
    }))
    .unwrap();

    assert!(config.podcast_guid.is_none());
}

// --- JSON round-trip for PresetMeta ---

#[test]
fn preset_meta_round_trip() {
    let json_val = json!({
        "dataVersion": 2,
        "id": "coten",
        "podcastGuid": "guid-123",
        "feedUrls": ["https://feed.example.com/rss"],
        "yearGroupedEpisodes": true,
        "playlists": ["main", "bonus"]
    });

    let meta: PresetMeta = serde_json::from_value(json_val).unwrap();
    assert_eq!(meta.data_version, 2);
    assert_eq!(meta.id, "coten");
    assert!(meta.year_grouped_episodes);
    assert_eq!(meta.playlists.len(), 2);

    let serialized = serde_json::to_value(&meta).unwrap();
    assert_eq!(serialized["dataVersion"], 2);
    assert_eq!(serialized["yearGroupedEpisodes"], true);
}

#[test]
fn preset_meta_defaults() {
    let json_val = json!({
        "id": "simple",
        "feedUrls": ["https://example.com/feed"],
        "playlists": ["main"]
    });

    let meta: PresetMeta = serde_json::from_value(json_val).unwrap();
    assert_eq!(meta.data_version, 1);
    assert!(!meta.year_grouped_episodes);

    let serialized = serde_json::to_value(&meta).unwrap();
    // yearGroupedEpisodes should be omitted when false
    assert!(serialized.get("yearGroupedEpisodes").is_none());
}

// --- JSON round-trip for RootMeta ---

#[test]
fn root_meta_round_trip() {
    let json_val = json!({
        "dataVersion": 1,
        "schemaVersion": 3,
        "presets": [
            {
                "id": "coten",
                "dataVersion": 1,
                "displayName": "COTEN RADIO",
                "feedUrlHint": "https://feed.example.com/rss",
                "playlistCount": 3
            }
        ]
    });

    let meta: RootMeta = serde_json::from_value(json_val).unwrap();
    assert_eq!(meta.data_version, 1);
    assert_eq!(meta.schema_version, 3);
    assert_eq!(meta.presets.len(), 1);
    assert_eq!(meta.presets[0].display_name, "COTEN RADIO");

    let serialized = serde_json::to_value(&meta).unwrap();
    assert_eq!(serialized["schemaVersion"], 3);
    assert_eq!(serialized["presets"][0]["displayName"], "COTEN RADIO");
}

// --- JSON round-trip for PresetSummary ---

#[test]
fn preset_summary_round_trip() {
    let json_val = json!({
        "id": "coten",
        "dataVersion": 2,
        "displayName": "COTEN RADIO",
        "feedUrlHint": "https://feed.example.com/rss",
        "playlistCount": 5
    });

    let summary: PresetSummary = serde_json::from_value(json_val).unwrap();
    assert_eq!(summary.id, "coten");
    assert_eq!(summary.data_version, 2);
    assert_eq!(summary.playlist_count, 5);

    let serialized = serde_json::to_value(&summary).unwrap();
    assert_eq!(serialized["feedUrlHint"], "https://feed.example.com/rss");
}

// --- NumberingExtractor JSON serialization ---

#[test]
fn episode_extractor_serialization_omits_defaults() {
    let extractor: NumberingExtractor = serde_json::from_value(json!({
        "source": "title",
        "pattern": "\\[(\\d+)-(\\d+)\\]"
    }))
    .unwrap();

    assert_eq!(extractor.season_group, Some(1));
    assert_eq!(extractor.episode_group, 2);
    assert_eq!(extractor.fallback_episode_capture_group, 1);
    assert!(!extractor.fallback_to_rss);

    let serialized = serde_json::to_value(&extractor).unwrap();
    assert_eq!(serialized["seasonGroup"], 1);
    assert_eq!(serialized["episodeGroup"], 2);
    assert!(serialized.get("fallbackEpisodeCaptureGroup").is_none());
    assert!(serialized.get("fallbackToRss").is_none());
}

#[test]
fn episode_extractor_serialization_includes_non_defaults() {
    let extractor: NumberingExtractor = serde_json::from_value(json!({
        "source": "title",
        "pattern": "\\[(\\d+)-(\\d+)\\]",
        "seasonGroup": null,
        "episodeGroup": 1,
        "fallbackToRss": true,
        "fallbackEpisodeCaptureGroup": 2
    }))
    .unwrap();

    let serialized = serde_json::to_value(&extractor).unwrap();
    assert!(serialized.get("seasonGroup").is_none());
    assert_eq!(serialized["episodeGroup"], 1);
    assert_eq!(serialized["fallbackToRss"], true);
    assert_eq!(serialized["fallbackEpisodeCaptureGroup"], 2);
}

// --- TitleExtractor JSON round-trip ---

#[test]
fn title_extractor_json_round_trip() {
    let json_val = json!({
        "source": "title",
        "pattern": "\\[(.+?)\\]",
        "template": "${1}",
        "fallback": {
            "source": "seasonNumber",
            "template": "Season ${0}"
        }
    });

    let extractor: TitleExtractor = serde_json::from_value(json_val).unwrap();
    assert_eq!(extractor.source, "title");
    assert_eq!(extractor.template.as_deref(), Some("${1}"));
    assert!(extractor.fallback.is_some());

    let serialized = serde_json::to_value(&extractor).unwrap();
    assert_eq!(serialized["source"], "title");
    assert_eq!(serialized["template"], "${1}");
    assert_eq!(serialized["fallback"]["source"], "seasonNumber");
    assert_eq!(serialized["fallback"]["template"], "Season ${0}");
}

#[test]
fn title_extractor_omits_defaults() {
    let json_val = json!({ "source": "title" });

    let extractor: TitleExtractor = serde_json::from_value(json_val).unwrap();
    assert!(extractor.template.is_none());
    assert!(extractor.pattern.is_none());

    let serialized = serde_json::to_value(&extractor).unwrap();
    assert!(serialized.get("pattern").is_none());
    assert!(serialized.get("template").is_none());
    assert!(serialized.get("fallback").is_none());
    assert!(serialized.get("fallbackValue").is_none());
}

// --- Playlist and PlaylistGroup output model tests ---

#[test]
fn presentation_enum_serialization() {
    assert_eq!(
        serde_json::to_value(Presentation::Separate).unwrap(),
        json!("separate")
    );
    assert_eq!(
        serde_json::to_value(Presentation::Combined).unwrap(),
        json!("combined")
    );
}

#[test]
fn year_binding_enum_serialization() {
    assert_eq!(
        serde_json::to_value(YearBinding::None).unwrap(),
        json!("none")
    );
    assert_eq!(
        serde_json::to_value(YearBinding::PinToYear).unwrap(),
        json!("pinToYear")
    );
    assert_eq!(
        serde_json::to_value(YearBinding::SplitByYear).unwrap(),
        json!("splitByYear")
    );
}

// --- PresetConfig JSON round-trip ---

#[test]
fn preset_config_round_trip() {
    let json_val = json!({
        "id": "test-pattern",
        "podcastGuid": "guid-abc",
        "feedUrls": ["https://example.com/feed"],
        "yearGroupedEpisodes": true,
        "playlists": [
            {
                "id": "main",
                "displayName": "Main",
                "priority": 0,
                "grouping": { "by": "seasonNumber" }
            }
        ]
    });

    let config: PresetConfig = serde_json::from_value(json_val).unwrap();
    assert_eq!(config.id, "test-pattern");
    assert!(config.year_grouped_episodes);
    assert_eq!(config.playlists.len(), 1);

    let serialized = serde_json::to_value(&config).unwrap();
    assert_eq!(serialized["podcastGuid"], "guid-abc");
    assert_eq!(serialized["yearGroupedEpisodes"], true);
}

#[test]
fn preset_config_omits_defaults() {
    let json_val = json!({
        "id": "minimal",
        "playlists": []
    });

    let config: PresetConfig = serde_json::from_value(json_val).unwrap();
    assert!(!config.year_grouped_episodes);
    assert!(config.podcast_guid.is_none());

    let serialized = serde_json::to_value(&config).unwrap();
    assert!(serialized.get("yearGroupedEpisodes").is_none());
    assert!(serialized.get("podcastGuid").is_none());
    assert!(serialized.get("feedUrls").is_none());
}
