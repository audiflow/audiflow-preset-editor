# TitleExtractor Multi-capture Template Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace `TitleExtractor.group` (single capture group selector) with `${N}` template-reference syntax so users can compose multiple regex captures into one display string.

**Architecture:** Drop the `group: u32` field from `TitleExtractor`. The `template` string gains placeholders `${0}` (full match), `${1}`, `${2}`, ... mapped to regex capture groups. Out-of-range references render as empty strings. When `pattern` is omitted, the source value substitutes `${0}` and higher indices substitute empty. When `template` is omitted, the renderer behaves as if it were `${0}`. Schema, Rust models, React form, Zod schema, locales, and docs all updated. No data migration shipped — users edit existing JSON by hand. All changes ride on v6 (already bumped).

**Tech Stack:** Rust (`sp_core` with `fancy-regex` + `serde`), React 19 + TypeScript + Zod 4 (`sp_react`), JSON Schema.

**Spec:** `docs/superpowers/specs/2026-05-03-title-extractor-multi-capture-design.md`

---

## File Map

**Modified:**
- `crates/sp_core/assets/playlist-definition.schema.json` — schema definition of `TitleExtractor`
- `crates/sp_core/src/models/title_extractor.rs` — model, render logic, unit tests
- `crates/sp_core/src/models/playlist_definition.rs` — JSON fixtures inside doc tests
- `crates/sp_core/tests/model_tests.rs` — JSON round-trip tests for `TitleExtractor`
- `crates/sp_core/tests/resolver_tests.rs` — resolver integration tests using `TitleExtractor`
- `crates/sp_core/tests/service_tests.rs` — service-level integration tests
- `packages/sp_react/src/schemas/config-schema.ts` — Zod schema and TS type
- `packages/sp_react/src/schemas/__tests__/config-schema.test.ts` — Zod schema tests
- `packages/sp_react/src/lib/__tests__/sanitize-config.test.ts` — config sanitizer tests
- `packages/sp_react/src/components/editor/title-extractor-form.tsx` — form UI
- `packages/sp_react/src/components/editor/__tests__/title-extractor-utils.test.ts` — form util tests
- `packages/sp_react/src/locales/{ja,en}/editor.json` — labels
- `packages/sp_react/src/locales/{ja,en}/hints.json` — help text
- `docs/schema-reference.md` — schema reference doc

---

## Task 1: Update JSON Schema

**Files:**
- Modify: `crates/sp_core/assets/playlist-definition.schema.json` (lines ~303–344)

- [ ] **Step 1: Edit the `TitleExtractor` definition**

In `playlist-definition.schema.json`, replace the entire `TitleExtractor` `$defs` block with:

```json
"TitleExtractor": {
  "type": "object",
  "description": "Generates a display name from episode data. Reads a value, optionally matches a pattern, and formats the result. Steps: (1) read the source field; (2) if pattern is set, find a match; (3) render template with ${0} (full match), ${1}, ${2}, ... for capture groups, or use the full match when template is omitted; (4) if any step fails, try the fallback. Supports chaining multiple fallback steps for complex podcast formats.",
  "required": [
    "source"
  ],
  "additionalProperties": false,
  "properties": {
    "source": {
      "type": "string",
      "enum": [
        "title",
        "description",
        "seasonNumber",
        "episodeNumber"
      ],
      "description": "Which part of the episode to read: 'title' (episode title), 'description' (episode description), 'seasonNumber' (season number from feed or numbering rule), or 'episodeNumber' (episode number from feed or numbering rule)."
    },
    "pattern": {
      "type": "string",
      "description": "A text pattern to find in the source value. Use parentheses () to mark parts you want to extract; refer to them in 'template' as ${1}, ${2}, ... When omitted, the raw source value is used directly (referenced as ${0} from template)."
    },
    "template": {
      "type": "string",
      "description": "How to format the matched text. Use ${0} for the entire match, ${1}, ${2}, ... for capture groups in parentheses. Example: '${1}. ${2}' joins the first and second capture groups with a period. Out-of-range references render as empty. When omitted, behaves as if '${0}'."
    },
    "fallback": {
      "$ref": "#/$defs/TitleExtractor",
      "description": "A fallback step tried when this one fails to find a match. Can be chained for multi-step fallbacks. Not tried when fallbackValue already returned a result."
    },
    "fallbackValue": {
      "type": "string",
      "description": "A default name used when the source is 'seasonNumber' or 'episodeNumber' and the value is missing or zero. Checked before pattern matching. Has no effect for 'title' or 'description' sources. Useful for labeling specials (e.g., 'Extras' for season 0)."
    }
  }
}
```

