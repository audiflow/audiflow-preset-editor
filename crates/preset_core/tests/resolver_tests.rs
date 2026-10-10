use chrono::{TimeZone, Utc};
use preset_core::models::{
    EpisodeData, GroupDef, GroupItemConfig, GroupingConfig, Matcher, PlaylistDefinition,
    SimpleEpisodeData, SortField, SortOrder, TitleExtractor,
};
use preset_core::resolvers::{
    CategoryResolver, Resolver, RssResolver, TitleAppearanceResolver, YearResolver,
};

// -- Helpers --

fn make_episode(id: i64, title: &str) -> SimpleEpisodeData {
    SimpleEpisodeData {
        id,
        title: title.to_string(),
        description: None,
        season_number: None,
        episode_number: None,
        published_at: Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()),
        image_url: None,
    }
}

fn make_episode_with_season(
    id: i64,
    title: &str,
    season: Option<i32>,
    episode: Option<i32>,
) -> SimpleEpisodeData {
    SimpleEpisodeData {
        id,
        title: title.to_string(),
        description: None,
        season_number: season,
        episode_number: episode,
        published_at: Some(Utc.with_ymd_and_hms(2024, 1, 1, 0, 0, 0).unwrap()),
        image_url: None,
    }
}

fn make_episode_with_date(
    id: i64,
    title: &str,
    year: i32,
    month: u32,
    day: u32,
) -> SimpleEpisodeData {
    SimpleEpisodeData {
        id,
        title: title.to_string(),
        description: None,
        season_number: None,
        episode_number: None,
        published_at: Some(Utc.with_ymd_and_hms(year, month, day, 0, 0, 0).unwrap()),
        image_url: None,
    }
}

fn make_episode_no_date(id: i64, title: &str) -> SimpleEpisodeData {
    SimpleEpisodeData {
        id,
        title: title.to_string(),
        description: None,
        season_number: None,
        episode_number: None,
        published_at: None,
        image_url: None,
    }
}

fn as_refs(episodes: &[SimpleEpisodeData]) -> Vec<&dyn EpisodeData> {
    episodes.iter().map(|e| e as &dyn EpisodeData).collect()
}

