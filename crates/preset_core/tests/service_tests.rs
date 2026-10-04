use std::collections::HashMap;

use chrono::{TimeZone, Utc};

use preset_core::models::{
    EpisodeFilterEntry, EpisodeFilters, GroupListingConfig, GroupingConfig, PlaylistDefinition,
    PlaylistGroup, Presentation, PresetConfig, PresetMeta, SimpleEpisodeData, SortField, SortOrder,
    SortRule, YearBinding,
};
use preset_core::resolvers::{RssResolver, YearResolver};
use preset_core::services::{
    ConfigAssembler, ResolverService, sort_episode_ids_by_published_at, sort_groups,
};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_episode_with_title(id: i64, title: &str, day: u32, month: u32) -> SimpleEpisodeData {
    SimpleEpisodeData {
        id,
        title: title.to_string(),
        description: None,
        season_number: None,
        episode_number: None,
        published_at: Some(Utc.with_ymd_and_hms(2024, month, day, 0, 0, 0).unwrap()),
        image_url: None,
    }
}

// ===========================================================================
// episode_sorter tests
// ===========================================================================

#[test]
fn episode_sorter_sorts_by_published_at_ascending() {
    let eps = [
        SimpleEpisodeData {
            id: 1,
            title: "A".into(),
            description: None,
            season_number: None,
            episode_number: None,
            published_at: Some(Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap()),
            image_url: None,
        },
        SimpleEpisodeData {
            id: 2,
            title: "B".into(),
            description: None,
            season_number: None,
            episode_number: None,
            published_at: Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()),
            image_url: None,
        },
        SimpleEpisodeData {
            id: 3,
            title: "C".into(),
            description: None,
            season_number: None,
            episode_number: None,
            published_at: Some(Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap()),
            image_url: None,
        },
    ];

    let episode_map: HashMap<i64, &dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| (e.id, e as &dyn preset_core::models::EpisodeData))
        .collect();

    let result = sort_episode_ids_by_published_at(&[1, 2, 3], &episode_map);
    assert_eq!(result, vec![2, 3, 1]); // Jan, Feb, Mar
}

#[test]
fn episode_sorter_null_dates_sort_after_dated_episodes() {
    let eps = [
        SimpleEpisodeData {
            id: 1,
            title: "A".into(),
            description: None,
            season_number: None,
            episode_number: None,
            published_at: None,
            image_url: None,
        },
        SimpleEpisodeData {
            id: 2,
            title: "B".into(),
            description: None,
            season_number: None,
            episode_number: None,
            published_at: Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()),
            image_url: None,
        },
    ];

    let episode_map: HashMap<i64, &dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| (e.id, e as &dyn preset_core::models::EpisodeData))
        .collect();

    let result = sort_episode_ids_by_published_at(&[1, 2], &episode_map);
    assert_eq!(result, vec![2, 1]); // dated first, null second
}

#[test]
fn episode_sorter_unknown_ids_sort_last() {
    let eps = [SimpleEpisodeData {
        id: 1,
        title: "A".into(),
        description: None,
        season_number: None,
        episode_number: None,
        published_at: Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()),
        image_url: None,
    }];

    let episode_map: HashMap<i64, &dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| (e.id, e as &dyn preset_core::models::EpisodeData))
        .collect();

    // id 99 not in map
    let result = sort_episode_ids_by_published_at(&[99, 1], &episode_map);
    assert_eq!(result, vec![1, 99]); // known first, unknown last
}

#[test]
fn episode_sorter_empty_list_returned_as_is() {
    let episode_map: HashMap<i64, &dyn preset_core::models::EpisodeData> = HashMap::new();
    let result = sort_episode_ids_by_published_at(&[], &episode_map);
    assert!(result.is_empty());
}

#[test]
fn episode_sorter_single_item_returned_as_is() {
    let episode_map: HashMap<i64, &dyn preset_core::models::EpisodeData> = HashMap::new();
    let result = sort_episode_ids_by_published_at(&[42], &episode_map);
    assert_eq!(result, vec![42]);
}

// ===========================================================================
// group_sorter tests
// ===========================================================================

fn make_group(id: &str, name: &str, sort_key: i32, episode_ids: Vec<i64>) -> PlaylistGroup {
    PlaylistGroup {
        id: id.to_string(),
        display_name: name.to_string(),
        sort_key,
        episode_ids,
        thumbnail_url: None,
        year_override: None,
        show_year_headers: None,
        show_date_range: false,
        earliest_date: None,
        latest_date: None,
        total_duration_ms: None,
        sub_groups: None,
    }
}

#[test]
fn group_sorter_sort_by_playlist_number() {
    let groups = vec![
        make_group("b", "Beta", 2, vec![]),
        make_group("a", "Alpha", 1, vec![]),
        make_group("c", "Charlie", 3, vec![]),
    ];

    let episode_map: HashMap<i64, &dyn preset_core::models::EpisodeData> = HashMap::new();
    let rule = SortRule {
        field: SortField::PlaylistNumber,
        order: SortOrder::Ascending,
    };

    let sorted = sort_groups(&groups, Some(&rule), &episode_map);
    assert_eq!(sorted[0].id, "a");
    assert_eq!(sorted[1].id, "b");
    assert_eq!(sorted[2].id, "c");
}