Key changes vs. current schema:
- The `group` property is removed entirely.
- `template`'s description switches from `{value}` semantics to `${N}` semantics.
- `pattern`'s description references `${1}`, `${2}` and notes the omitted-pattern + `${0}` behavior.
- `description` of the wrapping object updates to describe the new flow.

- [ ] **Step 2: Verify the file is valid JSON**

Run: `python3 -m json.tool crates/sp_core/assets/playlist-definition.schema.json > /dev/null`
Expected: no output, exit code 0.

- [ ] **Step 3: Commit**

```bash
git add crates/sp_core/assets/playlist-definition.schema.json
git commit -m "feat(schema): replace TitleExtractor.group with \${N} template syntax"
```

---

## Task 2: Add failing render tests for `${N}`

**Files:**
- Modify: `crates/sp_core/src/models/title_extractor.rs` — add a `#[cfg(test)] mod tests` block at end of file

- [ ] **Step 1: Add the failing tests**

Append to the end of `crates/sp_core/src/models/title_extractor.rs`:

```rust
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
        assert_eq!(
            ext.extract(&episode).as_deref(),
            Some("Promo - $5 cost"),
        );
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
        assert_eq!(
            ext.extract(&episode).as_deref(),
            Some("ok ${abc} ${"),
        );
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
}
```

- [ ] **Step 2: Run the tests and confirm they fail to compile**

Run: `cargo test -p sp_core --lib title_extractor::tests 2>&1 | head -30`
Expected: compilation error (`group` field is initialized in some places but not others, or — depending on current state — tests using `template: "${1}"` etc. parse fine but produce wrong output). Either way, this is the RED state. Do NOT proceed until this fails.

If the tests compile but fail at runtime, that's also RED — capture the failure messages.

---

## Task 3: Implement new `${N}` render and remove `group`

**Files:**
- Modify: `crates/sp_core/src/models/title_extractor.rs`

- [ ] **Step 1: Replace the entire file**

Overwrite `crates/sp_core/src/models/title_extractor.rs` with:

```rust
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

    /// Fallback string value for null/zero seasonNumber episodes.
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

        // Early return for null/zero seasonNumber when fallback_value is set.
        let season_num = episode.season_number();
        if ext.fallback_value.is_some()
            && (season_num.is_none() || season_num.is_some_and(|n| n < 1))
        {
            return ext.fallback_value.clone();
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
```

Notes for the engineer:
- The `is_zero` helper from before is no longer needed (deleted with `group`).
- Captures are eagerly copied into a `Vec<Option<String>>` so the renderer is independent of `fancy_regex`'s lifetimes. The allocation is per-extract; for high-volume use the `compile()` path can be optimized later.
- The byte-level scan is safe because `$`, `{`, `}`, and digits are all ASCII; non-ASCII characters are pushed via `chars().next()` so multi-byte UTF-8 (Japanese titles) round-trips correctly.
- The chained `if … && let …` uses Rust 2024 let-chains, which this workspace already uses (edition 2024).

- [ ] **Step 2: Run the new render tests**

Run: `cargo test -p sp_core --lib title_extractor::tests`
Expected: all 8 tests in the new `tests` module pass.

If anything fails, fix the renderer (do not weaken tests).

- [ ] **Step 3: Commit (incremental, model + tests only)**