fn minimal_definition(by: &str) -> PlaylistDefinition {
    PlaylistDefinition {
        id: "test".to_string(),
        display_name: "Test".to_string(),
        grouping: GroupingConfig {
            by: by.to_string(),
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

// ============================================================
// RssResolver Tests
// ============================================================

#[test]
fn rss_resolver_type_is_season_number() {
    let resolver = RssResolver;
    assert_eq!(resolver.resolver_type(), "seasonNumber");
}

#[test]
fn rss_default_sort_is_playlist_number_ascending() {
    let resolver = RssResolver;
    let sort = resolver.default_sort();
    assert_eq!(sort.field, SortField::PlaylistNumber);
    assert_eq!(sort.order, SortOrder::Ascending);
}

#[test]
fn rss_returns_none_when_no_episodes_have_season_numbers() {
    let resolver = RssResolver;
    let episodes = vec![
        make_episode(1, "Ep1"),
        make_episode(2, "Ep2"),
        make_episode(3, "Ep3"),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, None);
    assert!(result.is_none());
}

#[test]
fn rss_groups_episodes_by_season_number() {
    let resolver = RssResolver;
    let episodes = vec![
        make_episode_with_season(1, "Ep1", Some(1), Some(1)),
        make_episode_with_season(2, "Ep2", Some(1), Some(2)),
        make_episode_with_season(3, "Ep3", Some(2), Some(1)),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, None);

    assert!(result.is_some());
    let grouping = result.unwrap();
    assert_eq!(grouping.playlists.len(), 2);

    let p1 = grouping
        .playlists
        .iter()
        .find(|p| p.id == "season_1")
        .unwrap();
    let p2 = grouping
        .playlists
        .iter()
        .find(|p| p.id == "season_2")
        .unwrap();
    assert_eq!(p1.episode_ids, vec![1, 2]);
    assert_eq!(p2.episode_ids, vec![3]);
}

#[test]
fn rss_treats_null_season_as_ungrouped() {
    let resolver = RssResolver;
    let episodes = vec![
        make_episode_with_season(1, "Ep1", Some(1), Some(1)),
        make_episode_with_season(2, "Ep2", None, Some(100)),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, None);

    assert!(result.is_some());
    let grouping = result.unwrap();
    assert_eq!(grouping.playlists.len(), 1);
    assert_eq!(grouping.ungrouped_episode_ids, vec![2]);
}

#[test]
fn rss_uses_season_number_as_sort_key() {
    let resolver = RssResolver;
    let episodes = vec![
        make_episode_with_season(1, "Ep1", Some(1), Some(5)),
        make_episode_with_season(2, "Ep2", Some(1), Some(10)),
        make_episode_with_season(3, "Ep3", Some(2), Some(3)),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, None).unwrap();

    let p1 = result
        .playlists
        .iter()
        .find(|p| p.id == "season_1")
        .unwrap();
    let p2 = result
        .playlists
        .iter()
        .find(|p| p.id == "season_2")
        .unwrap();
    assert_eq!(p1.sort_key, 1);
    assert_eq!(p2.sort_key, 2);
}

#[test]
fn rss_seasons_sorted_by_season_number() {
    let resolver = RssResolver;
    let episodes = vec![
        make_episode_with_season(1, "Ep1", Some(3), Some(1)),
        make_episode_with_season(2, "Ep2", Some(1), Some(1)),
        make_episode_with_season(3, "Ep3", Some(2), Some(1)),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, None).unwrap();

    assert_eq!(result.playlists.len(), 3);
    assert_eq!(result.playlists[0].sort_key, 1);
    assert_eq!(result.playlists[1].sort_key, 2);
    assert_eq!(result.playlists[2].sort_key, 3);
}

#[test]
fn rss_uses_title_extractor_for_display_names() {
    let resolver = RssResolver;
    let mut def = minimal_definition("seasonNumber");
    def.group_item = Some(GroupItemConfig {
        show_date_range: None,
        pin_to_year: None,
        prepend_season_number: None,
        show_thumbnail: None,
        title_extractor: Some(TitleExtractor {
            source: "title".to_string(),
            pattern: Some(r"(.+?) \d+$".to_string()),
            template: Some("${1}".to_string()),
            fallback: None,
            fallback_value: None,
        }),
    });

    let episodes = vec![
        make_episode_with_season(1, "Topic A 1", Some(1), None),
        make_episode_with_season(2, "Topic A 2", Some(1), None),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def)).unwrap();

    assert_eq!(result.playlists[0].display_name, "Topic A");
}

#[test]
fn rss_display_name_defaults_to_season_n() {
    let resolver = RssResolver;
    let episodes = vec![make_episode_with_season(1, "Ep1", Some(5), None)];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, None).unwrap();

    assert_eq!(result.playlists[0].display_name, "Season 5");
}

// ============================================================
// CategoryResolver Tests
// ============================================================

#[test]
fn category_resolver_type_is_title_classifier() {
    let resolver = CategoryResolver;
    assert_eq!(resolver.resolver_type(), "titleClassifier");
}

#[test]
fn category_default_sort_is_playlist_number_ascending() {
    let resolver = CategoryResolver;
    let sort = resolver.default_sort();
    assert_eq!(sort.field, SortField::PlaylistNumber);
    assert_eq!(sort.order, SortOrder::Ascending);
}

#[test]
fn category_returns_none_without_definition() {
    let resolver = CategoryResolver;
    let episodes = vec![make_episode(1, "Episode 1")];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, None);
    assert!(result.is_none());
}

#[test]
fn category_returns_none_without_static_classifiers() {
    let resolver = CategoryResolver;
    let def = minimal_definition("titleClassifier");
    let episodes = vec![make_episode(1, "Episode 1")];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def));
    assert!(result.is_none());
}

