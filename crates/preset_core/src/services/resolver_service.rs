use std::collections::{HashMap, HashSet};

use fancy_regex::Regex;

use std::collections::BTreeMap;

use crate::models::{
    EpisodeData, EpisodeFilterEntry, GroupDef, Grouping, Playlist, PlaylistDefinition,
    PlaylistGroup, PlaylistPreviewResult, Presentation, PresetConfig, PreviewGrouping,
    SimpleEpisodeData, TitleExtractor, YearBinding,
};

/// Precompiled filter entry for efficient per-episode matching.
struct CompiledFilterEntry {
    title: Option<Regex>,
    description: Option<Regex>,
}

impl CompiledFilterEntry {
    fn compile(entry: &EpisodeFilterEntry) -> Self {
        // fancy-regex's RegexBuilder does not expose case_insensitive; use
        // the inline (?i) flag so the pattern itself requests the same
        // behavior that the old regex crate's RegexBuilder configured.
        fn build(pattern: &str) -> Option<Regex> {
            Regex::new(&format!("(?i){pattern}")).ok()
        }
        Self {
            title: entry.title.as_deref().and_then(build),
            description: entry.description.as_deref().and_then(build),
        }
    }

    fn matches(&self, episode: &dyn EpisodeData) -> bool {
        // fancy-regex returns Result<bool> for is_match (the VM fallback can
        // hit a backtrack limit). Treat any compile/runtime error as a
        // non-match so a malformed pattern does not crash the resolver.
        if let Some(ref regex) = self.title
            && !regex.is_match(episode.title()).unwrap_or(false)
        {
            return false;
        }
        if let Some(ref regex) = self.description
            && !regex
                .is_match(episode.description().unwrap_or(""))
                .unwrap_or(false)
        {
            return false;
        }
        true
    }
}

/// Precompiled require/exclude filters for a playlist definition.
struct CompiledFilters {
    require: Vec<CompiledFilterEntry>,
    exclude: Vec<CompiledFilterEntry>,
}

impl CompiledFilters {
    fn compile(definition: &PlaylistDefinition) -> Option<Self> {
        let filters = definition.episode_filters.as_ref()?;
        Some(Self {
            require: filters
                .require
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(CompiledFilterEntry::compile)
                .collect(),
            exclude: filters
                .exclude
                .as_deref()
                .unwrap_or(&[])
                .iter()
                .map(CompiledFilterEntry::compile)
                .collect(),
        })
    }

    fn matches(&self, episode: &dyn EpisodeData) -> bool {
        // The schema documents require/exclude as OR across entries: an
        // episode is included when it matches ANY require rule and is
        // excluded when it matches ANY exclude rule. Keep the runtime
        // aligned with that contract so multi-rule configs authored
        // against the current schema behave as the schema text advertises.
        let require_ok = self.require.is_empty() || self.require.iter().any(|f| f.matches(episode));
        let exclude_ok =
            self.exclude.is_empty() || !self.exclude.iter().any(|f| f.matches(episode));
        require_ok && exclude_ok
    }
}
use crate::resolvers::Resolver;
use crate::services::episode_sorter::sort_episode_ids_by_published_at;
use crate::services::group_sorter::sort_groups;
use crate::services::helpers::parse_year_binding;

/// Service that orchestrates the smart playlist resolver chain.
///
/// When a PresetConfig matches the podcast, its playlist definitions
/// are used to route episodes through the appropriate resolvers.
/// Otherwise, resolvers are tried in order with no definition
/// (auto-detect mode).
pub struct ResolverService {
    resolvers: Vec<Box<dyn Resolver>>,
    patterns: Vec<PresetConfig>,
}

impl ResolverService {
    pub fn new(resolvers: Vec<Box<dyn Resolver>>, patterns: Vec<PresetConfig>) -> Self {
        Self {
            resolvers,
            patterns,
        }
    }

