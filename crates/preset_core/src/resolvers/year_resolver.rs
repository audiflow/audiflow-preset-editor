use std::collections::BTreeMap;

use chrono::Datelike;

use crate::models::{
    EpisodeData, Grouping, Playlist, PlaylistDefinition, SortField, SortOrder, SortRule,
};

use super::resolver::Resolver;

/// Resolver that groups episodes by publication year.
pub struct YearResolver;

impl Resolver for YearResolver {
    fn resolver_type(&self) -> &str {
        "year"
    }

    fn default_sort(&self) -> SortRule {
        SortRule {
            field: SortField::PlaylistNumber,
            order: SortOrder::Descending, // Newest years first
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
            match episode.published_at() {
                Some(pub_date) => {
                    let year = pub_date.year();
                    grouped.entry(year).or_default().push(episode);
                }
                None => {
                    ungrouped.push(episode.id());
                }
            }
        }

        if grouped.is_empty() {
            return None;
        }

        let mut playlists: Vec<Playlist> = grouped
            .iter()
            .map(|(&year, eps)| {
                let display_name = super::extract_display_name_with_fallback(
                    year.to_string(),
                    eps,
                    title_extractor,
                );
                Playlist::new(
                    format!("year_{}", year),
                    display_name,
                    year,
                    eps.iter().map(|e| e.id()).collect(),
                )
            })
            .collect();

        // Sort by year descending (newest first)
        playlists.sort_by_key(|a| std::cmp::Reverse(a.sort_key));

        Some(Grouping {
            playlists,
            ungrouped_episode_ids: ungrouped,
            resolver_type: self.resolver_type().to_string(),
        })
    }
}