#[test]
fn category_returns_none_when_static_classifiers_empty() {
    let resolver = CategoryResolver;
    let mut def = minimal_definition("titleClassifier");
    def.grouping.static_classifiers = Some(vec![]);
    let episodes = vec![make_episode(1, "Episode 1")];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def));
    assert!(result.is_none());
}

#[test]
fn category_groups_episodes_by_pattern() {
    let resolver = CategoryResolver;
    let mut def = minimal_definition("titleClassifier");
    def.grouping.static_classifiers = Some(vec![
        GroupDef {
            id: "saturday".to_string(),
            display_name: "Saturday".to_string(),
            pattern: Some(Matcher {
                source: "title".to_string(),
                pattern: r"Saturday".to_string(),
            }),
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
            numbering_extractor: None,
        },
        GroupDef {
            id: "news_talk".to_string(),
            display_name: "News Talk".to_string(),
            pattern: Some(Matcher {
                source: "title".to_string(),
                pattern: r"News Talk".to_string(),
            }),
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
            numbering_extractor: None,
        },
        GroupDef {
            id: "other".to_string(),
            display_name: "Other".to_string(),
            pattern: None,
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
            numbering_extractor: None,
        },
    ]);

    let episodes = vec![
        make_episode(1, "Saturday #62 topic"),
        make_episode(2, "News Talk #200 bonds"),
        make_episode(3, "Jan 29 EU news"),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def));

    assert!(result.is_some());
    let grouping = result.unwrap();
    assert_eq!(grouping.playlists.len(), 3);
    assert_eq!(grouping.playlists[0].id, "saturday");
    assert_eq!(grouping.playlists[0].episode_ids, vec![1]);
    assert_eq!(grouping.playlists[1].id, "news_talk");
    assert_eq!(grouping.playlists[1].episode_ids, vec![2]);
    assert_eq!(grouping.playlists[2].id, "other");
    assert_eq!(grouping.playlists[2].episode_ids, vec![3]);
}

#[test]
fn category_pattern_matches_case_insensitively() {
    // The app compiles classifier patterns with caseSensitive: false; the
    // preview must agree or episodes like "The Empire Of Gold" drop out.
    let resolver = CategoryResolver;
    let mut def = minimal_definition("titleClassifier");
    def.grouping.static_classifiers = Some(vec![GroupDef {
        id: "empire".to_string(),
        display_name: "Empire of Gold".to_string(),
        pattern: Some(Matcher {
            source: "title".to_string(),
            pattern: r"Empire of Gold".to_string(),
        }),
        group_listing: None,
        group_item: None,
        episode_listing: None,
        episode_item: None,
        numbering_extractor: None,
    }]);

    let episodes = vec![
        make_episode(1, "Empire of Gold | 1. Matricide"),
        make_episode(2, "Introducing...The Empire Of Gold"),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def)).unwrap();

    assert_eq!(result.playlists[0].episode_ids, vec![1, 2]);
    assert!(result.ungrouped_episode_ids.is_empty());
}

#[test]
fn category_ungrouped_when_no_fallback_group() {
    let resolver = CategoryResolver;
    let mut def = minimal_definition("titleClassifier");
    def.grouping.static_classifiers = Some(vec![GroupDef {
        id: "saturday".to_string(),
        display_name: "Saturday".to_string(),
        pattern: Some(Matcher {
            source: "title".to_string(),
            pattern: r"Saturday".to_string(),
        }),
        group_listing: None,
        group_item: None,
        episode_listing: None,
        episode_item: None,
        numbering_extractor: None,
    }]);

    let episodes = vec![
        make_episode(1, "Saturday #62 topic"),
        make_episode(2, "No match"),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def)).unwrap();

    assert_eq!(result.ungrouped_episode_ids, vec![2]);
}