#[test]
fn group_sorter_sort_by_newest_episode_date() {
    let ep1 = SimpleEpisodeData {
        id: 1,
        title: "A".into(),
        description: None,
        season_number: None,
        episode_number: None,
        published_at: Some(Utc.with_ymd_and_hms(2024, 3, 1, 0, 0, 0).unwrap()),
        image_url: None,
    };
    let ep2 = SimpleEpisodeData {
        id: 2,
        title: "B".into(),
        description: None,
        season_number: None,
        episode_number: None,
        published_at: Some(Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap()),
        image_url: None,
    };

    let episode_map: HashMap<i64, &dyn preset_core::models::EpisodeData> = [
        (1i64, &ep1 as &dyn preset_core::models::EpisodeData),
        (2i64, &ep2 as &dyn preset_core::models::EpisodeData),
    ]
    .into_iter()
    .collect();

    let groups = vec![
        make_group("newer", "Newer", 0, vec![2]),
        make_group("older", "Older", 0, vec![1]),
    ];

    let rule = SortRule {
        field: SortField::NewestEpisodeDate,
        order: SortOrder::Ascending,
    };

    let sorted = sort_groups(&groups, Some(&rule), &episode_map);
    assert_eq!(sorted[0].id, "older");
    assert_eq!(sorted[1].id, "newer");
}

#[test]
fn group_sorter_sort_alphabetical() {
    let groups = vec![
        make_group("c", "Charlie", 0, vec![]),
        make_group("a", "Alpha", 0, vec![]),
        make_group("b", "Beta", 0, vec![]),
    ];

    let episode_map: HashMap<i64, &dyn preset_core::models::EpisodeData> = HashMap::new();
    let rule = SortRule {
        field: SortField::Alphabetical,
        order: SortOrder::Ascending,
    };

    let sorted = sort_groups(&groups, Some(&rule), &episode_map);
    assert_eq!(sorted[0].id, "a");
    assert_eq!(sorted[1].id, "b");
    assert_eq!(sorted[2].id, "c");
}

#[test]
fn group_sorter_descending_reverses() {
    let groups = vec![
        make_group("a", "Alpha", 1, vec![]),
        make_group("b", "Beta", 2, vec![]),
    ];

    let episode_map: HashMap<i64, &dyn preset_core::models::EpisodeData> = HashMap::new();
    let rule = SortRule {
        field: SortField::PlaylistNumber,
        order: SortOrder::Descending,
    };

    let sorted = sort_groups(&groups, Some(&rule), &episode_map);
    assert_eq!(sorted[0].id, "b"); // 2 first in descending
    assert_eq!(sorted[1].id, "a");
}

#[test]
fn group_sorter_null_rule_returns_unchanged() {
    let groups = vec![
        make_group("b", "Beta", 2, vec![]),
        make_group("a", "Alpha", 1, vec![]),
    ];

    let episode_map: HashMap<i64, &dyn preset_core::models::EpisodeData> = HashMap::new();
    let sorted = sort_groups(&groups, None, &episode_map);
    assert_eq!(sorted[0].id, "b"); // unchanged
    assert_eq!(sorted[1].id, "a");
}

#[test]
fn group_sorter_single_group_returns_unchanged() {
    let groups = vec![make_group("a", "Alpha", 1, vec![])];

    let episode_map: HashMap<i64, &dyn preset_core::models::EpisodeData> = HashMap::new();
    let rule = SortRule {
        field: SortField::PlaylistNumber,
        order: SortOrder::Ascending,
    };

    let sorted = sort_groups(&groups, Some(&rule), &episode_map);
    assert_eq!(sorted.len(), 1);
    assert_eq!(sorted[0].id, "a");
}

// ===========================================================================
// config_assembler tests
// ===========================================================================

fn make_definition(id: &str) -> PlaylistDefinition {
    PlaylistDefinition {
        id: id.to_string(),
        display_name: id.to_string(),
        grouping: GroupingConfig {
            by: "seasonNumber".to_string(),
            numbering_extractor: None,
            static_classifiers: None,
        },
        selector: None,
        priority: 0,
        episode_filters: None,
        group_listing: None,
        group_item: None,
        episode_listing: None,
        episode_item: None,
    }
}

#[test]
fn config_assembler_orders_by_meta_playlists() {
    let meta = PresetMeta {
        data_version: 1,
        id: "test".to_string(),
        podcast_guid: None,
        feed_urls: vec!["https://example.com".to_string()],
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec!["b".to_string(), "a".to_string()],
    };

    let playlists = vec![make_definition("a"), make_definition("b")];

    let config = ConfigAssembler::assemble(&meta, &playlists);
    assert_eq!(config.playlists[0].id, "b");
    assert_eq!(config.playlists[1].id, "a");
}

