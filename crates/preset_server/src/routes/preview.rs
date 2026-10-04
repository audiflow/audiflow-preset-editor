use std::collections::{HashMap, HashSet};

use axum::Json;
use axum::extract::State;
use serde_json::Value;

use crate::app::{AppError, SharedState};
use preset_core::models::{
    EpisodeData, GroupDef, NumberingExtractor, Playlist, PlaylistGroup, PlaylistPreviewResult,
    PresetConfig, PreviewGrouping, SimpleEpisodeData,
};
use preset_core::resolvers::{
    CategoryResolver, Resolver, RssResolver, TitleAppearanceResolver, YearResolver,
};
use preset_core::services::ResolverService;

/// POST /api/configs/preview -- previews smart playlists from config + feed.
pub async fn preview_config(
    State(state): State<SharedState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let obj = body
        .as_object()
        .ok_or_else(|| AppError::bad_request("Request body must be a JSON object"))?;

    let config_json = obj
        .get("config")
        .and_then(|v| v.as_object())
        .ok_or_else(|| AppError::bad_request("Missing or invalid \"config\" field"))?;

    let feed_url = obj
        .get("feedUrl")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| AppError::bad_request("Missing or invalid \"feedUrl\" field"))?;

    let mut config: PresetConfig = serde_json::from_value(Value::Object(config_json.clone()))
        .map_err(|e| AppError::bad_request(format!("Invalid config: {e}")))?;
    for playlist in &mut config.playlists {
        playlist.strip_conditional_fields();
    }

    let episode_maps = state
        .feed_cache
        .fetch_feed(feed_url, &state.http_client)
        .await
        .map_err(|e| AppError::bad_gateway(format!("Failed to fetch feed: {e}")))?;

    let mut episodes: Vec<SimpleEpisodeData> = Vec::with_capacity(episode_maps.len());
    let mut parse_failures = 0usize;
    for v in &episode_maps {
        match serde_json::from_value(v.clone()) {
            Ok(ep) => episodes.push(ep),
            Err(_) => parse_failures += 1,
        }
    }

    // Episodes are passed unenriched; the resolver service applies
    // per-definition episode extractors during preview resolution.
    let mut result = run_preview(&config, &episodes, feed_url);

    if 0 < parse_failures
        && let Some(debug) = result.get_mut("debug").and_then(|d| d.as_object_mut())
    {
        debug.insert("parseFailures".to_string(), Value::from(parse_failures));
    }

    Ok(Json(result))
}

fn run_preview(
    config: &PresetConfig,
    episodes: &[SimpleEpisodeData],
    request_feed_url: &str,
) -> Value {
    let episode_refs: Vec<&dyn EpisodeData> =
        episodes.iter().map(|e| e as &dyn EpisodeData).collect();

    let resolvers: Vec<Box<dyn Resolver>> = vec![
        Box::new(RssResolver),
        Box::new(CategoryResolver),
        Box::new(YearResolver),
        Box::new(TitleAppearanceResolver),
    ];

    let service = ResolverService::new(resolvers, vec![config.clone()]);
    let result = service.resolve_for_preview(
        config.podcast_guid.as_deref(),
        config
            .feed_urls
            .as_ref()
            .and_then(|u| u.first())
            .map(|s| s.as_str())
            .unwrap_or(request_feed_url),
        &episode_refs,
    );

    let result = match result {
        Some(r) => r,
        None => {
            return serde_json::json!({
                "playlists": [],
                "ungrouped": [],
                "excluded": [],
                "resolverType": null,
            });
        }
    };

    let episode_by_id: HashMap<i64, &SimpleEpisodeData> =
        episodes.iter().map(|e| (e.id, e)).collect();

    // Pre-compute per-definition enriched episodes (extracted season/episode numbers)
    // and extracted display names so preview serialization reflects what the resolver saw.
    let enriched_episodes = compute_enriched_episodes(config, episodes, &result);
    let extracted_display_names = compute_extracted_display_names(config, episodes, &result);

    let grouped_ids = collect_grouped_ids(&result);
    let ungrouped_set: HashSet<i64> = result.ungrouped_episode_ids.iter().copied().collect();

    let excluded_episodes: Vec<Value> = episodes
        .iter()
        .filter(|e| !grouped_ids.contains(&e.id) && !ungrouped_set.contains(&e.id))
        .map(|e| serialize_episode(e, None))
        .collect();

    let playlists: Vec<Value> = result
        .playlist_results
        .iter()
        .map(|pr| {
            let enriched_by_id = enriched_episodes.get(&pr.definition_id);
            serialize_preview_result(
                pr,
                result.resolver_type.as_str(),
                &episode_by_id,
                enriched_by_id,
                extracted_display_names.get(&pr.definition_id),
            )
        })
        .collect();

    let ungrouped: Vec<Value> = result
        .ungrouped_episode_ids
        .iter()
        .filter_map(|id| episode_by_id.get(id))
        .map(|e| serialize_episode(e, None))
        .collect();

    serde_json::json!({
        "playlists": playlists,
        "ungrouped": ungrouped,
        "excluded": excluded_episodes,
        "resolverType": result.resolver_type,
        "debug": {
            "totalEpisodes": episodes.len(),
            "groupedEpisodes": grouped_ids.len(),
            "ungroupedEpisodes": result.ungrouped_episode_ids.len(),
            "excludedEpisodes": excluded_episodes.len(),
        },
    })
}

