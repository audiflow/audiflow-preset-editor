use serde::{Deserialize, Serialize};

use super::group_def::GroupDef;
use super::numbering_extractor::NumberingExtractor;
use super::sort::{EpisodeSortRule, SortRule};
use super::title_extractor::TitleExtractor;

/// Configuration for the selector dropdown in the app UI.
///
/// Controls how resolver groups map to selector entries:
/// - No `partitionBy` -> single entry
/// - `partitionBy: "seasonNumber"` -> one entry per season, groups as cards within
/// - `partitionBy: "year"` -> one entry per year, groups as cards within
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectorConfig {
    /// How to partition groups into selector entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub partition_by: Option<String>,

    /// Generates display names for partitioned selector entries.
    /// Used when partitionBy is "seasonNumber" or "year".
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_extractor: Option<TitleExtractor>,
}

/// Grouping configuration: how episodes are organized into groups.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupingConfig {
    /// The grouping strategy.
    /// Values: "seasonNumber", "year", "titleDiscovery", "titleClassifier".
    #[serde(rename = "by")]
    pub by: String,

    /// Configuration for extracting season and episode numbers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub numbering_extractor: Option<NumberingExtractor>,

    /// Static group definitions for titleClassifier-based grouping.
    /// Also used by titleDiscovery for extraction pattern.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_classifiers: Option<Vec<GroupDef>>,
}

/// Group item display defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupItemConfig {
    /// Show date range on group cards.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_date_range: Option<bool>,

    /// Pin group to its earliest year's section.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin_to_year: Option<bool>,

    /// Whether to prepend "S{n}" to resolver result names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prepend_season_number: Option<bool>,

    /// Per-playlist default for showing thumbnails on group cards. None = use schema default (true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_thumbnail: Option<bool>,

    /// Configuration for extracting playlist/group display names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_extractor: Option<TitleExtractor>,
}

/// Episode item display defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeItemConfig {
    /// Per-playlist default for showing thumbnails on in-group episode rows. None = use schema default (true).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_thumbnail: Option<bool>,

    /// Configuration for extracting episode display names.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title_extractor: Option<TitleExtractor>,
}

/// Group listing settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupListingConfig {
    /// How groups relate to year headers.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year_binding: Option<String>,

    /// Allow users to change sort order at runtime.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_sortable: Option<bool>,

    /// Sort rule for ordering groups.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<SortRule>,
}

/// Episode listing settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeListingConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_year_headers: Option<bool>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<EpisodeSortRule>,
}

/// Unified per-playlist definition with all fields strongly typed (v7 schema).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDefinition {
    pub id: String,
    pub display_name: String,

    /// Episode claiming order among siblings (lower = first).
    pub priority: i32,

    /// Episode filters applied before resolver processing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_filters: Option<EpisodeFilters>,

    /// Grouping block: how episodes are organized into groups.
    pub grouping: GroupingConfig,

    /// Selector configuration controlling how groups map to dropdown entries.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selector: Option<SelectorConfig>,

    /// Group listing settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_listing: Option<GroupListingConfig>,

    /// Group item display defaults.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group_item: Option<GroupItemConfig>,

    /// Episode listing settings.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_listing: Option<EpisodeListingConfig>,

    /// Episode item display defaults.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub episode_item: Option<EpisodeItemConfig>,
}

impl PlaylistDefinition {
    /// Removes fields that are irrelevant to the current grouping strategy.
    ///
    /// This keeps form-state values safe (the editor preserves them for undo),
    /// but ensures persisted/previewed data is clean.
    ///
    /// Conditional fields by grouping.by:
    /// - `seasonNumber`:      numberingExtractor, groupItem.titleExtractor, groupItem.prependSeasonNumber
    /// - `titleDiscovery`:    groupItem.titleExtractor, staticClassifiers
    /// - `titleClassifier`:   staticClassifiers
    /// - `year`:              groupItem.titleExtractor
    /// - others:              none of the above
    pub fn strip_conditional_fields(&mut self) {
        let by = self.grouping.by.clone();

        // numberingExtractor: only seasonNumber
        if by != "seasonNumber" {
            self.grouping.numbering_extractor = None;
        }

        // titleExtractor (group-level only): seasonNumber, titleDiscovery, or year
        if by != "seasonNumber"
            && by != "titleDiscovery"
            && by != "year"
            && let Some(gi) = &mut self.group_item
        {
            gi.title_extractor = None;
        }

        // prependSeasonNumber: only meaningful for seasonNumber groups.
        if by != "seasonNumber"
            && let Some(gi) = &mut self.group_item
        {
            gi.prepend_season_number = None;
        }

        // staticClassifiers: titleClassifier or titleDiscovery
        if by != "titleClassifier" && by != "titleDiscovery" {
            self.grouping.static_classifiers = None;
        }
    }

    /// Whether this definition has any effective episode filters.
    pub fn has_filters(&self) -> bool {
        match &self.episode_filters {
            None => false,
            Some(f) => {
                let has_require = f.require.as_ref().is_some_and(|r| !r.is_empty());
                let has_exclude = f.exclude.as_ref().is_some_and(|e| !e.is_empty());
                has_require || has_exclude
            }
        }
    }
}

