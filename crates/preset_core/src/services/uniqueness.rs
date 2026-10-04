use crate::models::PresetMeta;

/// A conflict found during cross-pattern uniqueness validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UniquenessConflict {
    /// The field that has a duplicate value.
    pub field: &'static str,
    /// The duplicated value.
    pub value: String,
    /// The pattern ID that already claims this value.
    pub claimed_by: String,
}

impl std::fmt::Display for UniquenessConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} \"{}\" is already used by pattern \"{}\"",
            self.field, self.value, self.claimed_by
        )
    }
}

fn find_guid_conflict(candidate: &PresetMeta, others: &[PresetMeta]) -> Option<UniquenessConflict> {
    let guid = candidate.podcast_guid.as_deref()?;
    others.iter().find_map(|other| {
        (other.id != candidate.id && other.podcast_guid.as_deref() == Some(guid)).then(|| {
            UniquenessConflict {
                field: "podcastGuid",
                value: guid.to_string(),
                claimed_by: other.id.clone(),
            }
        })
    })
}

fn find_feed_url_conflicts(
    candidate: &PresetMeta,
    others: &[PresetMeta],
) -> Vec<UniquenessConflict> {
    let mut seen = std::collections::HashSet::new();
    candidate
        .feed_urls
        .iter()
        .filter(|url| seen.insert(url.as_str()))
        .filter_map(|url| {
            others.iter().find_map(|other| {
                (other.id != candidate.id && other.feed_urls.contains(url)).then(|| {
                    UniquenessConflict {
                        field: "feedUrls",
                        value: url.clone(),
                        claimed_by: other.id.clone(),
                    }
                })
            })
        })
        .collect()
}

/// Checks that the given pattern's `podcastGuid` and `feedUrls` do not
/// overlap with any other pattern in `others`.
///
/// Entries in `others` whose `id` matches `candidate.id` are skipped,
/// so callers need not pre-filter the list.
pub fn check_uniqueness(candidate: &PresetMeta, others: &[PresetMeta]) -> Vec<UniquenessConflict> {
    let mut conflicts: Vec<UniquenessConflict> =
        find_guid_conflict(candidate, others).into_iter().collect();
    conflicts.extend(find_feed_url_conflicts(candidate, others));
    conflicts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn meta(id: &str, guid: Option<&str>, feed_urls: &[&str]) -> PresetMeta {
        PresetMeta {
            data_version: 1,
            id: id.to_string(),
            podcast_guid: guid.map(|s| s.to_string()),
            feed_urls: feed_urls.iter().map(|s| s.to_string()).collect(),
            year_grouped_episodes: false,
            show_episode_thumbnail: None,
            playlists: vec!["p1".to_string()],
        }
    }

    #[test]
    fn no_conflict_when_no_overlap() {
        let candidate = meta("a", Some("guid-a"), &["https://a.com/feed"]);
        let others = vec![meta("b", Some("guid-b"), &["https://b.com/feed"])];
        assert!(check_uniqueness(&candidate, &others).is_empty());
    }

    #[test]
    fn detects_duplicate_podcast_guid() {
        let candidate = meta("a", Some("guid-shared"), &["https://a.com/feed"]);
        let others = vec![meta("b", Some("guid-shared"), &["https://b.com/feed"])];
        let conflicts = check_uniqueness(&candidate, &others);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].field, "podcastGuid");
        assert_eq!(conflicts[0].value, "guid-shared");
        assert_eq!(conflicts[0].claimed_by, "b");
    }

    #[test]
    fn detects_duplicate_feed_url() {
        let candidate = meta("a", None, &["https://shared.com/feed"]);
        let others = vec![meta("b", None, &["https://shared.com/feed"])];
        let conflicts = check_uniqueness(&candidate, &others);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].field, "feedUrls");
        assert_eq!(conflicts[0].value, "https://shared.com/feed");
        assert_eq!(conflicts[0].claimed_by, "b");
    }

    #[test]
    fn detects_multiple_conflicts() {
        let candidate = meta(
            "a",
            Some("guid-x"),
            &["https://x.com/feed", "https://y.com/feed"],
        );
        let others = vec![
            meta("b", Some("guid-x"), &["https://b.com/feed"]),
            meta("c", None, &["https://y.com/feed"]),
        ];
        let conflicts = check_uniqueness(&candidate, &others);
        assert_eq!(conflicts.len(), 2);
    }

    #[test]
    fn no_conflict_when_guid_is_none() {
        let candidate = meta("a", None, &["https://a.com/feed"]);
        let others = vec![meta("b", None, &["https://b.com/feed"])];
        assert!(check_uniqueness(&candidate, &others).is_empty());
    }

    #[test]
    fn no_conflict_with_empty_others() {
        let candidate = meta("a", Some("guid-a"), &["https://a.com/feed"]);
        assert!(check_uniqueness(&candidate, &[]).is_empty());
    }

    #[test]
    fn skips_self_in_others() {
        let candidate = meta("a", Some("guid-a"), &["https://a.com/feed"]);
        let others = vec![candidate.clone()];
        assert!(check_uniqueness(&candidate, &others).is_empty());
    }

    #[test]
    fn reports_first_match_per_feed_url() {
        let candidate = meta("a", None, &["https://shared.com/feed"]);
        let others = vec![
            meta("b", None, &["https://shared.com/feed"]),
            meta("c", None, &["https://shared.com/feed"]),
        ];
        let conflicts = check_uniqueness(&candidate, &others);
        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].claimed_by, "b");
    }
}