    /// Attempts to group episodes into smart playlists.
    ///
    /// Returns None if no resolver succeeds or episodes are empty.
    pub fn resolve_smart_playlists(
        &self,
        podcast_guid: Option<&str>,
        feed_url: &str,
        episodes: &[&dyn EpisodeData],
    ) -> Option<Grouping> {
        if episodes.is_empty() {
            return None;
        }

        let episode_by_id: HashMap<i64, &dyn EpisodeData> =
            episodes.iter().map(|&e| (e.id(), e)).collect();

        if let Some(config) = self.find_matching_config(podcast_guid, feed_url) {
            let result = self.resolve_with_config(config, episodes, &episode_by_id);
            return result.map(|r| self.sort_grouping_episodes(&r, &episode_by_id));
        }

        // Fallback: try resolvers in order with no definition
        for resolver in &self.resolvers {
            if let Some(result) = resolver.resolve(episodes, None) {
                return Some(self.sort_grouping_episodes(&result, &episode_by_id));
            }
        }

        None
    }

    /// Resolves smart playlists for preview, tracking which episodes
    /// each definition lost to higher-priority definitions.
    ///
    /// Returns None if no config matches or episodes are empty.
    pub fn resolve_for_preview(
        &self,
        podcast_guid: Option<&str>,
        feed_url: &str,
        episodes: &[&dyn EpisodeData],
    ) -> Option<PreviewGrouping> {
        if episodes.is_empty() {
            return None;
        }

        let config = self.find_matching_config(podcast_guid, feed_url)?;
        let episode_by_id: HashMap<i64, &dyn EpisodeData> =
            episodes.iter().map(|&e| (e.id(), e)).collect();

        let result = self.resolve_with_config_for_preview(config, episodes, &episode_by_id)?;
        Some(self.sort_preview_grouping(&result, &episode_by_id))
    }

    fn resolve_with_config(
        &self,
        config: &PresetConfig,
        episodes: &[&dyn EpisodeData],
        episode_by_id: &HashMap<i64, &dyn EpisodeData>,
    ) -> Option<Grouping> {
        let mut all_playlists: Vec<Playlist> = Vec::new();
        let mut all_ungrouped_ids: HashSet<i64> = HashSet::new();
        let mut claimed_ids: HashSet<i64> = HashSet::new();
        let mut resolver_type: Option<String> = None;

        let sorted = Self::sort_by_processing_order(&config.playlists);

        for definition in &sorted {
            let compiled_filters = CompiledFilters::compile(definition);
            let filtered = Self::filter_episodes(episodes, &compiled_filters, &claimed_ids);
            if filtered.is_empty() {
                continue;
            }

            let resolver = match self.find_resolver_by_type(definition.grouping.by.as_str()) {
                Some(r) => r,
                None => continue,
            };

            // Per-definition enrichment: apply this definition's numbering
            // extractor so non-preview resolution sees the same extracted
            // season/episode numbers that preview already uses. Without this,
            // season-based grouping that relies on titles/descriptions
            // behaves differently in production vs preview.
            let enriched_storage = Self::enrich_for_definition(definition, &filtered);
            let resolve_slice: Vec<&dyn EpisodeData> = match &enriched_storage {
                Some(enriched) => enriched.iter().map(|e| e as &dyn EpisodeData).collect(),
                None => filtered,
            };

            let result = match resolver.resolve(&resolve_slice, Some(definition)) {
                Some(r) => r,
                None => continue,
            };

            if resolver_type.is_none() {
                resolver_type = Some(result.resolver_type.clone());
            }

            // Overlay the enriched episodes on top of the raw lookup so the
            // partition helpers (which read season_number / published_at)
            // see the same values the resolver grouped on. Classifier-level
            // extractors layer on top of the playlist-level pass so that
            // partition bucketing honors per-classifier season overrides.
            let classifier_enriched =
                Self::enrich_for_classifiers(definition, &result, episode_by_id);
            let overlay_storage: HashMap<i64, &dyn EpisodeData> = Self::build_overlay_episode_by_id(
                episode_by_id,
                enriched_storage.as_deref(),
                classifier_enriched.as_deref(),
            );

            self.add_grouped_playlist(&mut all_playlists, definition, &result, &overlay_storage);

            all_ungrouped_ids.extend(&result.ungrouped_episode_ids);

            if definition.has_filters() {
                for p in &result.playlists {
                    for &id in &p.episode_ids {
                        claimed_ids.insert(id);
                    }
                }
            }
        }

        if all_playlists.is_empty() {
            return None;
        }

        for &id in &claimed_ids {
            all_ungrouped_ids.remove(&id);
        }

        Some(Grouping {
            playlists: all_playlists,
            ungrouped_episode_ids: all_ungrouped_ids.into_iter().collect(),
            resolver_type: resolver_type.unwrap_or_else(|| "config".to_string()),
        })
    }