/// Episode filters applied before resolver processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeFilters {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require: Option<Vec<EpisodeFilterEntry>>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<EpisodeFilterEntry>>,
}

/// A single filter condition matched against episode fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeFilterEntry {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_definition(by: &str) -> PlaylistDefinition {
        let json = serde_json::json!({
            "id": "test",
            "displayName": "Test",
            "priority": 0,
            "grouping": {
                "by": by,
                "numberingExtractor": {
                    "source": "title",
                    "pattern": "(\\d+)",
                    "seasonGroup": 0,
                    "episodeGroup": 1
                },
                "staticClassifiers": [{ "id": "g1", "displayName": "G1", "pattern": { "source": "title", "pattern": ".*" } }]
            },
            "groupItem": {
                "titleExtractor": {
                    "source": "title",
                    "pattern": "(.+)",
                    "template": "${1}"
                }
            },
            "episodeItem": {
                "titleExtractor": {
                    "source": "title",
                    "pattern": "(.+)",
                    "template": "${1}"
                }
            }
        });
        serde_json::from_value(json).unwrap()
    }

    #[test]
    fn season_number_keeps_numbering_and_title_extractor() {
        let mut def = make_definition("seasonNumber");
        def.strip_conditional_fields();

        assert!(def.grouping.numbering_extractor.is_some());
        assert!(def.group_item.as_ref().unwrap().title_extractor.is_some());
        assert!(def.grouping.static_classifiers.is_none());
    }

    #[test]
    fn title_discovery_keeps_title_extractor_and_static_classifiers() {
        let mut def = make_definition("titleDiscovery");
        def.strip_conditional_fields();

        assert!(def.grouping.numbering_extractor.is_none());
        assert!(def.group_item.as_ref().unwrap().title_extractor.is_some());
        assert!(def.grouping.static_classifiers.is_some());
    }

    #[test]
    fn title_classifier_keeps_static_classifiers_only() {
        let mut def = make_definition("titleClassifier");
        def.strip_conditional_fields();

        assert!(def.grouping.numbering_extractor.is_none());
        assert!(def.group_item.as_ref().unwrap().title_extractor.is_none());
        assert!(def.grouping.static_classifiers.is_some());
    }

    #[test]
    fn year_keeps_title_extractor_strips_rest() {
        let mut def = make_definition("year");
        def.strip_conditional_fields();

        assert!(def.grouping.numbering_extractor.is_none());
        assert!(def.group_item.as_ref().unwrap().title_extractor.is_some());
        assert!(def.grouping.static_classifiers.is_none());
    }

    #[test]
    fn v5_grouping_deserializes() {
        let json = serde_json::json!({
            "id": "test",
            "displayName": "Test",
            "priority": 0,
            "grouping": {
                "by": "seasonNumber",
                "numberingExtractor": {
                    "source": "title",
                    "pattern": "(\\d+)",
                    "seasonGroup": 0,
                    "episodeGroup": 1
                }
            },
            "groupItem": {
                "showDateRange": true,
                "titleExtractor": {
                    "source": "title",
                    "pattern": "(.+)",
                    "template": "${1}"
                }
            },
            "episodeItem": {
                "titleExtractor": {
                    "source": "title",
                    "pattern": "(.+)",
                    "template": "${1}"
                }
            }
        });
        let def: PlaylistDefinition = serde_json::from_value(json).unwrap();
        assert_eq!(def.grouping.by, "seasonNumber");
        assert!(def.grouping.numbering_extractor.is_some());
        assert!(def.group_item.as_ref().unwrap().title_extractor.is_some());
        assert!(def.episode_item.as_ref().unwrap().title_extractor.is_some());
        assert!(def.group_item.as_ref().unwrap().show_date_range.unwrap());
    }

    #[test]
    fn show_thumbnail_round_trips_on_group_and_episode_items() {
        let json = serde_json::json!({
            "id": "test",
            "displayName": "Test",
            "priority": 0,
            "grouping": { "by": "seasonNumber" },
            "groupItem": { "showThumbnail": false },
            "episodeItem": { "showThumbnail": false }
        });
        let def: PlaylistDefinition = serde_json::from_value(json).unwrap();
        assert_eq!(def.group_item.as_ref().unwrap().show_thumbnail, Some(false));
        assert_eq!(
            def.episode_item.as_ref().unwrap().show_thumbnail,
            Some(false)
        );

        let out = serde_json::to_value(&def).unwrap();
        assert_eq!(out["groupItem"]["showThumbnail"], serde_json::json!(false));
        assert_eq!(
            out["episodeItem"]["showThumbnail"],
            serde_json::json!(false)
        );
    }

    #[test]
    fn show_thumbnail_absent_serializes_omitted() {
        let json = serde_json::json!({
            "id": "test",
            "displayName": "Test",
            "priority": 0,
            "grouping": { "by": "seasonNumber" },
            "groupItem": { "showDateRange": true },
            "episodeItem": {}
        });
        let def: PlaylistDefinition = serde_json::from_value(json).unwrap();
        assert!(def.group_item.as_ref().unwrap().show_thumbnail.is_none());
        let out = serde_json::to_value(&def).unwrap();
        assert!(out["groupItem"].get("showThumbnail").is_none());
    }
}