/// Applies each definition's episode extractor (and group-level extractors)
/// to produce enriched episodes with extracted season/episode numbers,
/// keyed by definition id then episode id.
///
/// Definition-level extractors apply to all episodes first.
/// Group-level extractors apply only to their group's episodes and
/// override any definition-level enrichment.
fn compute_enriched_episodes(
    config: &PresetConfig,
    episodes: &[SimpleEpisodeData],
    preview: &PreviewGrouping,
) -> HashMap<String, HashMap<i64, SimpleEpisodeData>> {
    // Build a lookup from definition_id -> (group_id -> set of episode IDs).
    // When `selector.partitionBy` is `seasonNumber` or `year`, the real
    // classifier IDs move into nested `sub_groups` under synthetic
    // `season_*`/`year_*` parents, so we must walk the tree to preserve
    // per-classifier IDs for downstream numberingExtractor lookups.
    let group_episode_ids: HashMap<&str, HashMap<&str, HashSet<i64>>> = preview
        .playlist_results
        .iter()
        .map(|pr| {
            let mut groups: HashMap<&str, HashSet<i64>> = HashMap::new();
            if let Some(gs) = pr.playlist.groups.as_ref() {
                for g in gs {
                    collect_group_episode_ids(g, &mut groups);
                }
            }
            (pr.definition_id.as_str(), groups)
        })
        .collect();

    let mut result = HashMap::new();
    for definition in &config.playlists {
        let mut map = HashMap::new();

        // Definition-level extractor: applies to all episodes (skip already-enriched)
        if let Some(ext) = definition.grouping.numbering_extractor.as_ref() {
            enrich_with_extractor(ext, episodes, &mut map, false);
        }

        // Group-level extractors: scoped to that group's resolved episodes,
        // overwriting any definition-level entry
        if let Some(groups) = definition.grouping.static_classifiers.as_ref()
            && let Some(def_groups) = group_episode_ids.get(definition.id.as_str())
        {
            for group in groups {
                if let Some(ext) = &group.numbering_extractor
                    && let Some(ep_ids) = def_groups.get(group.id.as_str())
                {
                    let group_episodes: Vec<&SimpleEpisodeData> =
                        episodes.iter().filter(|e| ep_ids.contains(&e.id)).collect();
                    enrich_group_episodes(ext, &group_episodes, &mut map);
                }
            }
        }

        if !map.is_empty() {
            result.insert(definition.id.clone(), map);
        }
    }
    result
}

/// Recursively records each group's episode IDs into `out`, walking
/// into `sub_groups` so classifier IDs under synthetic season/year
/// partitions are still captured.
fn collect_group_episode_ids<'a>(
    group: &'a PlaylistGroup,
    out: &mut HashMap<&'a str, HashSet<i64>>,
) {
    out.entry(group.id.as_str())
        .or_default()
        .extend(group.episode_ids.iter().copied());
    if let Some(sub_groups) = &group.sub_groups {
        for sg in sub_groups {
            collect_group_episode_ids(sg, out);
        }
    }
}