    fn resolve_with_config_for_preview(
        &self,
        config: &PresetConfig,
        episodes: &[&dyn EpisodeData],
        episode_by_id: &HashMap<i64, &dyn EpisodeData>,
    ) -> Option<PreviewGrouping> {
        let mut playlist_results: Vec<PlaylistPreviewResult> = Vec::new();
        let mut all_ungrouped_ids: HashSet<i64> = HashSet::new();
        let mut claimed_ids: HashSet<i64> = HashSet::new();
        let mut claimed_by_map: HashMap<i64, String> = HashMap::new();
        let mut resolver_type: Option<String> = None;

        let sorted = Self::sort_by_processing_order(&config.playlists);

        for definition in &sorted {
            let compiled_filters = CompiledFilters::compile(definition);
            let claimed_by_others = Self::compute_claimed_by_others(
                definition,
                episodes,
                &compiled_filters,
                &claimed_ids,
                &claimed_by_map,
            );

            let filtered = Self::filter_episodes(episodes, &compiled_filters, &claimed_ids);

            if filtered.is_empty() {
                self.add_empty_preview_result(&mut playlist_results, definition, claimed_by_others);
                continue;
            }

            let resolver = match self.find_resolver_by_type(definition.grouping.by.as_str()) {
                Some(r) => r,
                None => continue,
            };

            // Per-definition enrichment: apply this definition's episode
            // extractor so each resolver sees correctly extracted
            // season/episode numbers for its own definition.
            let enriched_storage = Self::enrich_for_definition(definition, &filtered);
            let resolve_slice: Vec<&dyn EpisodeData> = match &enriched_storage {
                Some(enriched) => enriched.iter().map(|e| e as &dyn EpisodeData).collect(),
                None => filtered,
            };

            let result = match resolver.resolve(&resolve_slice, Some(definition)) {
                Some(r) => r,
                None => continue,
            };

            if resolver_type.is_none() {
                resolver_type = Some(result.resolver_type.clone());
            }

            // Overlay the enriched episodes on the lookup so partition
            // helpers (season/year) read the same season_number the
            // resolver used for grouping instead of the raw feed value.
            // Classifier-level extractors layer on top for episodes that
            // landed in their classifier group, matching how the preview
            // serializer enriches per-classifier display values.
            let classifier_enriched =
                Self::enrich_for_classifiers(definition, &result, episode_by_id);
            let overlay_storage: HashMap<i64, &dyn EpisodeData> = Self::build_overlay_episode_by_id(
                episode_by_id,
                enriched_storage.as_deref(),
                classifier_enriched.as_deref(),
            );

            let playlist = self.build_preview_playlist(
                definition,
                &result,
                playlist_results.len() as i32,
                &overlay_storage,
            );

            playlist_results.push(PlaylistPreviewResult {
                definition_id: definition.id.clone(),
                playlist,
                claimed_by_others,
            });

            all_ungrouped_ids.extend(&result.ungrouped_episode_ids);

            if definition.has_filters() {
                self.claim_episodes(
                    &result,
                    &definition.id,
                    &mut claimed_ids,
                    &mut claimed_by_map,
                );
            }
        }

        if playlist_results.is_empty() {
            return None;
        }

        for &id in &claimed_ids {
            all_ungrouped_ids.remove(&id);
        }

        Some(PreviewGrouping {
            playlist_results,
            ungrouped_episode_ids: all_ungrouped_ids.into_iter().collect(),
            resolver_type: resolver_type.unwrap_or_else(|| "config".to_string()),
        })
    }

    fn compute_claimed_by_others(
        definition: &PlaylistDefinition,
        episodes: &[&dyn EpisodeData],
        compiled_filters: &Option<CompiledFilters>,
        claimed_ids: &HashSet<i64>,
        claimed_by_map: &HashMap<i64, String>,
    ) -> HashMap<i64, String> {
        let mut claimed_by_others: HashMap<i64, String> = HashMap::new();
        if !definition.has_filters() {
            return claimed_by_others;
        }

        let all_candidates = Self::filter_episodes(episodes, compiled_filters, &HashSet::new());
        for ep in &all_candidates {
            let id = ep.id();
            if claimed_ids.contains(&id)
                && let Some(claimer) = claimed_by_map.get(&id)
            {
                claimed_by_others.insert(id, claimer.clone());
            }
        }
        claimed_by_others
    }