#[test]
fn category_first_match_wins() {
    let resolver = CategoryResolver;
    let mut def = minimal_definition("titleClassifier");
    def.grouping.static_classifiers = Some(vec![
        GroupDef {
            id: "first".to_string(),
            display_name: "First".to_string(),
            pattern: Some(Matcher {
                source: "title".to_string(),
                pattern: r"Hello".to_string(),
            }),
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
            numbering_extractor: None,
        },
        GroupDef {
            id: "second".to_string(),
            display_name: "Second".to_string(),
            pattern: Some(Matcher {
                source: "title".to_string(),
                pattern: r"Hello World".to_string(),
            }),
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
            numbering_extractor: None,
        },
    ]);

    let episodes = vec![make_episode(1, "Hello World")];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def)).unwrap();

    assert_eq!(result.playlists.len(), 1);
    assert_eq!(result.playlists[0].id, "first");
}

#[test]
fn category_assigns_incrementing_sort_keys() {
    let resolver = CategoryResolver;
    let mut def = minimal_definition("titleClassifier");
    def.grouping.static_classifiers = Some(vec![
        GroupDef {
            id: "alpha".to_string(),
            display_name: "Alpha".to_string(),
            pattern: Some(Matcher {
                source: "title".to_string(),
                pattern: r"AAA".to_string(),
            }),
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
            numbering_extractor: None,
        },
        GroupDef {
            id: "beta".to_string(),
            display_name: "Beta".to_string(),
            pattern: Some(Matcher {
                source: "title".to_string(),
                pattern: r"BBB".to_string(),
            }),
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
            numbering_extractor: None,
        },
        GroupDef {
            id: "other".to_string(),
            display_name: "Other".to_string(),
            pattern: None,
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
            numbering_extractor: None,
        },
    ]);

    let episodes = vec![
        make_episode(1, "AAA episode"),
        make_episode(2, "BBB episode"),
        make_episode(3, "CCC episode"),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def)).unwrap();

    assert_eq!(result.playlists.len(), 3);
    assert_eq!(result.playlists[0].sort_key, 1);
    assert_eq!(result.playlists[1].sort_key, 2);
    assert_eq!(result.playlists[2].sort_key, 3);
}

#[test]
fn category_fallback_collects_unmatched() {
    let resolver = CategoryResolver;
    let mut def = minimal_definition("titleClassifier");
    def.grouping.static_classifiers = Some(vec![
        GroupDef {
            id: "matched".to_string(),
            display_name: "Matched".to_string(),
            pattern: Some(Matcher {
                source: "title".to_string(),
                pattern: r"AAA".to_string(),
            }),
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
            numbering_extractor: None,
        },
        GroupDef {
            id: "fallback".to_string(),
            display_name: "Fallback".to_string(),
            pattern: None,
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
            numbering_extractor: None,
        },
    ]);

    let episodes = vec![
        make_episode(1, "AAA episode"),
        make_episode(2, "BBB episode"),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def)).unwrap();

    assert_eq!(result.playlists.len(), 2);
    assert_eq!(result.playlists[0].id, "matched");
    assert_eq!(result.playlists[0].episode_ids, vec![1]);
    assert_eq!(result.playlists[1].id, "fallback");
    assert_eq!(result.playlists[1].episode_ids, vec![2]);
}