```bash
git add crates/sp_core/src/models/title_extractor.rs
git commit -m "feat(sp_core): render TitleExtractor templates with \${N} captures"
```

The full workspace will not compile yet — the next task fixes downstream callers and fixtures.

---

## Task 4: Fix downstream Rust call sites and fixtures

**Files:**
- Modify: `crates/sp_core/src/models/playlist_definition.rs` (lines 240, 247, 314, 321 — JSON inside doc/test fixtures)
- Modify: `crates/sp_core/tests/model_tests.rs` (lines 161, 188, 752–790)
- Modify: `crates/sp_core/tests/resolver_tests.rs` (lines 220–227, 726–, 922–)
- Modify: `crates/sp_core/tests/service_tests.rs` (lines 1255–1280)

- [ ] **Step 1: Update `playlist_definition.rs` test fixtures**

In `crates/sp_core/src/models/playlist_definition.rs`, find each occurrence of:

```json
"group": 1
```

inside JSON-literal blocks (lines ~240, 247, 314, 321) and replace with:

```json
"template": "${1}"
```

Adjust trailing commas in the JSON blocks if needed so the result remains valid JSON.

- [ ] **Step 2: Update `model_tests.rs` round-trip tests**

In `crates/sp_core/tests/model_tests.rs`, replace the two `TitleExtractor` round-trip tests (the bodies starting around lines 752 and 775) with:

```rust
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
    assert!(serialized.get("group").is_none());
    assert!(serialized.get("pattern").is_none());
    assert!(serialized.get("template").is_none());
    assert!(serialized.get("fallback").is_none());
    assert!(serialized.get("fallbackValue").is_none());
}
```

Then find the two earlier hits at lines ~161 and ~188 (other JSON literals containing `"group": 1`) and rewrite them the same way: drop `"group": N,` and prepend the equivalent template:

- If the existing test asserts the rendered output explicitly, adjust the expected string to match `${N}` rendering (it will be the same since `template` was previously absent and `group: 1` rendered group 1 — the equivalent template is `"${1}"`).
- If `"template"` already has a `{value}` placeholder in the same fixture, replace `{value}` with `${N}` where `N` was the old `group`.

- [ ] **Step 3: Update `resolver_tests.rs`**

In `crates/sp_core/tests/resolver_tests.rs`, find the three `TitleExtractor { ... group: 1, ... }` struct literals (around lines 220, 726, 922). For each, replace:

```rust
pattern: Some(r"...".to_string()),
group: 1,
template: None,
```

with:

```rust
pattern: Some(r"...".to_string()),
template: Some("${1}".to_string()),
```

Keep all other fields (`source`, `fallback`, `fallback_value`) unchanged.

- [ ] **Step 4: Update `service_tests.rs`**

In `crates/sp_core/tests/service_tests.rs` around lines 1255–1280 there are two `TitleExtractor` struct literals using `group: 0` and `group: 1`. Apply:
- `group: 0` + `template: None` → `template: None` (omitted-template behaviour is identical to the old `group: 0` default).
- `group: 1` + `template: None` → `template: Some("${1}".to_string())`.
- If the surrounding fixture already had `template: Some("Season {value}".to_string())` (search the file), rewrite `{value}` → `${0}` if `group: 0` was paired, or `${N}` matching the original group.

(The grep at planning time showed only the bare `group: 0,` and `group: 1,` lines — but verify this with `grep -n "group:" crates/sp_core/tests/service_tests.rs` while editing in case fixtures grow.)

- [ ] **Step 5: Build and run all Rust tests**

Run:
```bash
cargo test -p sp_core
```
Expected: all tests pass (no compile errors, no failures).

If a fixture you missed still references `group:`, the compiler will tell you precisely where.

- [ ] **Step 6: Run clippy**

