use fancy_regex::Regex;
use serde::{Deserialize, Serialize};

use super::episode_data::EpisodeData;

/// Configuration for extracting smart playlist display names from episode data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TitleExtractor {
    /// Episode field to extract from: "title", "description", "seasonNumber", "episodeNumber".
    pub source: String,

    /// Regex pattern to match against the source value (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,

    /// Template using `${N}` references (0 = full match, 1+ = capture groups).
    /// When `None`, behaves as `${0}`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub template: Option<String>,

    /// Fallback extractor to use when this one fails.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback: Option<Box<TitleExtractor>>,

    /// Fallback string value used when `source` is `seasonNumber` or `episodeNumber`
    /// and the episode lacks that number (or the number is `< 1`). Has no effect
    /// for `title` / `description` sources, matching the JSON Schema description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_value: Option<String>,
}

/// A `TitleExtractor` with its regex pattern precompiled.
pub struct CompiledTitleExtractor<'a> {
    extractor: &'a TitleExtractor,
    regex: Option<Regex>,
    fallback: Option<Box<CompiledTitleExtractor<'a>>>,
}

impl TitleExtractor {
    /// Precompiles the regex pattern (and any fallback chain) for reuse.
    pub fn compile(&self) -> CompiledTitleExtractor<'_> {
        let regex = self.pattern.as_ref().and_then(|p| Regex::new(p).ok());
        let fallback = self.fallback.as_ref().map(|f| Box::new(f.compile()));
        CompiledTitleExtractor {
            extractor: self,
            regex,
            fallback,
        }
    }

    /// Extracts the smart playlist title from an episode.
    /// Compiles the regex on every call. For batch use prefer `compile()`.
    pub fn extract(&self, episode: &dyn EpisodeData) -> Option<String> {
        self.compile().extract(episode)
    }

    fn get_source_value(&self, episode: &dyn EpisodeData) -> Option<String> {
        match self.source.as_str() {
            "title" => Some(episode.title().to_string()),
            "description" => episode.description().map(|s| s.to_string()),
            "seasonNumber" => episode.season_number().map(|n| n.to_string()),
            "episodeNumber" => episode.episode_number().map(|n| n.to_string()),
            _ => None,
        }
    }
}

impl<'a> CompiledTitleExtractor<'a> {
    /// Extracts the smart playlist title using the precompiled regex.
    pub fn extract(&self, episode: &dyn EpisodeData) -> Option<String> {
        let ext = self.extractor;

        // Early return when source is a numeric field that's missing or < 1.
        // fallback_value is documented as having no effect for title/description,
        // so it must only short-circuit when source is seasonNumber or episodeNumber.
        if ext.fallback_value.is_some() {
            let numeric = match ext.source.as_str() {
                "seasonNumber" => Some(episode.season_number()),
                "episodeNumber" => Some(episode.episode_number()),
                _ => None,
            };
            if let Some(n) = numeric
                && (n.is_none() || n.is_some_and(|v| v < 1))
            {
                return ext.fallback_value.clone();
            }
        }

        let Some(source_value) = ext.get_source_value(episode) else {
            return self.fallback.as_ref().and_then(|f| f.extract(episode));
        };

        let groups: Vec<Option<String>> = match self.regex.as_ref() {
            Some(regex) => match regex.captures(&source_value).ok().flatten() {
                Some(captures) => (0..captures.len())
                    .map(|i| captures.get(i).map(|m| m.as_str().to_string()))
                    .collect(),
                None => return self.fallback.as_ref().and_then(|f| f.extract(episode)),
            },
            // No pattern: source value substitutes ${0}; higher indices are empty.
            None => vec![Some(source_value)],
        };

        Some(render(ext.template.as_deref(), &groups))
    }
}

/// Renders a template with `${N}` substitution. When `template` is `None`,
/// returns capture group 0 or empty if unavailable.
fn render(template: Option<&str>, groups: &[Option<String>]) -> String {
    let Some(t) = template else {
        return group_value(groups, 0).to_string();
    };
    expand_template(t, groups)
}