#[test]
fn category_skips_empty_pattern_groups_in_sort_keys() {
    let resolver = CategoryResolver;
    let mut def = minimal_definition("titleClassifier");
    def.grouping.static_classifiers = Some(vec![
        GroupDef {
            id: "alpha".to_string(),
            display_name: "Alpha".to_string(),
            pattern: Some(Matcher {
                source: "title".to_string(),
                pattern: r"AAA".to_string(),
            }),
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
            numbering_extractor: None,
        },
        GroupDef {
            id: "beta".to_string(),
            display_name: "Beta".to_string(),
            pattern: Some(Matcher {
                source: "title".to_string(),
                pattern: r"BBB".to_string(),
            }),
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
            numbering_extractor: None,
        },
        GroupDef {
            id: "gamma".to_string(),
            display_name: "Gamma".to_string(),
            pattern: Some(Matcher {
                source: "title".to_string(),
                pattern: r"CCC".to_string(),
            }),
            group_listing: None,
            group_item: None,
            episode_listing: None,
            episode_item: None,
            numbering_extractor: None,
        },
    ]);

    // Only alpha and gamma have episodes; beta is empty
    let episodes = vec![
        make_episode(1, "AAA episode"),
        make_episode(2, "CCC episode"),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def)).unwrap();

    assert_eq!(result.playlists.len(), 2);
    assert_eq!(result.playlists[0].id, "alpha");
    assert_eq!(result.playlists[0].sort_key, 1);
    assert_eq!(result.playlists[1].id, "gamma");
    assert_eq!(result.playlists[1].sort_key, 2);
}

#[test]
fn category_pattern_supports_negative_lookahead() {
    // Guard against a future regression that swaps the server regex
    // engine back to the `regex` crate, which cannot compile
    // lookaround. This asserts the pattern actually constrains which
    // episodes match — if the engine rejected the pattern, every
    // episode would fall through to the ungrouped bucket.
    let resolver = CategoryResolver;
    let mut def = minimal_definition("titleClassifier");
    def.grouping.static_classifiers = Some(vec![GroupDef {
        id: "non_recap".to_string(),
        display_name: "Not a recap".to_string(),
        pattern: Some(Matcher {
            source: "title".to_string(),
            pattern: r"^(?!Recap)".to_string(),
        }),
        group_listing: None,
        group_item: None,
        episode_listing: None,
        episode_item: None,
        numbering_extractor: None,
    }]);
    let episodes = vec![
        make_episode(1, "Recap of Episode 1"),
        make_episode(2, "Live show"),
        make_episode(3, "Recap of Episode 2"),
        make_episode(4, "Another live show"),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def)).unwrap();

    assert_eq!(result.playlists.len(), 1);
    assert_eq!(result.playlists[0].id, "non_recap");
    assert_eq!(result.playlists[0].episode_ids, vec![2, 4]);
    assert_eq!(result.ungrouped_episode_ids, vec![1, 3]);
}

// ============================================================
// YearResolver Tests
// ============================================================

#[test]
fn year_resolver_type_is_year() {
    let resolver = YearResolver;
    assert_eq!(resolver.resolver_type(), "year");
}

#[test]
fn year_default_sort_is_playlist_number_descending() {
    let resolver = YearResolver;
    let sort = resolver.default_sort();
    assert_eq!(sort.field, SortField::PlaylistNumber);
    assert_eq!(sort.order, SortOrder::Descending);
}

#[test]
fn year_returns_none_when_no_episodes_have_dates() {
    let resolver = YearResolver;
    let episodes = vec![
        make_episode_no_date(1, "Ep1"),
        make_episode_no_date(2, "Ep2"),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, None);
    assert!(result.is_none());
}

#[test]
fn year_groups_episodes_by_publish_year() {
    let resolver = YearResolver;
    let episodes = vec![
        make_episode_with_date(1, "Ep1", 2023, 3, 15),
        make_episode_with_date(2, "Ep2", 2023, 8, 20),
        make_episode_with_date(3, "Ep3", 2024, 1, 10),
        make_episode_with_date(4, "Ep4", 2024, 6, 5),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, None);

    assert!(result.is_some());
    let grouping = result.unwrap();
    assert_eq!(grouping.playlists.len(), 2);

    // Sorted descending (newest first)
    assert_eq!(grouping.playlists[0].display_name, "2024");
    assert_eq!(grouping.playlists[0].episode_ids, vec![3, 4]);
    assert_eq!(grouping.playlists[1].display_name, "2023");
    assert_eq!(grouping.playlists[1].episode_ids, vec![1, 2]);
}