    fn add_empty_preview_result(
        &self,
        results: &mut Vec<PlaylistPreviewResult>,
        definition: &PlaylistDefinition,
        claimed_by_others: HashMap<i64, String>,
    ) {
        results.push(PlaylistPreviewResult {
            definition_id: definition.id.clone(),
            playlist: Playlist::new(
                definition.id.clone(),
                definition.display_name.clone(),
                results.len() as i32,
                Vec::new(),
            ),
            claimed_by_others,
        });
    }

    fn build_preview_playlist(
        &self,
        definition: &PlaylistDefinition,
        result: &Grouping,
        sort_key: i32,
        episode_by_id: &HashMap<i64, &dyn EpisodeData>,
    ) -> Playlist {
        let presentation = Presentation::Combined;
        let year_binding = parse_year_binding(
            definition
                .group_listing
                .as_ref()
                .and_then(|gl| gl.year_binding.as_deref()),
        );
        // Pin every resolved group to its earliest year when the playlist
        // opts in via `groupItem.pinToYear`, so consumers that render year
        // sections honor the toggle instead of treating it as a no-op.
        let year_override = Self::year_override_from_definition(definition);

        let unsorted_groups: Vec<PlaylistGroup> = result
            .playlists
            .iter()
            .map(|p| PlaylistGroup {
                id: p.id.clone(),
                display_name: p.display_name.clone(),
                sort_key: p.sort_key,
                episode_ids: p.episode_ids.clone(),
                thumbnail_url: p.thumbnail_url.clone(),
                year_override: year_override.clone(),
                show_year_headers: None,
                show_date_range: false,
                earliest_date: None,
                latest_date: None,
                total_duration_ms: None,
                sub_groups: None,
            })
            .collect();

        let sort_rule = definition
            .group_listing
            .as_ref()
            .and_then(|gl| gl.sort.as_ref());
        let groups = sort_groups(&unsorted_groups, sort_rule, episode_by_id);

        // Apply partitioning if configured
        let groups = match definition
            .selector
            .as_ref()
            .and_then(|s| s.partition_by.as_deref())
        {
            Some("seasonNumber") => {
                let title_ext = definition
                    .selector
                    .as_ref()
                    .and_then(|s| s.title_extractor.as_ref());
                Self::partition_groups_by_season(&groups, episode_by_id, title_ext)
            }
            Some("year") => {
                let title_ext = definition
                    .selector
                    .as_ref()
                    .and_then(|s| s.title_extractor.as_ref());
                Self::partition_groups_by_year(&groups, episode_by_id, title_ext)
            }
            _ => groups,
        };

        let all_episode_ids: Vec<i64> = groups
            .iter()
            .flat_map(|g| g.episode_ids.iter())
            .copied()
            .collect();

        Playlist {
            id: definition.id.clone(),
            display_name: definition.display_name.clone(),
            sort_key,
            episode_ids: all_episode_ids,
            thumbnail_url: None,
            presentation,
            year_binding,
            show_year_headers: definition
                .episode_listing
                .as_ref()
                .and_then(|el| el.show_year_headers)
                .unwrap_or(false),
            show_date_range: definition
                .group_item
                .as_ref()
                .and_then(|gi| gi.show_date_range)
                .unwrap_or(false),
            groups: Some(groups),
        }
    }

    fn claim_episodes(
        &self,
        result: &Grouping,
        definition_id: &str,
        claimed_ids: &mut HashSet<i64>,
        claimed_by_map: &mut HashMap<i64, String>,
    ) {
        for p in &result.playlists {
            for &id in &p.episode_ids {
                claimed_ids.insert(id);
                claimed_by_map.insert(id, definition_id.to_string());
            }
        }
    }