#[test]
fn config_assembler_appends_unlisted_playlists() {
    let meta = PresetMeta {
        data_version: 1,
        id: "test".to_string(),
        podcast_guid: None,
        feed_urls: vec!["https://example.com".to_string()],
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec!["a".to_string()],
    };

    let playlists = vec![
        make_definition("a"),
        make_definition("b"),
        make_definition("c"),
    ];

    let config = ConfigAssembler::assemble(&meta, &playlists);
    assert_eq!(config.playlists[0].id, "a");
    // b and c appended (order of HashMap iteration is not guaranteed)
    assert_eq!(config.playlists.len(), 3);
    let remaining_ids: Vec<&str> = config.playlists[1..]
        .iter()
        .map(|p| p.id.as_str())
        .collect();
    assert!(remaining_ids.contains(&"b"));
    assert!(remaining_ids.contains(&"c"));
}

// ===========================================================================
// resolver_service tests
// ===========================================================================

fn make_resolver_service(patterns: Vec<PresetConfig>) -> ResolverService {
    ResolverService::new(
        vec![Box::new(RssResolver), Box::new(YearResolver)],
        patterns,
    )
}

fn make_rss_episode(id: i64, season: i32, title: &str, month: u32, day: u32) -> SimpleEpisodeData {
    SimpleEpisodeData {
        id,
        title: title.to_string(),
        description: None,
        season_number: Some(season),
        episode_number: None,
        published_at: Some(Utc.with_ymd_and_hms(2024, month, day, 0, 0, 0).unwrap()),
        image_url: None,
    }
}