#[test]
fn year_episodes_without_date_go_to_ungrouped() {
    let resolver = YearResolver;
    let episodes = vec![
        make_episode_with_date(1, "Ep1", 2024, 1, 1),
        make_episode_no_date(2, "Ep2"),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, None).unwrap();

    assert_eq!(result.ungrouped_episode_ids, vec![2]);
}

#[test]
fn year_display_name_defaults_to_year_string() {
    let resolver = YearResolver;
    let episodes = vec![make_episode_with_date(1, "Ep1", 2024, 6, 15)];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, None).unwrap();

    assert_eq!(result.playlists[0].display_name, "2024");
}

#[test]
fn year_sort_key_is_year_value() {
    let resolver = YearResolver;
    let episodes = vec![
        make_episode_with_date(1, "Ep1", 2022, 1, 1),
        make_episode_with_date(2, "Ep2", 2024, 1, 1),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, None).unwrap();

    assert_eq!(result.playlists[0].sort_key, 2024);
    assert_eq!(result.playlists[1].sort_key, 2022);
}

#[test]
fn year_uses_title_extractor_for_display_names() {
    let resolver = YearResolver;
    let mut def = minimal_definition("year");
    def.group_item = Some(GroupItemConfig {
        show_date_range: None,
        pin_to_year: None,
        prepend_season_number: None,
        show_thumbnail: None,
        title_extractor: Some(TitleExtractor {
            source: "title".to_string(),
            pattern: Some(r"^(\w+)".to_string()),
            template: Some("${1}".to_string()),
            fallback: None,
            fallback_value: None,
        }),
    });

    let episodes = vec![make_episode_with_date(1, "Spring 2024", 2024, 4, 1)];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def)).unwrap();

    assert_eq!(result.playlists[0].display_name, "Spring");
}

// ============================================================
// TitleAppearanceResolver Tests
// ============================================================

#[test]
fn title_appearance_resolver_type() {
    let resolver = TitleAppearanceResolver;
    assert_eq!(resolver.resolver_type(), "titleDiscovery");
}

#[test]
fn title_appearance_default_sort_ascending() {
    let resolver = TitleAppearanceResolver;
    let sort = resolver.default_sort();
    assert_eq!(sort.field, SortField::PlaylistNumber);
    assert_eq!(sort.order, SortOrder::Ascending);
}

#[test]
fn title_appearance_returns_none_without_definition() {
    let resolver = TitleAppearanceResolver;
    let episodes = vec![make_episode_with_date(1, "[Rome 1] First", 2024, 1, 1)];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, None);
    assert!(result.is_none());
}

#[test]
fn title_appearance_returns_none_without_extractor_or_pattern() {
    let resolver = TitleAppearanceResolver;
    let def = minimal_definition("titleDiscovery");
    let episodes = vec![make_episode_with_date(1, "[Rome 1] First", 2024, 1, 1)];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def));
    assert!(result.is_none());
}