    fn add_grouped_playlist(
        &self,
        all_playlists: &mut Vec<Playlist>,
        definition: &PlaylistDefinition,
        result: &Grouping,
        episode_by_id: &HashMap<i64, &dyn EpisodeData>,
    ) {
        let presentation = Presentation::Combined;
        let year_binding = parse_year_binding(
            definition
                .group_listing
                .as_ref()
                .and_then(|gl| gl.year_binding.as_deref()),
        );
        let group_def_map: HashMap<&str, &GroupDef> = definition
            .grouping
            .static_classifiers
            .as_ref()
            .map(|gs| gs.iter().map(|g| (g.id.as_str(), g)).collect())
            .unwrap_or_default();

        // Same pinToYear propagation as the preview path so runtime and
        // preview agree on the emitted year_override.
        let year_override = Self::year_override_from_definition(definition);

        let unsorted_groups: Vec<PlaylistGroup> = result
            .playlists
            .iter()
            .map(|p| {
                let g_def = group_def_map.get(p.id.as_str());
                PlaylistGroup {
                    id: p.id.clone(),
                    display_name: p.display_name.clone(),
                    sort_key: p.sort_key,
                    episode_ids: p.episode_ids.clone(),
                    thumbnail_url: p.thumbnail_url.clone(),
                    year_override: year_override.clone(),
                    show_year_headers: g_def.and_then(|d| {
                        d.episode_listing
                            .as_ref()
                            .and_then(|el| el.show_year_headers)
                    }),
                    show_date_range: g_def
                        .and_then(|d| d.group_item.as_ref().and_then(|g| g.show_date_range))
                        .unwrap_or_else(|| {
                            definition
                                .group_item
                                .as_ref()
                                .and_then(|gi| gi.show_date_range)
                                .unwrap_or(false)
                        }),
                    earliest_date: None,
                    latest_date: None,
                    total_duration_ms: None,
                    sub_groups: None,
                }
            })
            .collect();

        let sort_rule = definition
            .group_listing
            .as_ref()
            .and_then(|gl| gl.sort.as_ref());
        let groups = sort_groups(&unsorted_groups, sort_rule, episode_by_id);

        // Apply partitioning if configured
        let groups = match definition
            .selector
            .as_ref()
            .and_then(|s| s.partition_by.as_deref())
        {
            Some("seasonNumber") => {
                let title_ext = definition
                    .selector
                    .as_ref()
                    .and_then(|s| s.title_extractor.as_ref());
                Self::partition_groups_by_season(&groups, episode_by_id, title_ext)
            }
            Some("year") => {
                let title_ext = definition
                    .selector
                    .as_ref()
                    .and_then(|s| s.title_extractor.as_ref());
                Self::partition_groups_by_year(&groups, episode_by_id, title_ext)
            }
            _ => groups,
        };

        let all_episode_ids: Vec<i64> = groups
            .iter()
            .flat_map(|g| g.episode_ids.iter())
            .copied()
            .collect();

        all_playlists.push(Playlist {
            id: definition.id.clone(),
            display_name: definition.display_name.clone(),
            sort_key: all_playlists.len() as i32,
            episode_ids: all_episode_ids,
            thumbnail_url: None,
            presentation: presentation.clone(),
            year_binding: year_binding.clone(),
            show_year_headers: definition
                .episode_listing
                .as_ref()
                .and_then(|el| el.show_year_headers)
                .unwrap_or(false),
            show_date_range: definition
                .group_item
                .as_ref()
                .and_then(|gi| gi.show_date_range)
                .unwrap_or(false),
            groups: Some(groups),
        });
    }

    fn sort_grouping_episodes(
        &self,
        grouping: &Grouping,
        episode_by_id: &HashMap<i64, &dyn EpisodeData>,
    ) -> Grouping {
        let sorted_playlists: Vec<Playlist> = grouping
            .playlists
            .iter()
            .map(|playlist| {
                // Recursively sort so partition sub-groups (where the real
                // renderable groups live) get their episode_ids sorted too.
                let sorted_groups = playlist
                    .groups
                    .as_ref()
                    .map(|gs| Self::sort_groups_recursive(gs, episode_by_id));

                Playlist {
                    episode_ids: sort_episode_ids_by_published_at(
                        &playlist.episode_ids,
                        episode_by_id,
                    ),
                    groups: sorted_groups,
                    ..playlist.clone()
                }
            })
            .collect();

        Grouping {
            playlists: sorted_playlists,
            ungrouped_episode_ids: sort_episode_ids_by_published_at(
                &grouping.ungrouped_episode_ids,
                episode_by_id,
            ),
            resolver_type: grouping.resolver_type.clone(),
        }
    }