Run: `cargo clippy --workspace --all-targets -- -D warnings`
Expected: zero warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/sp_core
git commit -m "test(sp_core): migrate TitleExtractor fixtures to \${N} templates"
```

---

## Task 5: Update React Zod schema

**Files:**
- Modify: `packages/sp_react/src/schemas/config-schema.ts` (lines ~52–71)
- Modify: `packages/sp_react/src/schemas/__tests__/config-schema.test.ts` (lines 61, 68 area)

- [ ] **Step 1: Edit the Zod schema**

In `packages/sp_react/src/schemas/config-schema.ts`, replace the `TitleExtractorInput` type and `titleExtractorSchema` (currently lines ~52–71) with:

```typescript
// Recursive type for title extractor with fallback chain
export type TitleExtractorInput = {
  source: string;
  pattern?: string | null;
  template?: string | null;
  fallback?: TitleExtractorInput | null;
  fallbackValue?: string | null;
};

export const titleExtractorSchema: z.ZodType<TitleExtractorInput> = z.lazy(
  () =>
    z.object({
      source: z.string(),
      pattern: z.string().nullish(),
      template: z.string().nullish(),
      fallback: titleExtractorSchema.nullish(),
      fallbackValue: z.string().nullish(),
    }),
);
```

The only structural change is removing the `group` field and its `transform`.

- [ ] **Step 2: Update `config-schema.test.ts`**

Open `packages/sp_react/src/schemas/__tests__/config-schema.test.ts`. Around lines 60–70 there are two fixture objects with `group: 1`. Remove the `group: 1,` line from each and ensure the surrounding test still asserts a valid parse. If a test asserts on `result.…titleExtractor.group`, replace that assertion with one on `result.…titleExtractor.template` (e.g., expect `${1}`).

If a fixture was relying on `group` to make the parse meaningful, add `template: '${1}',` so the intent is preserved.

- [ ] **Step 3: Update `sanitize-config.test.ts`**

In `packages/sp_react/src/lib/__tests__/sanitize-config.test.ts` (lines 175 and 193), change:

```typescript
titleExtractor: { source: 'title', pattern: '(.+)', group: 1 },
```

to:

```typescript
titleExtractor: { source: 'title', pattern: '(.+)', template: '${1}' },
```

(Both occurrences.)

- [ ] **Step 4: Run schema and sanitizer tests**

Run:
```bash
cd packages/sp_react && pnpm test -- --run src/schemas src/lib
```
Expected: all targeted tests pass.

- [ ] **Step 5: Commit**

```bash
git add packages/sp_react/src/schemas packages/sp_react/src/lib/__tests__
git commit -m "feat(sp_react): drop TitleExtractor.group from Zod schema"
```

---

## Task 6: Update React form UI

**Files:**
- Modify: `packages/sp_react/src/components/editor/title-extractor-form.tsx`
- Modify: `packages/sp_react/src/components/editor/__tests__/title-extractor-utils.test.ts`

- [ ] **Step 1: Update the form component**

In `packages/sp_react/src/components/editor/title-extractor-form.tsx`:

**(a)** Around line 92, change:
```typescript
const newStep: TitleExtractor = { source: 'title', group: 0 };
```
to:
```typescript
const newStep: TitleExtractor = { source: 'title' };
```

**(b)** Around lines 148–153 inside the "no extractor → add" button handler, change:
```typescript
setValue(fieldPath as any, {
  source: 'title',
  group: 0,
}, { shouldDirty: true })
```
to:
```typescript
setValue(fieldPath as any, {
  source: 'title',
}, { shouldDirty: true })
```

**(c)** Remove the entire group input block (lines ~268–283):
```typescript
<div className="space-y-1.5">
  <HintLabel
    htmlFor={`${idPrefix}-${stepIndex}-group`}
    hint="titleExtractorGroup"
  >
    {t('titleExtractorGroup')}
  </HintLabel>
  <Input
    id={`${idPrefix}-${stepIndex}-group`}
    type="number" className="w-24"
    value={step.group ?? 0}
    onChange={(e) =>
      onUpdate({ group: parseInt(e.target.value, 10) || 0 })
    }
  />