#[test]
fn title_appearance_groups_by_first_appearance_using_pattern() {
    let resolver = TitleAppearanceResolver;
    let mut def = minimal_definition("titleDiscovery");
    def.grouping.static_classifiers = Some(vec![GroupDef {
        id: "extract".to_string(),
        display_name: "Extract".to_string(),
        pattern: Some(Matcher {
            source: "title".to_string(),
            pattern: r"\[(\w+)\s+\d+\]".to_string(),
        }),
        group_listing: None,
        group_item: None,
        episode_listing: None,
        episode_item: None,
        numbering_extractor: None,
    }]);

    let episodes = vec![
        make_episode_with_date(6, "[Firenze 1] Renaissance", 2024, 3, 1),
        make_episode_with_date(5, "[Venezia 2] Canals", 2024, 2, 15),
        make_episode_with_date(4, "[Venezia 1] Arrival", 2024, 2, 1),
        make_episode_with_date(3, "[Rome 3] Colosseum", 2024, 1, 20),
        make_episode_with_date(2, "[Rome 2] Vatican", 2024, 1, 10),
        make_episode_with_date(1, "[Rome 1] First Steps", 2024, 1, 1),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def));

    assert!(result.is_some());
    let grouping = result.unwrap();
    assert_eq!(grouping.playlists.len(), 3);

    // Rome appeared first chronologically
    assert_eq!(grouping.playlists[0].display_name, "Rome");
    assert_eq!(grouping.playlists[0].sort_key, 1);
    assert!(grouping.playlists[0].episode_ids.contains(&1));
    assert!(grouping.playlists[0].episode_ids.contains(&2));
    assert!(grouping.playlists[0].episode_ids.contains(&3));

    // Venezia appeared second
    assert_eq!(grouping.playlists[1].display_name, "Venezia");
    assert_eq!(grouping.playlists[1].sort_key, 2);

    // Firenze appeared third
    assert_eq!(grouping.playlists[2].display_name, "Firenze");
    assert_eq!(grouping.playlists[2].sort_key, 3);
}

#[test]
fn title_appearance_non_matching_go_to_ungrouped() {
    let resolver = TitleAppearanceResolver;
    let mut def = minimal_definition("titleDiscovery");
    def.grouping.static_classifiers = Some(vec![GroupDef {
        id: "extract".to_string(),
        display_name: "Extract".to_string(),
        pattern: Some(Matcher {
            source: "title".to_string(),
            pattern: r"\[(\w+)\s+\d+\]".to_string(),
        }),
        group_listing: None,
        group_item: None,
        episode_listing: None,
        episode_item: None,
        numbering_extractor: None,
    }]);

    let episodes = vec![
        make_episode_with_date(1, "[Rome 1] First", 2024, 1, 1),
        make_episode_with_date(2, "Bonus Episode", 2024, 1, 5),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def)).unwrap();

    assert_eq!(result.ungrouped_episode_ids, vec![2]);
}

#[test]
fn title_appearance_returns_none_when_no_matches() {
    let resolver = TitleAppearanceResolver;
    let mut def = minimal_definition("titleDiscovery");
    def.grouping.static_classifiers = Some(vec![GroupDef {
        id: "extract".to_string(),
        display_name: "Extract".to_string(),
        pattern: Some(Matcher {
            source: "title".to_string(),
            pattern: r"\[(\w+)\s+\d+\]".to_string(),
        }),
        group_listing: None,
        group_item: None,
        episode_listing: None,
        episode_item: None,
        numbering_extractor: None,
    }]);

    let episodes = vec![
        make_episode_with_date(1, "No Pattern Here", 2024, 1, 1),
        make_episode_with_date(2, "Another One", 2024, 1, 2),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def));
    assert!(result.is_none());
}

#[test]
fn title_appearance_episodes_without_date_appended_at_end() {
    let resolver = TitleAppearanceResolver;
    let mut def = minimal_definition("titleDiscovery");
    def.grouping.static_classifiers = Some(vec![GroupDef {
        id: "extract".to_string(),
        display_name: "Extract".to_string(),
        pattern: Some(Matcher {
            source: "title".to_string(),
            pattern: r"\[(\w+)\s+\d+\]".to_string(),
        }),
        group_listing: None,
        group_item: None,
        episode_listing: None,
        episode_item: None,
        numbering_extractor: None,
    }]);

    let episodes = vec![
        make_episode_with_date(1, "[Rome 1] First", 2024, 1, 1),
        make_episode_no_date(2, "[Rome 2] Second"),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def)).unwrap();

    assert_eq!(result.playlists.len(), 1);
    assert_eq!(result.playlists[0].display_name, "Rome");
    assert_eq!(result.playlists[0].episode_ids, vec![1, 2]);
}