    /// Sorts every group's episode_ids by publish date and descends into
    /// `sub_groups` so partition containers don't mask unsorted children.
    fn sort_groups_recursive(
        groups: &[PlaylistGroup],
        episode_by_id: &HashMap<i64, &dyn EpisodeData>,
    ) -> Vec<PlaylistGroup> {
        groups
            .iter()
            .map(|group| PlaylistGroup {
                episode_ids: sort_episode_ids_by_published_at(&group.episode_ids, episode_by_id),
                sub_groups: group
                    .sub_groups
                    .as_ref()
                    .map(|sg| Self::sort_groups_recursive(sg, episode_by_id)),
                ..group.clone()
            })
            .collect()
    }

    fn sort_preview_grouping(
        &self,
        grouping: &PreviewGrouping,
        episode_by_id: &HashMap<i64, &dyn EpisodeData>,
    ) -> PreviewGrouping {
        let sorted_results: Vec<PlaylistPreviewResult> = grouping
            .playlist_results
            .iter()
            .map(|preview_result| {
                let playlist = &preview_result.playlist;
                let sorted_groups = playlist
                    .groups
                    .as_ref()
                    .map(|gs| Self::sort_groups_recursive(gs, episode_by_id));

                PlaylistPreviewResult {
                    definition_id: preview_result.definition_id.clone(),
                    playlist: Playlist {
                        episode_ids: sort_episode_ids_by_published_at(
                            &playlist.episode_ids,
                            episode_by_id,
                        ),
                        groups: sorted_groups,
                        ..playlist.clone()
                    },
                    claimed_by_others: preview_result.claimed_by_others.clone(),
                }
            })
            .collect();

        PreviewGrouping {
            playlist_results: sorted_results,
            ungrouped_episode_ids: sort_episode_ids_by_published_at(
                &grouping.ungrouped_episode_ids,
                episode_by_id,
            ),
            resolver_type: grouping.resolver_type.clone(),
        }
    }

    /// Filters episodes based on precompiled definition filters.
    ///
    /// Episodes already claimed by a higher-priority definition are excluded.
    /// A definition with no filters acts as a fallback, receiving all
    /// unclaimed episodes.
    fn filter_episodes<'a>(
        episodes: &[&'a dyn EpisodeData],
        compiled_filters: &Option<CompiledFilters>,
        claimed_ids: &HashSet<i64>,
    ) -> Vec<&'a dyn EpisodeData> {
        let unclaimed: Vec<&'a dyn EpisodeData> = episodes
            .iter()
            .filter(|e| !claimed_ids.contains(&e.id()))
            .copied()
            .collect();