</div>
```

Delete this whole `<div>...</div>`. Inspect the parent flex/grid container that previously held the source `<select>` next to this input. If the source was inside a two-column row that now collapses to one cell, change the row's grid/flex classes so the source select takes full width (or remove the `grid-cols-2`-style wrapper). The exact class change depends on the surrounding markup — make the source label/input occupy the row cleanly.

**(d)** Around line 316, update the template input placeholder:
```typescript
placeholder="{value}"
```
to:
```typescript
placeholder="${1}. ${2}"
```

(The new placeholder hints at multi-capture composition.)

- [ ] **Step 2: Update form util tests**

In `packages/sp_react/src/components/editor/__tests__/title-extractor-utils.test.ts` (lines 7–8, 20–21):

Replace:
```typescript
{ source: 'title', pattern: '^(.+)', group: 1, ... }
{ source: 'seasonNumber', group: 0, template: 'Season {value}' }
```

with:
```typescript
{ source: 'title', pattern: '^(.+)', template: '${1}', ... }
{ source: 'seasonNumber', template: 'Season ${0}' }
```

(Apply to both occurrences in the file.)

- [ ] **Step 3: Run React tests**

Run:
```bash
cd packages/sp_react && pnpm test -- --run src/components/editor
```
Expected: all editor tests pass.

- [ ] **Step 4: Commit**

```bash
git add packages/sp_react/src/components/editor
git commit -m "feat(sp_react): drop group input from TitleExtractor form"
```

---

## Task 7: Update locales

**Files:**
- Modify: `packages/sp_react/src/locales/ja/editor.json`
- Modify: `packages/sp_react/src/locales/en/editor.json`
- Modify: `packages/sp_react/src/locales/ja/hints.json`
- Modify: `packages/sp_react/src/locales/en/hints.json`

- [ ] **Step 1: Edit `editor.json` (ja and en)**

In both `packages/sp_react/src/locales/ja/editor.json` and `.../en/editor.json`, **delete the entire line** containing `"titleExtractorGroup":` (line ~117 in each). Make sure the JSON remains valid (mind trailing commas — adjust the previous line if needed).

- [ ] **Step 2: Edit `hints.json` (ja)**

In `packages/sp_react/src/locales/ja/hints.json`:
- Delete the entire `"titleExtractorGroup": "..."` line (~line 33).
- Replace the `"titleExtractorTemplate"` entry (~line 34) with:

```json
"titleExtractorTemplate": "抽出した値の表示フォーマット。${0} はマッチ全体、${1}, ${2}, … は丸括弧で囲んだキャプチャ部分です。例: '${1}. ${2}' で2つのキャプチャを結合。省略すると ${0} が使われます。",
```

- Replace `"titleExtractorPattern"` (~line 32) with:

```json
"titleExtractorPattern": "読み取り元から見つけるテキストパターン。丸括弧 () で抽出したい部分をマークし、表示フォーマットから ${1}, ${2}, … で参照します。",
```

- [ ] **Step 3: Edit `hints.json` (en)**

In `packages/sp_react/src/locales/en/hints.json`:
- Delete the entire `"titleExtractorGroup"` line.
- Replace `"titleExtractorTemplate"` with:

```json
"titleExtractorTemplate": "How to format the extracted text. Use ${0} for the entire match, ${1}, ${2}, … for capture groups in parentheses. Example: '${1}. ${2}' joins two captures. Omit to use ${0}.",
```

- Replace `"titleExtractorPattern"` with:

```json
"titleExtractorPattern": "A text pattern to find in the source. Use parentheses () to mark parts; reference them from the template as ${1}, ${2}, …",
```

- [ ] **Step 4: Validate JSON files**

Run:
```bash
for f in packages/sp_react/src/locales/{ja,en}/{editor,hints}.json; do
  python3 -m json.tool "$f" > /dev/null && echo "$f OK"