fn enrich_with_extractor(
    extractor: &NumberingExtractor,
    episodes: &[SimpleEpisodeData],
    map: &mut HashMap<i64, SimpleEpisodeData>,
    allow_overwrite: bool,
) {
    let compiled = extractor.compile();
    for ep in episodes {
        if !allow_overwrite && map.contains_key(&ep.id) {
            continue;
        }
        let r = compiled.extract(ep);
        if r.has_values() {
            map.insert(
                ep.id,
                SimpleEpisodeData {
                    id: ep.id,
                    title: ep.title.clone(),
                    description: ep.description.clone(),
                    season_number: r.season_number.or(ep.season_number),
                    episode_number: r.episode_number.or(ep.episode_number),
                    published_at: ep.published_at,
                    image_url: ep.image_url.clone(),
                },
            );
        }
    }
}

/// Like `enrich_with_extractor` but takes a pre-filtered slice of episode
/// references and always overwrites existing entries.
fn enrich_group_episodes(
    extractor: &NumberingExtractor,
    episodes: &[&SimpleEpisodeData],
    map: &mut HashMap<i64, SimpleEpisodeData>,
) {
    let compiled = extractor.compile();
    for ep in episodes {
        let r = compiled.extract(*ep);
        if r.has_values() {
            map.insert(
                ep.id,
                SimpleEpisodeData {
                    id: ep.id,
                    title: ep.title.clone(),
                    description: ep.description.clone(),
                    season_number: r.season_number.or(ep.season_number),
                    episode_number: r.episode_number.or(ep.episode_number),
                    published_at: ep.published_at,
                    image_url: ep.image_url.clone(),
                },
            );
        }
    }
}

/// Computes display names per definition using per-definition enriched
/// episodes so that titleExtractors reading seasonNumber/episodeNumber
/// see the same values the resolver used for grouping. When a classifier
/// sets its own `episodeItem.titleExtractor`, episodes resolved into that
/// classifier group use the override instead of the playlist default so
/// preview reflects the same per-classifier rule consumers honor.
fn compute_extracted_display_names(
    config: &PresetConfig,
    episodes: &[SimpleEpisodeData],
    preview: &PreviewGrouping,
) -> HashMap<String, HashMap<i64, String>> {
    // definition_id -> (episode_id -> classifier_id that claimed it)
    let classifier_by_episode: HashMap<&str, HashMap<i64, &str>> = preview
        .playlist_results
        .iter()
        .map(|pr| {
            let mut m: HashMap<i64, &str> = HashMap::new();
            if let Some(groups) = pr.playlist.groups.as_ref() {
                for g in groups {
                    collect_classifier_episode_ids(g, &mut m);
                }
            }
            (pr.definition_id.as_str(), m)
        })
        .collect();

    let mut result = HashMap::new();
    for definition in &config.playlists {
        let playlist_extractor = definition
            .episode_item
            .as_ref()
            .and_then(|ei| ei.title_extractor.as_ref());

        let classifier_map: HashMap<&str, &GroupDef> = definition
            .grouping
            .static_classifiers
            .as_deref()
            .map(|cs| cs.iter().map(|c| (c.id.as_str(), c)).collect())
            .unwrap_or_default();

        // Short-circuit when no extractor (playlist or per-classifier) applies.
        let has_classifier_extractor = classifier_map.values().any(|c| {
            c.episode_item
                .as_ref()
                .and_then(|ei| ei.title_extractor.as_ref())
                .is_some()
        });
        if playlist_extractor.is_none() && !has_classifier_extractor {
            continue;
        }

        // Enrich episodes with this definition's numbering extractor
        // (mirrors what the resolver does) so title extraction sees
        // the same season/episode numbers the resolver used.
        let enriched: Option<Vec<SimpleEpisodeData>> =
            definition.grouping.numbering_extractor.as_ref().map(|ext| {
                let compiled_ep = ext.compile();
                episodes
                    .iter()
                    .map(|ep| {
                        let r = compiled_ep.extract(ep);
                        if r.has_values() {
                            SimpleEpisodeData {
                                id: ep.id,
                                title: ep.title.clone(),
                                description: ep.description.clone(),
                                season_number: r.season_number.or(ep.season_number),
                                episode_number: r.episode_number.or(ep.episode_number),
                                published_at: ep.published_at,
                                image_url: ep.image_url.clone(),
                            }
                        } else {
                            ep.clone()
                        }
                    })
                    .collect()
            });

        let source: &[SimpleEpisodeData] = enriched.as_deref().unwrap_or(episodes);
        let episode_to_classifier = classifier_by_episode
            .get(definition.id.as_str())
            .cloned()
            .unwrap_or_default();

        let mut names = HashMap::new();
        for episode in source {
            // Prefer the classifier-level extractor when the episode landed
            // in a classifier that defines one; fall back to the playlist
            // default otherwise.
            let classifier = episode_to_classifier
                .get(&episode.id)
                .and_then(|cid| classifier_map.get(cid).copied());
            let classifier_title_extractor = classifier
                .and_then(|c| c.episode_item.as_ref())
                .and_then(|ei| ei.title_extractor.as_ref());
            let extractor = classifier_title_extractor.or(playlist_extractor);
            let Some(extractor) = extractor else {
                continue;
            };

            // Re-enrich this episode with the classifier's own numbering
            // extractor (if any) so a classifier-level titleExtractor
            // reading seasonNumber/episodeNumber sees values that match
            // the group it actually resolved into, not stale playlist-level
            // numbers that may belong to a different classifier.
            let episode_cow = classifier
                .and_then(|c| c.numbering_extractor.as_ref())
                .and_then(|ne| {
                    let r = ne.compile().extract(episode);
                    if !r.has_values() {
                        return None;
                    }
                    Some(SimpleEpisodeData {
                        id: episode.id,
                        title: episode.title.clone(),
                        description: episode.description.clone(),
                        season_number: r.season_number.or(episode.season_number),
                        episode_number: r.episode_number.or(episode.episode_number),
                        published_at: episode.published_at,
                        image_url: episode.image_url.clone(),
                    })
                });
            let episode_for_title: &SimpleEpisodeData = episode_cow.as_ref().unwrap_or(episode);
            if let Some(name) = extractor.compile().extract(episode_for_title) {
                names.insert(episode.id, name);
            }
        }
        result.insert(definition.id.clone(), names);
    }
    result
}

