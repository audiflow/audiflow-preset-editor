use std::collections::BTreeMap;

use crate::models::{
    EpisodeData, Grouping, Playlist, PlaylistDefinition, SortField, SortOrder, SortRule,
};

use super::resolver::Resolver;

/// Resolver that groups episodes using RSS metadata (seasonNumber field).
pub struct RssResolver;

impl Resolver for RssResolver {
    fn resolver_type(&self) -> &str {
        "seasonNumber"
    }

    fn default_sort(&self) -> SortRule {
        SortRule {
            field: SortField::PlaylistNumber,
            order: SortOrder::Ascending,
        }
    }

    fn resolve(
        &self,
        episodes: &[&dyn EpisodeData],
        definition: Option<&PlaylistDefinition>,
    ) -> Option<Grouping> {
        let title_extractor = definition
            .and_then(|d| d.group_item.as_ref())
            .and_then(|gi| gi.title_extractor.as_ref());

        let mut grouped: BTreeMap<i32, Vec<&dyn EpisodeData>> = BTreeMap::new();
        let mut ungrouped: Vec<i64> = Vec::new();

        for &episode in episodes {
            let season_num = episode.season_number();
            match season_num {
                Some(n) if 1 <= n => {
                    grouped.entry(n).or_default().push(episode);
                }
                _ => {
                    ungrouped.push(episode.id());
                }
            }
        }

        if grouped.is_empty() {
            return None;
        }

        let mut playlists: Vec<Playlist> = grouped
            .iter()
            .map(|(&season_number, eps)| {
                let display_name = super::extract_display_name_with_fallback(
                    format!("Season {}", season_number),
                    eps,
                    title_extractor,
                );
                Playlist::new(
                    format!("season_{}", season_number),
                    display_name,
                    season_number,
                    eps.iter().map(|e| e.id()).collect(),
                )
            })
            .collect();

        playlists.sort_by_key(|a| a.sort_key);

        Some(Grouping {
            playlists,
            ungrouped_episode_ids: ungrouped,
            resolver_type: self.resolver_type().to_string(),
        })
    }
}