/// Returns the value of capture group `n`, or `""` for both
/// out-of-bounds indices and in-bounds-but-non-participating groups.
fn group_value(groups: &[Option<String>], n: usize) -> &str {
    groups.get(n).and_then(|g| g.as_deref()).unwrap_or("")
}

/// Expands `${N}` tokens in `template`. Out-of-range groups become empty.
/// Malformed tokens (`${abc}`, unclosed `${`) are emitted literally.
fn expand_template(template: &str, groups: &[Option<String>]) -> String {
    let mut out = String::with_capacity(template.len());
    let bytes = template.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$'
            && i + 1 < bytes.len()
            && bytes[i + 1] == b'{'
            && let Some(end_off) = template[i + 2..].find('}')
            && let Ok(n) = template[i + 2..i + 2 + end_off].parse::<usize>()
        {
            out.push_str(group_value(groups, n));
            i = i + 2 + end_off + 1;
            continue;
        }
        // Emit one full UTF-8 character starting at `i`.
        let ch = template[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::SimpleEpisodeData;

    fn ep(title: &str) -> SimpleEpisodeData {
        SimpleEpisodeData {
            id: 1,
            title: title.into(),
            description: None,
            season_number: None,
            episode_number: None,
            published_at: None,
            image_url: None,
        }
    }

    #[test]
    fn template_combines_multiple_capture_groups() {
        let ext = TitleExtractor {
            source: "title".into(),
            pattern: Some(r"【[^】]+(\d+)】\s*(.+?)#\d+$".into()),
            template: Some("${1}. ${2}".into()),
            fallback: None,
            fallback_value: None,
        };
        let episode = ep(
            "【アダム・スミス9】社会の秩序をつくるのは「優しさ」か「正義感」か。スミスが出した答えとは？#150",
        );
        assert_eq!(
            ext.extract(&episode).as_deref(),
            Some("9. 社会の秩序をつくるのは「優しさ」か「正義感」か。スミスが出した答えとは？"),
        );
    }

    #[test]
    fn out_of_range_capture_renders_empty() {
        let ext = TitleExtractor {
            source: "title".into(),
            pattern: Some(r"^(\w+) (\w+)$".into()),
            template: Some("${1}/${5}/${2}".into()),
            fallback: None,
            fallback_value: None,
        };
        let episode = ep("foo bar");
        assert_eq!(ext.extract(&episode).as_deref(), Some("foo//bar"));
    }

    #[test]
    fn template_zero_returns_full_match() {
        let ext = TitleExtractor {
            source: "title".into(),
            pattern: Some(r"(\d+)".into()),
            template: Some("[${0}]".into()),
            fallback: None,
            fallback_value: None,
        };
        let episode = ep("Episode 42");
        assert_eq!(ext.extract(&episode).as_deref(), Some("[42]"));
    }

    #[test]
    fn omitted_pattern_uses_source_for_zero() {
        let ext = TitleExtractor {
            source: "title".into(),
            pattern: None,
            template: Some("Title: ${0} / ${1}".into()),
            fallback: None,
            fallback_value: None,
        };
        let episode = ep("Hello");
        assert_eq!(ext.extract(&episode).as_deref(), Some("Title: Hello / "));
    }

    #[test]
    fn omitted_template_with_pattern_returns_full_match() {
        let ext = TitleExtractor {
            source: "title".into(),
            pattern: Some(r"\d+".into()),
            template: None,
            fallback: None,
            fallback_value: None,
        };
        let episode = ep("Episode 99");
        assert_eq!(ext.extract(&episode).as_deref(), Some("99"));
    }

    #[test]
    fn literal_dollar_outside_braces_is_preserved() {
        let ext = TitleExtractor {
            source: "title".into(),
            pattern: Some(r"^(\w+)$".into()),
            template: Some("${1} - $5 cost".into()),
            fallback: None,
            fallback_value: None,
        };
        let episode = ep("Promo");
        assert_eq!(ext.extract(&episode).as_deref(), Some("Promo - $5 cost"),);
    }

    #[test]
    fn malformed_dollar_brace_emitted_literally() {
        let ext = TitleExtractor {
            source: "title".into(),
            pattern: Some(r"^(\w+)$".into()),
            template: Some("${1} ${abc} ${".into()),
            fallback: None,
            fallback_value: None,
        };
        let episode = ep("ok");
        assert_eq!(ext.extract(&episode).as_deref(), Some("ok ${abc} ${"),);
    }

    #[test]
    fn fallback_chain_uses_new_template_semantics_per_link() {
        let ext = TitleExtractor {
            source: "title".into(),
            pattern: Some(r"^primary-(\w+)-(\w+)$".into()),
            template: Some("P:${1}+${2}".into()),
            fallback: Some(Box::new(TitleExtractor {
                source: "title".into(),
                pattern: Some(r"^backup-(\w+)$".into()),
                template: Some("F:${1}".into()),
                fallback: None,
                fallback_value: None,
            })),
            fallback_value: None,
        };
        let primary = ep("primary-aa-bb");
        let backup = ep("backup-cc");
        assert_eq!(ext.extract(&primary).as_deref(), Some("P:aa+bb"));
        assert_eq!(ext.extract(&backup).as_deref(), Some("F:cc"));
    }

    #[test]
    fn non_participating_optional_group_renders_empty() {
        let ext = TitleExtractor {
            source: "title".into(),
            pattern: Some(r"^(?:aaa-(\w+))|(?:bbb-(\w+))$".into()),
            template: Some("a=${1} b=${2}".into()),
            fallback: None,
            fallback_value: None,
        };
        assert_eq!(ext.extract(&ep("aaa-foo")).as_deref(), Some("a=foo b="));
        assert_eq!(ext.extract(&ep("bbb-bar")).as_deref(), Some("a= b=bar"));
    }

    #[test]
    fn no_pattern_no_template_returns_source_value() {
        let ext = TitleExtractor {
            source: "title".into(),
            pattern: None,
            template: None,
            fallback: None,
            fallback_value: None,
        };
        assert_eq!(
            ext.extract(&ep("Just the title")).as_deref(),
            Some("Just the title")
        );
    }

    #[test]
    fn fallback_value_triggers_for_missing_season_number() {
        let ext = TitleExtractor {
            source: "seasonNumber".into(),
            pattern: None,
            template: Some("Season ${0}".into()),
            fallback: None,
            fallback_value: Some("Extras".into()),
        };
        assert_eq!(ext.extract(&ep("any")).as_deref(), Some("Extras"));

        let mut s1 = ep("any");
        s1.season_number = Some(1);
        assert_eq!(ext.extract(&s1).as_deref(), Some("Season 1"));
    }

    #[test]
    fn fallback_value_triggers_for_missing_episode_number() {
        let ext = TitleExtractor {
            source: "episodeNumber".into(),
            pattern: None,
            template: Some("Episode ${0}".into()),
            fallback: None,
            fallback_value: Some("Bonus".into()),
        };
        assert_eq!(ext.extract(&ep("any")).as_deref(), Some("Bonus"));

        let mut e1 = ep("any");
        e1.episode_number = Some(7);
        assert_eq!(ext.extract(&e1).as_deref(), Some("Episode 7"));
    }

    #[test]
    fn fallback_value_does_not_short_circuit_title_source() {
        // Episodes lacking a season number should still extract a title.
        // Regression: previously, fallback_value short-circuited regardless of source.
        let ext = TitleExtractor {
            source: "title".into(),
            pattern: Some(r"^(.+)$".into()),
            template: Some("${1}".into()),
            fallback: None,
            fallback_value: Some("Should never appear".into()),
        };
        let episode = ep("Real title");
        assert_eq!(episode.season_number, None);
        assert_eq!(ext.extract(&episode).as_deref(), Some("Real title"));
    }
}