done
```
Expected: every file prints `OK`.

- [ ] **Step 5: Run React tests + lint + typecheck**

Run:
```bash
cd packages/sp_react && pnpm test -- --run && npx oxlint && npx tsc -b --noEmit
```
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add packages/sp_react/src/locales
git commit -m "feat(sp_react): update TitleExtractor locale strings for \${N} syntax"
```

---

## Task 8: Update documentation

**Files:**
- Modify: `docs/schema-reference.md` (TitleExtractor section)
- Inspect: `docs/integration/editor-to-schema.md`, `docs/integration/preset-contract.md` — only edit if they show a `group:` example or use `{value}` for templates

- [ ] **Step 1: Find the TitleExtractor section**

Run: `grep -n -i "titleextractor\|title extractor\|\"group\"\|{value}" docs/schema-reference.md`

Open the file at the matching lines.

- [ ] **Step 2: Rewrite the TitleExtractor reference**

In `docs/schema-reference.md`, replace any description of the `group` field and any `{value}` placeholder reference with the `${N}` semantics. The replacement section should cover:

- Field list: `source`, `pattern`, `template`, `fallback`, `fallbackValue` (no `group`).
- Template syntax: `${0}` = full match, `${1}`, `${2}`, … = capture groups, out-of-range → empty.
- Behavior when `pattern` is omitted: source value substitutes `${0}`, `${N>=1}` is empty.
- Behavior when `template` is omitted: equivalent to `${0}`.
- An example using the user's case:

```json
{
  "source": "title",
  "pattern": "【[^】]+(\\d+)】\\s*(.+?)#\\d+$",
  "template": "${1}. ${2}"
}
```

with a note that this turns `【アダム・スミス9】社会の秩序をつくるのは「優しさ」か「正義感」か。スミスが出した答えとは？#150` into `9. 社会の秩序をつくるのは「優しさ」か「正義感」か。スミスが出した答えとは？`.

- [ ] **Step 3: Scan integration docs**

Run: `grep -n '"group"\|{value}' docs/integration/*.md`

For each hit:
- If it's part of a `TitleExtractor` example, rewrite it the same way (drop `"group"`, switch `{value}` → `${N}`).
- If it documents some unrelated `group` (e.g., capture groups in regex generally), leave it alone.

If neither file has hits, no edits needed.

- [ ] **Step 4: Commit**

```bash
git add docs
git commit -m "docs: document TitleExtractor \${N} template syntax"
```

---

## Task 9: Final cross-stack validation

- [ ] **Step 1: Rust full check**

Run:
```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```
Expected: all tests pass, zero clippy warnings.

- [ ] **Step 2: React full check**

Run:
```bash
cd packages/sp_react && pnpm test -- --run && npx oxlint && npx tsc -b --noEmit
```
Expected: all tests pass, no lint errors, no type errors.

- [ ] **Step 3: Smoke test the editor**

Run: `cargo run -p sp_cli -- serve --data-dir <path/to/local/data/repo> --port 8080`

In a browser at `http://localhost:8080`, open an existing playlist (or create one). Open the "Episode name rule" / "Group name rule" editor:
- Confirm the "Use matched part #" / "使用するマッチ部分 #" input no longer appears.
- Confirm the "Format as" / "表示フォーマット" placeholder shows `${1}. ${2}`.
- Manually enter a `pattern` of `【[^】]+(\d+)】\s*(.+?)#\d+$` and a `template` of `${1}. ${2}`.
- Trigger preview against a feed whose titles match the user's example. Confirm rendered names are `9. ...`, `10. ...`, etc.

If the UI does not match expectations, revisit Tasks 6 & 7.

- [ ] **Step 4: No commit needed if validation only**

If smoke-test surfaced fixes, commit them under their relevant task scope.

---

## Done

At this point:
- The schema has no `group` field; it accepts `${N}` templates.
- The Rust renderer expands `${N}` with capture-group substitution.
- The React form, Zod schema, locales, and docs all reflect the new syntax.
- All existing fixtures have been migrated to `${N}` templates.
- v6 ships with this additional breaking change. Users with old `group` data must edit it by hand (no automatic migration tool).