#[test]
fn resolver_returns_none_when_no_resolver_succeeds() {
    let service = make_resolver_service(vec![]);

    // Episodes with no season and no date -- neither resolver can group
    let eps = [
        SimpleEpisodeData {
            id: 1,
            title: "A".into(),
            description: None,
            season_number: None,
            episode_number: None,
            published_at: None,
            image_url: None,
        },
        SimpleEpisodeData {
            id: 2,
            title: "B".into(),
            description: None,
            season_number: None,
            episode_number: None,
            published_at: None,
            image_url: None,
        },
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service.resolve_smart_playlists(None, "https://example.com/feed", &refs);
    assert!(result.is_none());
}

#[test]
fn resolver_returns_none_when_episodes_empty() {
    let service = make_resolver_service(vec![]);
    let refs: Vec<&dyn preset_core::models::EpisodeData> = vec![];

    let result = service.resolve_smart_playlists(None, "https://example.com/feed", &refs);
    assert!(result.is_none());
}

#[test]
fn resolver_uses_first_successful_resolver() {
    let service = make_resolver_service(vec![]);

    let eps = [
        make_rss_episode(1, 1, "S1E1", 1, 1),
        make_rss_episode(2, 1, "S1E2", 2, 1),
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service.resolve_smart_playlists(None, "https://example.com/feed", &refs);
    assert!(result.is_some());
    assert_eq!(result.unwrap().resolver_type, "seasonNumber");
}

#[test]
fn resolver_falls_back_to_next_resolver() {
    let service = make_resolver_service(vec![]);

    // No season numbers, but has dates -> year resolver
    let eps = [
        make_episode_with_title(1, "Ep 1", 1, 6), // June
        make_episode_with_title(2, "Ep 2", 1, 3), // March 2024
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service.resolve_smart_playlists(None, "https://example.com/feed", &refs);
    assert!(result.is_some());
    assert_eq!(result.unwrap().resolver_type, "year");
}

#[test]
fn resolver_matches_config_by_feed_url() {
    let service = make_resolver_service(vec![PresetConfig {
        id: "test".to_string(),
        podcast_guid: None,
        feed_urls: Some(vec!["https://example.com/feed.rss".to_string()]),
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec![PlaylistDefinition {
            id: "main".to_string(),
            display_name: "Main".to_string(),
            grouping: GroupingConfig {
                by: "seasonNumber".to_string(),
                numbering_extractor: None,
                static_classifiers: None,
            },
            selector: None,
            priority: 0,
            episode_filters: None,
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
        }],
    }]);

    let eps = [
        make_rss_episode(1, 1, "S1E1", 1, 1),
        make_rss_episode(2, 1, "S1E2", 1, 2),
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service.resolve_smart_playlists(None, "https://example.com/feed.rss", &refs);
    assert!(result.is_some());
    assert_eq!(result.unwrap().resolver_type, "seasonNumber");
}

#[test]
fn resolver_matches_config_by_guid() {
    let service = make_resolver_service(vec![PresetConfig {
        id: "test".to_string(),
        podcast_guid: Some("test-guid".to_string()),
        feed_urls: None,
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec![PlaylistDefinition {
            id: "main".to_string(),
            display_name: "Main".to_string(),
            grouping: GroupingConfig {
                by: "seasonNumber".to_string(),
                numbering_extractor: None,
                static_classifiers: None,
            },
            selector: None,
            priority: 0,
            episode_filters: None,
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
        }],
    }]);

    let eps = [
        make_rss_episode(1, 1, "S1E1", 1, 1),
        make_rss_episode(2, 1, "S1E2", 1, 2),
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result =
        service.resolve_smart_playlists(Some("test-guid"), "https://other.com/feed", &refs);
    assert!(result.is_some());
    assert_eq!(result.unwrap().resolver_type, "seasonNumber");
}

#[test]
fn resolver_filters_by_require_regex() {
    let service = make_resolver_service(vec![PresetConfig {
        id: "test".to_string(),
        podcast_guid: None,
        feed_urls: Some(vec!["https://example.com/feed".to_string()]),
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec![
            PlaylistDefinition {
                id: "bonus".to_string(),
                display_name: "Bonus".to_string(),
                grouping: GroupingConfig {
                    by: "year".to_string(),
                    numbering_extractor: None,
                    static_classifiers: None,
                },
                selector: None,
                priority: 10,
                episode_filters: Some(EpisodeFilters {
                    require: Some(vec![EpisodeFilterEntry {
                        title: Some("Bonus".to_string()),
                        description: None,
                    }]),
                    exclude: None,
                }),
                group_listing: None,
                group_item: None,
                episode_listing: None,
                episode_item: None,
            },
            PlaylistDefinition {
                id: "main".to_string(),
                display_name: "Main".to_string(),
                grouping: GroupingConfig {
                    by: "year".to_string(),
                    numbering_extractor: None,
                    static_classifiers: None,
                },
                selector: None,
                priority: 0,
                episode_filters: Some(EpisodeFilters {
                    require: None,
                    exclude: Some(vec![EpisodeFilterEntry {
                        title: Some("Bonus".to_string()),
                        description: None,
                    }]),
                }),
                group_listing: None,
                group_item: None,
                episode_listing: None,
                episode_item: None,
            },
        ],
    }]);

    let eps = [
        make_episode_with_title(1, "Ep1 Main Story", 1, 1),
        make_episode_with_title(2, "Bonus: Behind the Scenes", 1, 2),
        make_episode_with_title(3, "Ep2 Main Story", 1, 3),
        make_episode_with_title(4, "Bonus: Outtakes", 1, 4),
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service
        .resolve_smart_playlists(None, "https://example.com/feed", &refs)
        .unwrap();

    // Collect all episode IDs from each playlist group
    let first_ids: Vec<i64> = result.playlists[0].episode_ids.clone();
    let second_ids: Vec<i64> = result.playlists[1].episode_ids.clone();

    let mut first_sorted = first_ids.clone();
    first_sorted.sort();
    let mut second_sorted = second_ids.clone();
    second_sorted.sort();

    // Main gets non-bonus, Bonus gets bonus episodes
    assert_eq!(first_sorted, vec![1, 3]);
    assert_eq!(second_sorted, vec![2, 4]);
}

#[test]
fn resolver_filter_regex_is_case_insensitive() {
    let service = make_resolver_service(vec![PresetConfig {
        id: "test".to_string(),
        podcast_guid: None,
        feed_urls: Some(vec!["https://example.com/feed".to_string()]),
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec![PlaylistDefinition {
            id: "bonus".to_string(),
            display_name: "Bonus".to_string(),
            grouping: GroupingConfig {
                by: "year".to_string(),
                numbering_extractor: None,
                static_classifiers: None,
            },
            selector: None,
            priority: 0,
            episode_filters: Some(EpisodeFilters {
                require: Some(vec![EpisodeFilterEntry {
                    title: Some("bonus".to_string()),
                    description: None,
                }]),
                exclude: None,
            }),
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
        }],
    }]);

    let eps = [
        make_episode_with_title(1, "BONUS Episode", 1, 1),
        make_episode_with_title(2, "Regular Episode", 1, 2),
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service
        .resolve_smart_playlists(None, "https://example.com/feed", &refs)
        .unwrap();

    let all_ids: Vec<i64> = result
        .playlists
        .iter()
        .flat_map(|p| &p.episode_ids)
        .copied()
        .collect();
    assert!(all_ids.contains(&1)); // BONUS matched by case-insensitive "bonus"
    assert!(!all_ids.contains(&2));
}

#[test]
fn resolver_filtered_definitions_process_before_fallbacks() {
    let service = make_resolver_service(vec![PresetConfig {
        id: "test".to_string(),
        podcast_guid: None,
        feed_urls: Some(vec!["https://example.com/feed".to_string()]),
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec![
            // Fallback (no filters) listed first, but should process last
            PlaylistDefinition {
                id: "all".to_string(),
                display_name: "All".to_string(),
                grouping: GroupingConfig {
                    by: "year".to_string(),
                    numbering_extractor: None,
                    static_classifiers: None,
                },
                selector: None,
                priority: 0,
                episode_filters: None,
                group_listing: None,
                group_item: None,
                episode_listing: None,
                episode_item: None,
            },
            PlaylistDefinition {
                id: "bonus".to_string(),
                display_name: "Bonus".to_string(),
                grouping: GroupingConfig {
                    by: "year".to_string(),
                    numbering_extractor: None,
                    static_classifiers: None,
                },
                selector: None,
                priority: 10,
                episode_filters: Some(EpisodeFilters {
                    require: Some(vec![EpisodeFilterEntry {
                        title: Some("Bonus".to_string()),
                        description: None,
                    }]),
                    exclude: None,
                }),
                group_listing: None,
                group_item: None,
                episode_listing: None,
                episode_item: None,
            },
        ],
    }]);

    let eps = [
        make_episode_with_title(1, "Regular Ep", 1, 1),
        make_episode_with_title(2, "Bonus Ep", 1, 2),
        make_episode_with_title(3, "Another Regular", 1, 3),
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service
        .resolve_smart_playlists(None, "https://example.com/feed", &refs)
        .unwrap();

    // Bonus (filtered) claims ep 2 before fallback processes
    // Fallback gets eps 1 and 3 (unclaimed)
    let all_ids: Vec<i64> = result
        .playlists
        .iter()
        .flat_map(|p| &p.episode_ids)
        .copied()
        .collect();
    let mut all_sorted = all_ids.clone();
    all_sorted.sort();
    all_sorted.dedup();
    assert_eq!(all_sorted, vec![1, 2, 3]);
}

#[test]
fn resolver_grouped_structure_produces_single_playlist_with_groups() {
    let service = make_resolver_service(vec![PresetConfig {
        id: "test".to_string(),
        podcast_guid: None,
        feed_urls: Some(vec!["https://example.com/feed".to_string()]),
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec![PlaylistDefinition {
            id: "regular".to_string(),
            display_name: "Regular Series".to_string(),
            grouping: GroupingConfig {
                by: "seasonNumber".to_string(),
                numbering_extractor: None,
                static_classifiers: None,
            },
            selector: None,
            priority: 0,
            episode_filters: None,
            group_listing: Some(GroupListingConfig {
                year_binding: Some("pinToYear".to_string()),
                user_sortable: None,
                sort: None,
            }),
            group_item: None,
            episode_listing: None,
            episode_item: None,
        }],
    }]);

    let eps = [
        make_rss_episode(1, 1, "S1E1", 1, 1),
        make_rss_episode(2, 1, "S1E2", 1, 2),
        make_rss_episode(3, 2, "S2E1", 3, 1),
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service
        .resolve_smart_playlists(None, "https://example.com/feed", &refs)
        .unwrap();

    // One parent playlist, not separate season playlists
    assert_eq!(result.playlists.len(), 1);

    let playlist = &result.playlists[0];
    assert_eq!(playlist.id, "regular");
    assert_eq!(playlist.display_name, "Regular Series");
    assert_eq!(playlist.presentation, Presentation::Combined);
    assert_eq!(playlist.year_binding, YearBinding::PinToYear);

    let mut ep_ids_sorted = playlist.episode_ids.clone();
    ep_ids_sorted.sort();
    assert_eq!(ep_ids_sorted, vec![1, 2, 3]);

    let groups = playlist.groups.as_ref().unwrap();
    assert_eq!(groups.len(), 2);

    let group_ids: Vec<&str> = groups.iter().map(|g| g.id.as_str()).collect();
    assert!(group_ids.contains(&"season_1"));
    assert!(group_ids.contains(&"season_2"));
}

#[test]
fn resolver_episode_ids_sorted_by_published_at_in_output() {
    let service = make_resolver_service(vec![PresetConfig {
        id: "test".to_string(),
        podcast_guid: None,
        feed_urls: Some(vec!["https://example.com/feed".to_string()]),
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec![PlaylistDefinition {
            id: "all".to_string(),
            display_name: "All".to_string(),
            grouping: GroupingConfig {
                by: "seasonNumber".to_string(),
                numbering_extractor: None,
                static_classifiers: None,
            },
            selector: None,
            priority: 0,
            episode_filters: None,
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
        }],
    }]);

    // Episodes in reverse chronological order
    let eps = [
        make_rss_episode(1, 1, "S1E1", 3, 1), // March
        make_rss_episode(2, 1, "S1E2", 1, 1), // January
        make_rss_episode(3, 1, "S1E3", 2, 1), // February
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service
        .resolve_smart_playlists(None, "https://example.com/feed", &refs)
        .unwrap();

    // Sorted ascending: Jan(2), Feb(3), Mar(1)
    assert_eq!(result.playlists[0].episode_ids, vec![2, 3, 1]);
}

#[test]
fn resolver_sorts_ungrouped_episode_ids() {
    let service = make_resolver_service(vec![PresetConfig {
        id: "test".to_string(),
        podcast_guid: None,
        feed_urls: Some(vec!["https://example.com/feed".to_string()]),
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec![PlaylistDefinition {
            id: "series".to_string(),
            display_name: "Series".to_string(),
            grouping: GroupingConfig {
                by: "seasonNumber".to_string(),
                numbering_extractor: None,
                static_classifiers: None,
            },
            selector: None,
            priority: 0,
            episode_filters: None,
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
        }],
    }]);

    let eps = [
        make_rss_episode(1, 1, "S1E1", 6, 1),
        // No season number -- becomes ungrouped
        SimpleEpisodeData {
            id: 2,
            title: "Bonus A".to_string(),
            description: None,
            season_number: None,
            episode_number: None,
            published_at: Some(Utc.with_ymd_and_hms(2024, 4, 1, 0, 0, 0).unwrap()),
            image_url: None,
        },
        SimpleEpisodeData {
            id: 3,
            title: "Bonus B".to_string(),
            description: None,
            season_number: None,
            episode_number: None,
            published_at: Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()),
            image_url: None,
        },
        SimpleEpisodeData {
            id: 4,
            title: "Bonus C".to_string(),
            description: None,
            season_number: None,
            episode_number: None,
            published_at: Some(Utc.with_ymd_and_hms(2024, 2, 1, 0, 0, 0).unwrap()),
            image_url: None,
        },
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service
        .resolve_smart_playlists(None, "https://example.com/feed", &refs)
        .unwrap();

    // Ungrouped sorted by publishedAt ascending: Jan(3), Feb(4), Apr(2)
    assert_eq!(result.ungrouped_episode_ids, vec![3, 4, 2]);
}

// ===========================================================================
// resolver_service preview tests
// ===========================================================================

#[test]
fn preview_returns_none_for_empty_episodes() {
    let service = make_resolver_service(vec![]);
    let refs: Vec<&dyn preset_core::models::EpisodeData> = vec![];

    let result = service.resolve_for_preview(None, "https://example.com/feed", &refs);
    assert!(result.is_none());
}

#[test]
fn preview_returns_none_when_no_config_matches() {
    let service = make_resolver_service(vec![]);

    let eps = [make_rss_episode(1, 1, "S1E1", 1, 1)];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service.resolve_for_preview(None, "https://example.com/feed", &refs);
    assert!(result.is_none());
}

#[test]
fn preview_returns_preview_grouping_with_single_playlist() {
    let service = make_resolver_service(vec![PresetConfig {
        id: "test".to_string(),
        podcast_guid: None,
        feed_urls: Some(vec!["https://example.com/feed".to_string()]),
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec![PlaylistDefinition {
            id: "seasons".to_string(),
            display_name: "Seasons".to_string(),
            grouping: GroupingConfig {
                by: "seasonNumber".to_string(),
                numbering_extractor: None,
                static_classifiers: None,
            },
            selector: None,
            priority: 0,
            episode_filters: None,
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
        }],
    }]);

    let eps = [
        make_rss_episode(1, 1, "S1E1", 1, 1),
        make_rss_episode(2, 1, "S1E2", 2, 1),
        make_rss_episode(3, 2, "S2E1", 3, 1),
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service
        .resolve_for_preview(None, "https://example.com/feed", &refs)
        .unwrap();

    assert_eq!(result.playlist_results.len(), 1);
    assert_eq!(result.playlist_results[0].definition_id, "seasons");
    assert!(result.playlist_results[0].claimed_by_others.is_empty());
    assert_eq!(result.resolver_type, "seasonNumber");
}

#[test]
fn preview_tracks_claimed_by_others() {
    let service = make_resolver_service(vec![PresetConfig {
        id: "test".to_string(),
        podcast_guid: None,
        feed_urls: Some(vec!["https://example.com/feed".to_string()]),
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec![
            PlaylistDefinition {
                id: "priority-a".to_string(),
                display_name: "Priority A".to_string(),
                grouping: GroupingConfig {
                    by: "year".to_string(),
                    numbering_extractor: None,
                    static_classifiers: None,
                },
                selector: None,
                priority: 10,
                episode_filters: Some(EpisodeFilters {
                    require: Some(vec![EpisodeFilterEntry {
                        title: Some(".".to_string()),
                        description: None,
                    }]),
                    exclude: None,
                }),
                group_listing: None,
                group_item: None,
                episode_listing: None,
                episode_item: None,
            },
            PlaylistDefinition {
                id: "priority-b".to_string(),
                display_name: "Priority B".to_string(),
                grouping: GroupingConfig {
                    by: "year".to_string(),
                    numbering_extractor: None,
                    static_classifiers: None,
                },
                selector: None,
                priority: 5,
                episode_filters: Some(EpisodeFilters {
                    require: Some(vec![EpisodeFilterEntry {
                        title: Some(".".to_string()),
                        description: None,
                    }]),
                    exclude: None,
                }),
                group_listing: None,
                group_item: None,
                episode_listing: None,
                episode_item: None,
            },
        ],
    }]);

    let eps = [
        make_episode_with_title(1, "Ep 1", 1, 1),
        make_episode_with_title(2, "Ep 2", 1, 2),
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service
        .resolve_for_preview(None, "https://example.com/feed", &refs)
        .unwrap();

    assert_eq!(result.playlist_results.len(), 2);

    // Priority B (lower number = higher precedence) processes first and claims both
    let b_result = result
        .playlist_results
        .iter()
        .find(|r| r.definition_id == "priority-b")
        .unwrap();
    let mut b_ids = b_result.playlist.episode_ids.clone();
    b_ids.sort();
    assert_eq!(b_ids, vec![1, 2]);
    assert!(b_result.claimed_by_others.is_empty());

    // Priority A: all candidates were claimed by B
    let a_result = result
        .playlist_results
        .iter()
        .find(|r| r.definition_id == "priority-a")
        .unwrap();
    assert!(a_result.playlist.episode_ids.is_empty());
    assert_eq!(a_result.claimed_by_others.len(), 2);
    assert_eq!(a_result.claimed_by_others.get(&1).unwrap(), "priority-b");
    assert_eq!(a_result.claimed_by_others.get(&2).unwrap(), "priority-b");
}

#[test]
fn preview_sorts_episode_ids_by_published_at() {
    let service = make_resolver_service(vec![PresetConfig {
        id: "test".to_string(),
        podcast_guid: None,
        feed_urls: Some(vec!["https://example.com/feed".to_string()]),
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec![PlaylistDefinition {
            id: "seasons".to_string(),
            display_name: "Seasons".to_string(),
            grouping: GroupingConfig {
                by: "seasonNumber".to_string(),
                numbering_extractor: None,
                static_classifiers: None,
            },
            selector: None,
            priority: 0,
            episode_filters: None,
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
        }],
    }]);

    // Episodes in reverse chronological order
    let eps = [
        make_rss_episode(1, 1, "S1E1", 3, 1), // March
        make_rss_episode(2, 1, "S1E2", 1, 1), // January
        make_rss_episode(3, 1, "S1E3", 2, 1), // February
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service
        .resolve_for_preview(None, "https://example.com/feed", &refs)
        .unwrap();

    // Sorted ascending: Jan(2), Feb(3), Mar(1)
    assert_eq!(
        result.playlist_results[0].playlist.episode_ids,
        vec![2, 3, 1]
    );
}

#[test]
fn preview_fallback_definition_has_empty_claimed_by_others() {
    let service = make_resolver_service(vec![PresetConfig {
        id: "test".to_string(),
        podcast_guid: None,
        feed_urls: Some(vec!["https://example.com/feed".to_string()]),
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec![
            PlaylistDefinition {
                id: "bonus".to_string(),
                display_name: "Bonus".to_string(),
                grouping: GroupingConfig {
                    by: "year".to_string(),
                    numbering_extractor: None,
                    static_classifiers: None,
                },
                selector: None,
                priority: 10,
                episode_filters: Some(EpisodeFilters {
                    require: Some(vec![EpisodeFilterEntry {
                        title: Some("Bonus".to_string()),
                        description: None,
                    }]),
                    exclude: None,
                }),
                group_listing: None,
                group_item: None,
                episode_listing: None,
                episode_item: None,
            },
            PlaylistDefinition {
                id: "all".to_string(),
                display_name: "All".to_string(),
                grouping: GroupingConfig {
                    by: "year".to_string(),
                    numbering_extractor: None,
                    static_classifiers: None,
                },
                selector: None,
                priority: 0,
                episode_filters: None, // fallback
                group_listing: None,
                group_item: None,
                episode_listing: None,
                episode_item: None,
            },
        ],
    }]);

    let eps = [
        make_episode_with_title(1, "Main Ep 1", 1, 1),
        make_episode_with_title(2, "Bonus: Extra", 1, 2),
        make_episode_with_title(3, "Main Ep 2", 1, 3),
    ];
    let refs: Vec<&dyn preset_core::models::EpisodeData> = eps
        .iter()
        .map(|e| e as &dyn preset_core::models::EpisodeData)
        .collect();

    let result = service
        .resolve_for_preview(None, "https://example.com/feed", &refs)
        .unwrap();

    assert_eq!(result.playlist_results.len(), 2);

    let bonus_result = result
        .playlist_results
        .iter()
        .find(|r| r.definition_id == "bonus")
        .unwrap();
    assert_eq!(bonus_result.playlist.episode_ids, vec![2]);
    assert!(bonus_result.claimed_by_others.is_empty());

    let all_result = result
        .playlist_results
        .iter()
        .find(|r| r.definition_id == "all")
        .unwrap();
    // Fallback (no filters) always has empty claimed_by_others
    assert!(all_result.claimed_by_others.is_empty());
}

// =========================================================================
// Partition by seasonNumber tests
// =========================================================================

#[test]
fn partition_by_season_creates_sub_groups() {
    use preset_core::models::{EpisodeData, SelectorConfig, TitleExtractor};
    use preset_core::resolvers::TitleAppearanceResolver;

    // Primary resolver: titleDiscovery (groups by professor name)
    // Selector: partitionBy seasonNumber (organize groups under seasons)
    let def = PlaylistDefinition {
        id: "by_prof".to_string(),
        display_name: "By Professor".to_string(),
        grouping: GroupingConfig {
            by: "titleDiscovery".to_string(),
            numbering_extractor: None,
            static_classifiers: None,
        },
        selector: Some(SelectorConfig {
            partition_by: Some("seasonNumber".to_string()),
            title_extractor: Some(TitleExtractor {
                source: "seasonNumber".to_string(),
                pattern: None,
                template: Some("Season ${0}".to_string()),
                fallback: None,
                fallback_value: None,
            }),
        }),
        priority: 0,
        episode_filters: None,
        group_listing: None,
        group_item: Some(preset_core::models::GroupItemConfig {
            show_date_range: None,
            pin_to_year: None,
            prepend_season_number: None,
            show_thumbnail: None,
            title_extractor: Some(TitleExtractor {
                source: "title".to_string(),
                pattern: Some(r"\[(\w+)".to_string()),
                template: Some("${1}".to_string()),
                fallback: None,
                fallback_value: None,
            }),
        }),
        episode_listing: None,
        episode_item: None,
    };

    let episodes = [
        SimpleEpisodeData {
            id: 1,
            title: "[ProfA 1] Topic".to_string(),
            description: None,
            season_number: Some(1),
            episode_number: Some(1),
            published_at: Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()),
            image_url: None,
        },
        SimpleEpisodeData {
            id: 2,
            title: "[ProfA 2] Topic".to_string(),
            description: None,
            season_number: Some(1),
            episode_number: Some(2),
            published_at: Some(Utc.with_ymd_and_hms(2024, 1, 8, 0, 0, 0).unwrap()),
            image_url: None,
        },
        SimpleEpisodeData {
            id: 3,
            title: "[ProfB 1] Topic".to_string(),
            description: None,
            season_number: Some(1),
            episode_number: Some(3),
            published_at: Some(Utc.with_ymd_and_hms(2024, 1, 15, 0, 0, 0).unwrap()),
            image_url: None,
        },
        SimpleEpisodeData {
            id: 4,
            title: "[ProfC 1] Topic".to_string(),
            description: None,
            season_number: Some(2),
            episode_number: Some(1),
            published_at: Some(Utc.with_ymd_and_hms(2024, 6, 1, 0, 0, 0).unwrap()),
            image_url: None,
        },
        SimpleEpisodeData {
            id: 5,
            title: "[ProfC 2] Topic".to_string(),
            description: None,
            season_number: Some(2),
            episode_number: Some(2),
            published_at: Some(Utc.with_ymd_and_hms(2024, 6, 8, 0, 0, 0).unwrap()),
            image_url: None,
        },
    ];

    let config = PresetConfig {
        id: "test".to_string(),
        podcast_guid: None,
        feed_urls: Some(vec!["https://example.com/feed".to_string()]),
        year_grouped_episodes: false,
        show_episode_thumbnail: None,
        playlists: vec![def],
    };

    let resolvers: Vec<Box<dyn preset_core::resolvers::Resolver>> = vec![
        Box::new(RssResolver),
        Box::new(TitleAppearanceResolver),
        Box::new(YearResolver),
    ];

    let refs: Vec<&dyn EpisodeData> = episodes.iter().map(|e| e as &dyn EpisodeData).collect();
    let service = ResolverService::new(resolvers, vec![config]);
    let result = service
        .resolve_smart_playlists(None, "https://example.com/feed", &refs)
        .expect("should produce grouping");

    // Should have 1 playlist (combined mode since partitionBy != "group")
    assert_eq!(result.playlists.len(), 1);
    let playlist = &result.playlists[0];

    // Top-level groups should be seasons (partitions)
    let groups = playlist.groups.as_ref().expect("should have groups");
    assert_eq!(groups.len(), 2);

    // Season 1 partition should have sub-groups: ProfA, ProfB
    let s1 = &groups[0];
    assert_eq!(s1.episode_ids.len(), 3);
    let s1_subs = s1
        .sub_groups
        .as_ref()
        .expect("season 1 should have sub_groups");
    assert_eq!(s1_subs.len(), 2);

    // Season 2 partition should have sub-group: ProfC
    let s2 = &groups[1];
    assert_eq!(s2.episode_ids.len(), 2);
    let s2_subs = s2
        .sub_groups
        .as_ref()
        .expect("season 2 should have sub_groups");
    assert_eq!(s2_subs.len(), 1);
}