/// Records the classifier id that contains each episode (only direct
/// children, not synthetic season/year partition parents). Partition
/// parents have synthetic ids like `season_*` / `year_*` that do not
/// appear in `grouping.staticClassifiers`, so they are skipped and we
/// recurse into their sub_groups where the real classifier ids live.
fn collect_classifier_episode_ids<'a>(group: &'a PlaylistGroup, out: &mut HashMap<i64, &'a str>) {
    let is_synthetic = group.id.starts_with("season_") || group.id.starts_with("year_");
    if !is_synthetic {
        for &id in &group.episode_ids {
            out.insert(id, group.id.as_str());
        }
    }
    if let Some(sub_groups) = &group.sub_groups {
        for sg in sub_groups {
            collect_classifier_episode_ids(sg, out);
        }
    }
}

fn collect_grouped_ids(result: &PreviewGrouping) -> HashSet<i64> {
    let mut ids = HashSet::new();
    for pr in &result.playlist_results {
        for &id in &pr.playlist.episode_ids {
            ids.insert(id);
        }
        if let Some(groups) = &pr.playlist.groups {
            for group in groups {
                for &id in &group.episode_ids {
                    ids.insert(id);
                }
            }
        }
    }
    ids
}

fn serialize_preview_result(
    pr: &PlaylistPreviewResult,
    resolver_type: &str,
    episode_by_id: &HashMap<i64, &SimpleEpisodeData>,
    enriched_by_id: Option<&HashMap<i64, SimpleEpisodeData>>,
    extracted_names: Option<&HashMap<i64, String>>,
) -> Value {
    let mut base = serialize_playlist(
        &pr.playlist,
        resolver_type,
        episode_by_id,
        enriched_by_id,
        extracted_names,
    );

    if !pr.claimed_by_others.is_empty() {
        let claimed: Vec<Value> = pr
            .claimed_by_others
            .iter()
            .map(|(&id, claimer)| {
                let mut obj = serde_json::Map::new();
                if let Some(episode) = episode_by_id.get(&id) {
                    obj.insert("id".to_string(), Value::from(episode.id));
                    obj.insert("title".to_string(), Value::String(episode.title.clone()));
                    if let Some(sn) = episode.season_number {
                        obj.insert("seasonNumber".to_string(), Value::from(sn));
                    }
                    if let Some(en) = episode.episode_number {
                        obj.insert("episodeNumber".to_string(), Value::from(en));
                    }
                }
                obj.insert("claimedBy".to_string(), Value::String(claimer.clone()));
                Value::Object(obj)
            })
            .collect();
        base["claimedByOthers"] = Value::Array(claimed);
    }

    let filter_matched = pr.playlist.episode_ids.len() + pr.claimed_by_others.len();
    base["debug"] = serde_json::json!({
        "filterMatched": filter_matched,
        "episodeCount": pr.playlist.episode_ids.len(),
        "claimedByOthersCount": pr.claimed_by_others.len(),
    });

    base
}