        match compiled_filters {
            Some(filters) => unclaimed
                .into_iter()
                .filter(|episode| filters.matches(*episode))
                .collect(),
            None => unclaimed,
        }
    }

    fn find_matching_config(&self, guid: Option<&str>, feed_url: &str) -> Option<&PresetConfig> {
        self.patterns
            .iter()
            .find(|config| config.matches_podcast(guid, feed_url))
    }

    /// Maps the playlist-level `groupItem.pinToYear` toggle to a
    /// `year_override` that downstream consumers can apply per group.
    /// Returns `None` when the toggle is off or unset, keeping serialized
    /// output minimal.
    fn year_override_from_definition(definition: &PlaylistDefinition) -> Option<YearBinding> {
        let pin_to_year = definition
            .group_item
            .as_ref()
            .and_then(|gi| gi.pin_to_year)
            .unwrap_or(false);
        if pin_to_year {
            Some(YearBinding::PinToYear)
        } else {
            None
        }
    }

    /// Builds an episode lookup that prefers enriched values (extracted
    /// season/episode numbers) over the raw feed episodes. Used by
    /// partition helpers so they observe the same season_number the
    /// resolver grouped on. The classifier layer overrides the playlist
    /// layer for any overlapping episode id so per-classifier extractors
    /// win over the playlist-level default.
    fn build_overlay_episode_by_id<'a>(
        base: &HashMap<i64, &'a dyn EpisodeData>,
        enriched: Option<&'a [SimpleEpisodeData]>,
        classifier_enriched: Option<&'a [SimpleEpisodeData]>,
    ) -> HashMap<i64, &'a dyn EpisodeData> {
        let mut out: HashMap<i64, &'a dyn EpisodeData> = base.clone();
        if let Some(enriched) = enriched {
            for ep in enriched {
                out.insert(ep.id, ep as &dyn EpisodeData);
            }
        }
        if let Some(enriched) = classifier_enriched {
            for ep in enriched {
                out.insert(ep.id, ep as &dyn EpisodeData);
            }
        }
        out
    }

    /// Applies per-classifier numbering extractors to episodes that landed
    /// in their matching classifier group, layering the result on top of
    /// an already-enriched slice produced by `enrich_for_definition`.
    ///
    /// Returns `None` when nothing needed enriching. Returned episodes
    /// override any playlist-level enrichment for the same episode id,
    /// so downstream partition helpers see the season_number the
    /// classifier's own extractor produced (matching what preview
    /// renders for that classifier's episodes).
    fn enrich_for_classifiers(
        definition: &PlaylistDefinition,
        result: &Grouping,
        episode_by_id: &HashMap<i64, &dyn EpisodeData>,
    ) -> Option<Vec<SimpleEpisodeData>> {
        let classifiers = definition.grouping.static_classifiers.as_ref()?;
        if classifiers.is_empty() {
            return None;
        }

        let mut classifier_by_id: HashMap<&str, &GroupDef> = HashMap::new();
        for c in classifiers {
            classifier_by_id.insert(c.id.as_str(), c);
        }

        let mut out: Vec<SimpleEpisodeData> = Vec::new();
        for resolved_group in &result.playlists {
            let Some(classifier) = classifier_by_id.get(resolved_group.id.as_str()) else {
                continue;
            };
            let Some(extractor) = classifier.numbering_extractor.as_ref() else {
                continue;
            };
            let compiled = extractor.compile();
            for &id in &resolved_group.episode_ids {
                let Some(ep) = episode_by_id.get(&id) else {
                    continue;
                };
                let r = compiled.extract(*ep);
                if !r.has_values() {
                    continue;
                }
                out.push(SimpleEpisodeData {
                    id: ep.id(),
                    title: ep.title().to_string(),
                    description: ep.description().map(|s| s.to_string()),
                    season_number: r.season_number.or(ep.season_number()),
                    episode_number: r.episode_number.or(ep.episode_number()),
                    published_at: ep.published_at(),
                    image_url: ep.image_url().map(|s| s.to_string()),
                });
            }
        }

        if out.is_empty() { None } else { Some(out) }
    }

    /// Enriches episodes using the definition's own episode extractor.
    /// Group-level extractors are intentionally excluded from this pass:
    /// they are applied as a second layer via `enrich_for_classifiers`
    /// only for episodes that actually landed in their classifier group,
    /// so cross-group scan order cannot misroute them.  Returns `None`
    /// when the definition has no extractor, letting callers skip the copy.
    fn enrich_for_definition(
        definition: &PlaylistDefinition,
        episodes: &[&dyn EpisodeData],
    ) -> Option<Vec<SimpleEpisodeData>> {
        let extractor = definition.grouping.numbering_extractor.as_ref()?;

        let compiled = extractor.compile();
        Some(
            episodes
                .iter()
                .map(|ep| {
                    let result = compiled.extract(*ep);
                    SimpleEpisodeData {
                        id: ep.id(),
                        title: ep.title().to_string(),
                        description: ep.description().map(|s| s.to_string()),
                        season_number: if result.has_values() {
                            result.season_number.or(ep.season_number())
                        } else {
                            ep.season_number()
                        },
                        episode_number: if result.has_values() {
                            result.episode_number.or(ep.episode_number())
                        } else {
                            ep.episode_number()
                        },
                        published_at: ep.published_at(),
                        image_url: ep.image_url().map(|s| s.to_string()),
                    }
                })
                .collect(),
        )
    }

    fn find_resolver_by_type(&self, resolver_type: &str) -> Option<&dyn Resolver> {
        self.resolvers
            .iter()
            .find(|r| r.resolver_type() == resolver_type)
            .map(|r| r.as_ref())
    }

    /// Sorts definitions so filtered definitions process before fallbacks.
    /// Within each group, sorts by priority ascending (lower number first).
    fn sort_by_processing_order(definitions: &[PlaylistDefinition]) -> Vec<&PlaylistDefinition> {
        let mut filtered: Vec<&PlaylistDefinition> = Vec::new();
        let mut fallbacks: Vec<&PlaylistDefinition> = Vec::new();

        for def in definitions {
            if def.has_filters() {
                filtered.push(def);
            } else {
                fallbacks.push(def);
            }
        }

        filtered.sort_by_key(|a| a.priority);
        fallbacks.sort_by_key(|a| a.priority);

        filtered.extend(fallbacks);
        filtered
    }

    /// Partitions groups by episode season number.
    ///
    /// Each resolver group is assigned to the season of its earliest
    /// episode (pin behavior). The result is a list of partition groups
    /// where each partition's `sub_groups` contains the original groups
    /// that belong to that season.
    fn partition_groups_by_season(
        groups: &[PlaylistGroup],
        episode_by_id: &HashMap<i64, &dyn EpisodeData>,
        title_extractor: Option<&TitleExtractor>,
    ) -> Vec<PlaylistGroup> {
        // Assign each group to a season based on its first episode
        let mut season_map: BTreeMap<i32, Vec<PlaylistGroup>> = BTreeMap::new();
        for group in groups {
            let season = group
                .episode_ids
                .iter()
                .filter_map(|id| episode_by_id.get(id))
                .filter_map(|ep| ep.season_number())
                .next()
                .unwrap_or(0);
            season_map.entry(season).or_default().push(group.clone());
        }

        season_map
            .iter()
            .map(|(&season, sub_groups)| {
                let display_name = title_extractor
                    .and_then(|ext| {
                        // Find a representative episode for title extraction
                        let rep_ep = sub_groups
                            .first()
                            .and_then(|g| g.episode_ids.first())
                            .and_then(|id| episode_by_id.get(id));
                        rep_ep.and_then(|ep| ext.extract(*ep))
                    })
                    .unwrap_or_else(|| format!("Season {}", season));

                let all_ids: Vec<i64> = sub_groups
                    .iter()
                    .flat_map(|g| g.episode_ids.iter())
                    .copied()
                    .collect();

                PlaylistGroup {
                    id: format!("season_{}", season),
                    display_name,
                    // Preserve the actual season number so UI labels like
                    // "S13" stay correct for non-contiguous seasons.
                    sort_key: season,
                    episode_ids: all_ids,
                    thumbnail_url: None,
                    year_override: None,
                    show_year_headers: None,
                    show_date_range: false,
                    earliest_date: None,
                    latest_date: None,
                    total_duration_ms: None,
                    sub_groups: Some(sub_groups.clone()),
                }
            })
            .collect()
    }

    /// Partitions groups by episode publication year.
    fn partition_groups_by_year(
        groups: &[PlaylistGroup],
        episode_by_id: &HashMap<i64, &dyn EpisodeData>,
        title_extractor: Option<&TitleExtractor>,
    ) -> Vec<PlaylistGroup> {
        let mut year_map: BTreeMap<i32, Vec<PlaylistGroup>> = BTreeMap::new();
        for group in groups {
            let year = group
                .episode_ids
                .iter()
                .filter_map(|id| episode_by_id.get(id))
                .filter_map(|ep| ep.published_at())
                .map(|dt| dt.format("%Y").to_string().parse::<i32>().unwrap_or(0))
                .next()
                .unwrap_or(0);
            year_map.entry(year).or_default().push(group.clone());
        }

        year_map
            .iter()
            .map(|(&year, sub_groups)| {
                let display_name = title_extractor
                    .and_then(|ext| {
                        let rep_ep = sub_groups
                            .first()
                            .and_then(|g| g.episode_ids.first())
                            .and_then(|id| episode_by_id.get(id));
                        rep_ep.and_then(|ep| ext.extract(*ep))
                    })
                    .unwrap_or_else(|| format!("{}", year));

                let all_ids: Vec<i64> = sub_groups
                    .iter()
                    .flat_map(|g| g.episode_ids.iter())
                    .copied()
                    .collect();

                PlaylistGroup {
                    id: format!("year_{}", year),
                    display_name,
                    // Use the actual year as sort_key so downstream
                    // consumers that rely on it see a real year value.
                    sort_key: year,
                    episode_ids: all_ids,
                    thumbnail_url: None,
                    year_override: None,
                    show_year_headers: None,
                    show_date_range: false,
                    earliest_date: None,
                    latest_date: None,
                    total_duration_ms: None,
                    sub_groups: Some(sub_groups.clone()),
                }
            })
            .collect()
    }
}