#[test]
fn title_appearance_uses_title_extractor() {
    let resolver = TitleAppearanceResolver;
    let mut def = minimal_definition("titleDiscovery");
    def.group_item = Some(GroupItemConfig {
        show_date_range: None,
        pin_to_year: None,
        prepend_season_number: None,
        show_thumbnail: None,
        title_extractor: Some(TitleExtractor {
            source: "title".to_string(),
            pattern: Some(r"\[(\w+)\s+\d+\]".to_string()),
            template: Some("${1}".to_string()),
            fallback: None,
            fallback_value: None,
        }),
    });

    let episodes = vec![
        make_episode_with_date(1, "[Rome 1] First Steps", 2024, 1, 1),
        make_episode_with_date(2, "[Rome 2] Vatican", 2024, 1, 10),
        make_episode_with_date(3, "[Venezia 1] Arrival", 2024, 2, 1),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def)).unwrap();

    assert_eq!(result.playlists.len(), 2);
    assert_eq!(result.playlists[0].display_name, "Rome");
    assert_eq!(result.playlists[1].display_name, "Venezia");
}

#[test]
fn title_appearance_playlist_ids_use_appearance_prefix() {
    let resolver = TitleAppearanceResolver;
    let mut def = minimal_definition("titleDiscovery");
    def.grouping.static_classifiers = Some(vec![GroupDef {
        id: "extract".to_string(),
        display_name: "Extract".to_string(),
        pattern: Some(Matcher {
            source: "title".to_string(),
            pattern: r"\[(\w+)\s+\d+\]".to_string(),
        }),
        group_listing: None,
        group_item: None,
        episode_listing: None,
        episode_item: None,
        numbering_extractor: None,
    }]);

    let episodes = vec![
        make_episode_with_date(1, "[Rome 1] First", 2024, 1, 1),
        make_episode_with_date(2, "[Venezia 1] Arr", 2024, 2, 1),
    ];
    let refs = as_refs(&episodes);
    let result = resolver.resolve(&refs, Some(&def)).unwrap();

    assert_eq!(result.playlists[0].id, "appearance_1");
    assert_eq!(result.playlists[1].id, "appearance_2");
}

// ============================================================
// Empty episode list tests
// ============================================================

#[test]
fn rss_empty_episodes_returns_none() {
    let resolver = RssResolver;
    let refs: Vec<&dyn EpisodeData> = vec![];
    assert!(resolver.resolve(&refs, None).is_none());
}

#[test]
fn category_empty_episodes_returns_none() {
    let resolver = CategoryResolver;
    let mut def = minimal_definition("titleClassifier");
    def.grouping.static_classifiers = Some(vec![GroupDef {
        id: "g".to_string(),
        display_name: "G".to_string(),
        pattern: Some(Matcher {
            source: "title".to_string(),
            pattern: r"x".to_string(),
        }),
        group_listing: None,
        group_item: None,
        episode_listing: None,
        episode_item: None,
        numbering_extractor: None,
    }]);
    let refs: Vec<&dyn EpisodeData> = vec![];
    assert!(resolver.resolve(&refs, Some(&def)).is_none());
}

#[test]
fn year_empty_episodes_returns_none() {
    let resolver = YearResolver;
    let refs: Vec<&dyn EpisodeData> = vec![];
    assert!(resolver.resolve(&refs, None).is_none());
}

#[test]
fn title_appearance_empty_episodes_returns_none() {
    let resolver = TitleAppearanceResolver;
    let mut def = minimal_definition("titleDiscovery");
    def.grouping.static_classifiers = Some(vec![GroupDef {
        id: "extract".to_string(),
        display_name: "Extract".to_string(),
        pattern: Some(Matcher {
            source: "title".to_string(),
            pattern: r"\[(\w+)\s+\d+\]".to_string(),
        }),
        group_listing: None,
        group_item: None,
        episode_listing: None,
        episode_item: None,
        numbering_extractor: None,
    }]);
    let refs: Vec<&dyn EpisodeData> = vec![];
    assert!(resolver.resolve(&refs, Some(&def)).is_none());
}