fn serialize_playlist(
    playlist: &Playlist,
    resolver_type: &str,
    episode_by_id: &HashMap<i64, &SimpleEpisodeData>,
    enriched_by_id: Option<&HashMap<i64, SimpleEpisodeData>>,
    extracted_names: Option<&HashMap<i64, String>>,
) -> Value {
    let mut obj = serde_json::json!({
        "id": playlist.id,
        "displayName": playlist.display_name,
        "sortKey": playlist.sort_key,
        "resolverType": resolver_type,
        "episodeCount": playlist.episode_count(),
        "yearBinding": serde_json::to_value(&playlist.year_binding).unwrap_or(Value::Null),
    });

    if let Some(groups) = &playlist.groups {
        let groups_json: Vec<Value> = groups
            .iter()
            .map(|g| serialize_group(g, episode_by_id, enriched_by_id, extracted_names))
            .collect();
        obj["groups"] = Value::Array(groups_json);
    }

    obj
}

fn serialize_group(
    group: &PlaylistGroup,
    episode_by_id: &HashMap<i64, &SimpleEpisodeData>,
    enriched_by_id: Option<&HashMap<i64, SimpleEpisodeData>>,
    extracted_names: Option<&HashMap<i64, String>>,
) -> Value {
    let episodes_json: Vec<Value> = group
        .episode_ids
        .iter()
        .filter_map(|id| {
            // Use enriched episode (with extracted season/episode numbers) if available
            let ep: &SimpleEpisodeData = enriched_by_id
                .and_then(|m| m.get(id))
                .or_else(|| episode_by_id.get(id).copied())?;
            let name = extracted_names.and_then(|m| m.get(id)).map(|s| s.as_str());
            Some(serialize_episode(ep, name))
        })
        .collect();

    let mut obj = serde_json::json!({
        "id": group.id,
        "displayName": group.display_name,
        "sortKey": group.sort_key,
        "episodeCount": group.episode_count(),
        "episodes": episodes_json,
    });

    if let Some(sub_groups) = &group.sub_groups {
        let sub_groups_json: Vec<Value> = sub_groups
            .iter()
            .map(|sg| serialize_group(sg, episode_by_id, enriched_by_id, extracted_names))
            .collect();
        obj["subGroups"] = Value::Array(sub_groups_json);
    }

    obj
}

fn serialize_episode(episode: &SimpleEpisodeData, extracted_display_name: Option<&str>) -> Value {
    let mut obj = serde_json::Map::new();
    obj.insert("id".to_string(), Value::from(episode.id));
    obj.insert("title".to_string(), Value::String(episode.title.clone()));

    if let Some(dt) = episode.published_at {
        obj.insert("publishedAt".to_string(), Value::String(dt.to_rfc3339()));
    }
    if let Some(sn) = episode.season_number {
        obj.insert("seasonNumber".to_string(), Value::from(sn));
    }
    if let Some(en) = episode.episode_number {
        obj.insert("episodeNumber".to_string(), Value::from(en));
    }
    if let Some(name) = extracted_display_name {
        obj.insert(
            "extractedDisplayName".to_string(),
            Value::String(name.to_string()),
        );
    }

    Value::Object(obj)
}
