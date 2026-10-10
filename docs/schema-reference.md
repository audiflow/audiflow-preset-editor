# Schema Reference (v6)

Schema reference for audiflow smart playlist configuration files. Covers all three schema levels: pattern index, pattern meta, and playlist definition.

JSON Schema definitions are versioned at **v6**; the pattern-index file format is versioned independently via its own `schemaVersion` field (see [Pattern Index](#pattern-index)).

## Table of Contents

- [Three-Level Hierarchy](#three-level-hierarchy)
- [Pattern Index](#pattern-index)
- [Pattern Meta](#pattern-meta)
- [Playlist Definition](#playlist-definition)
  - [Required Fields](#required-fields)
  - [episodeFilters](#episodefilters)
  - [grouping](#grouping)
  - [selector](#selector)
  - [groupListing](#grouplisting)
  - [groupItem](#groupitem)
  - [episodeListing](#episodelisting)
  - [episodeItem](#episodeitem)
  - [Legacy v4 Fields](#legacy-v4-fields)
- [Grouping Methods](#grouping-methods)
- [TitleExtractor](#titleextractor)
- [NumberingExtractor](#numberingextractor)
- [StaticClassifier](#staticclassifier)
- [Sort Rules](#sort-rules)
- [Pattern ID Derivation](#pattern-id-derivation)

---

## Three-Level Hierarchy

Configuration data is organized in a three-level file hierarchy:

```
patterns/
  meta.json                          # Level 1: Pattern Index
  {presetId}/
    meta.json                        # Level 2: Pattern Meta
    playlists/
      {playlistId}.json              # Level 3: Playlist Definition
```

**Level 1 -- Pattern Index** (`presets/meta.json`): Root index listing all available patterns. The app fetches this first and uses `feedUrlHint` for fast pre-filtering.

**Level 2 -- Pattern Meta** (`{presetId}/meta.json`): Per-podcast metadata containing feed URLs for matching and an ordered list of playlist IDs.

**Level 3 -- Playlist Definition** (`{presetId}/playlists/{playlistId}.json`): Individual playlist configuration defining how episodes are filtered, grouped, and displayed.

---

## Pattern Index

**File:** `presets/meta.json`

**Schema:** `https://audiflow.app/schema/v6/pattern-index.json`

Root index of all patterns. The app fetches this file first and compares `dataVersion` to detect changes.

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `dataVersion` | integer | yes | Monotonically increasing. Bumped by CI when any pattern changes. |
| `schemaVersion` | integer | yes | Schema structure version. Bumped when JSON structure changes. |
| `patterns` | array | yes | Summary entries for all patterns. |

### PresetIndexEntry

Each element in the `patterns` array:

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Pattern identifier. See [Pattern ID Derivation](#pattern-id-derivation). |
| `dataVersion` | integer | yes | Mirrors `{id}/meta.json` dataVersion for stale-cache detection. |
| `displayName` | string | yes | Human-readable podcast name for admin/editor UIs. |
| `feedUrlHint` | string | yes | Substring of the feed URL for fast pre-filtering. |
| `playlistCount` | integer | yes | Number of playlist definitions in this pattern. |

### Example

```json
{
  "dataVersion": 42,
  "schemaVersion": 5,
  "patterns": [
    {
      "id": "a1b2c3d4e5f6",
      "dataVersion": 7,
      "displayName": "COTEN RADIO",
      "feedUrlHint": "anchor.fm/s/8c2088c",
      "playlistCount": 3
    }
  ]
}
```

---

## Pattern Meta

**File:** `{presetId}/meta.json`

**Schema:** `https://audiflow.app/schema/v6/pattern-meta.json`

Per-podcast metadata. The app matches incoming feeds against `podcastGuid` (preferred) or `feedUrls` to find the right config.

### Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `dataVersion` | integer | yes | -- | Monotonically increasing. Bumped when any file in this pattern changes. |
| `id` | string | yes | -- | Must match the parent directory name. |
| `podcastGuid` | string | no | -- | Podcast's RSS GUID. Takes priority over feed URL matching. |
| `feedUrls` | string[] | yes | -- | One or more RSS feed URLs. Supports host-migration scenarios. |
| `yearGroupedEpisodes` | boolean | no | `false` | Show year headers in the main episode list (independent of playlist grouping). |
| `showEpisodeThumbnail` | boolean | no | `true` | Show thumbnails on each row of the main podcast episode list. The page header artwork is unaffected. |
| `playlists` | string[] | yes | -- | Ordered list of playlist definition IDs. Determines display order. |

### Example

```json
{
  "dataVersion": 7,
  "id": "a1b2c3d4e5f6",
  "podcastGuid": "abc123-def456",
  "feedUrls": [
    "https://anchor.fm/s/8c2088c/podcast/rss"
  ],
  "yearGroupedEpisodes": true,
  "showEpisodeThumbnail": false,
  "playlists": ["regular", "extras", "shorts"]
}
```

---

## Playlist Definition

**File:** `{presetId}/playlists/{playlistId}.json`

**Schema:** `https://audiflow.app/schema/v6/playlist-definition.json`

Defines how a playlist filters, groups, and displays episodes. The field layout follows the data processing pipeline:

```
episodeFilters   -->  filter episodes before processing
     |
  grouping       -->  organize filtered episodes into groups
     |
  selector       -->  map groups to dropdown entries
     |
  display        -->  control visual presentation
     |--- groupListing    how the group list is arranged
     |--- groupItem       defaults for each group card
     |--- episodeListing  how episodes are arranged within a group
     |--- episodeItem     defaults for each episode row
```

### Required Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Unique within the pattern. Must match filename (without `.json`). |
| `displayName` | string | yes | Name shown to users in the app. |
| `priority` | integer | no | Default `0`. Lower numbers claim episodes first. Playlists with filters always run before filter-less ones. |

```json
{
  "id": "regular",
  "displayName": "Regular Series",
  "priority": 0
}
```

---

### episodeFilters

Optional. Pre-processing step that includes/excludes episodes before grouping.

| Field | Type | Description |
|-------|------|-------------|
| `require` | EpisodeFilterEntry[] | Include rules (OR). An episode is included when it matches ANY rule. Within each rule, all fields must match (AND). |
| `exclude` | EpisodeFilterEntry[] | Exclude rules (OR). An episode is excluded if it matches ANY rule. Within each rule, all fields must match (AND). |

**EpisodeFilterEntry**: At least one field required. When multiple fields are specified, all must match (AND).

| Field | Type | Description |
|-------|------|-------------|
| `title` | string | Case-insensitive regex matched against episode title. |
| `description` | string | Case-insensitive regex matched against episode description. |

```json
{
  "episodeFilters": {
    "require": [{ "title": "\\[\\d+-\\d+\\]" }],
    "exclude": [
      { "title": "COTEN RADIO\\s*short" },
      { "title": "preview" }
    ]
  }
}
```

---

### grouping

Required. Defines how episodes are organized into groups.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `by` | enum | yes | Grouping method. See [Grouping Methods](#grouping-methods). |
| `numberingExtractor` | object | no | Parses season/episode numbers from titles. See [NumberingExtractor](#numberingextractor). |
| `staticClassifiers` | array | conditional | Group definitions. Required when `by` is `titleClassifier`. See [StaticClassifier](#staticclassifier). |

```json
{
  "grouping": {
    "by": "seasonNumber",
    "numberingExtractor": {
      "source": "title",
      "pattern": "\\[(\\d+)-(\\d+)\\]",
      "seasonGroup": 1,
      "episodeGroup": 2
    }
  }
}
```

**Legacy note:** In v4, `resolverType` was a top-level field and `numberingExtractor` lived at root level. Both are accepted as aliases during migration.

---

### selector

Optional. Controls how groups map to dropdown entries in the app UI. When absent, all groups appear as cards inside a single entry.

| Field | Type | Description |
|-------|------|-------------|
| `partitionBy` | enum | `"seasonNumber"` or `"year"`. |
| `titleExtractor` | object | Generates display names for partitioned entries. Used with `seasonNumber` or `year`. See [TitleExtractor](#titleextractor). |

**partitionBy values:**

| Value | Behavior |
|-------|----------|
| `"seasonNumber"` | Groups are organized under season-number entries. |
| `"year"` | Groups are organized under year entries. |
| _(absent)_ | Single entry containing all groups as cards. |

```json
{
  "selector": {
    "partitionBy": "seasonNumber",
    "titleExtractor": {
      "source": "seasonNumber",
      "template": "Season ${0}"
    }
  }
}
```

**Legacy note:** Replaces v4 `presentation`. `"combined"` maps to no selector.

---

### groupListing

Optional. Controls how the group list is arranged.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `sort` | SortRule | natural order | Sort rule for groups. See [Sort Rules](#sort-rules). |
| `userSortable` | boolean | `true` | Allow users to flip between ascending/descending. |
| `sectionBy` | object | -- | Controls section headers in the list. |

**sectionBy:**

| Field | Type | Description |
|-------|------|-------------|
| `year` | object | `{ "pin": boolean }` -- when `pin` is true, each group is pinned to the year of its earliest episode. |
| `seasonNumber` | boolean | When true, groups are sectioned by season number. |

```json
{
  "groupListing": {
    "sort": { "field": "playlistNumber", "order": "ascending" },
    "userSortable": true,
    "sectionBy": {
      "year": { "pin": true }
    }
  }
}
```

**Legacy note:** Replaces v4 `groupList`. The v4 `groupList.yearBinding` is replaced by `groupListing.sectionBy`. The v4 `groupList.showDateRange` moved to `groupItem.showDateRange`.

---

### groupItem

Optional. Default display settings for individual group cards. Overridable per-classifier via `grouping.staticClassifiers[].groupItem`.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `showDateRange` | boolean | `false` | Show the date range (earliest to latest) on group cards. |
| `pinToYear` | boolean | `false` | Pin group to its earliest year's section. |
| `prependSeasonNumber` | boolean | `false` | Prefix group title with `S{n}` (e.g., "S13 Lincoln Arc"). |
| `showThumbnail` | boolean | `true` | Show a thumbnail on each group card in this playlist. |
| `titleExtractor` | object | -- | Generates display names for group titles. See [TitleExtractor](#titleextractor). |

```json
{
  "groupItem": {
    "showDateRange": true,
    "pinToYear": true,
    "prependSeasonNumber": false,
    "showThumbnail": false,
    "titleExtractor": {
      "source": "title",
      "pattern": "COTEN RADIO\\s*([^]+?)\\s*\\d+",
      "template": "${1}",
      "fallbackValue": "Other"
    }
  }
}
```

**Legacy note:** In v4, `titleExtractor` and `prependSeasonNumber` were top-level fields. `showDateRange` was under `groupList`.

---

### episodeListing

Optional. Controls how episodes are arranged within groups. Overridable per-classifier via `grouping.staticClassifiers[].episodeListing`.

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `sort` | EpisodeSortRule | publishedAt ascending | Sort order for episodes within groups. See [Sort Rules](#sort-rules). |
| `showYearHeaders` | boolean | `false` | Show year dividers in episode lists. |

```json
{
  "episodeListing": {
    "sort": { "field": "publishedAt", "order": "ascending" },
    "showYearHeaders": false
  }
}
```

**Legacy note:** Replaces v4 `episodeList`. The v4 `episodeList.titleExtractor` moved to `episodeItem.titleExtractor`.

---

### episodeItem

Optional. Default display settings for individual episode rows. Overridable per-classifier via `grouping.staticClassifiers[].episodeItem`.

| Field | Type | Description |
|-------|------|-------------|
| `titleExtractor` | object | Transforms episode display names. Strips redundant information conveyed by group context. See [TitleExtractor](#titleextractor). |
| `showThumbnail` | boolean | Show a thumbnail on each episode row inside a group. Default `true`. |

```json
{
  "episodeItem": {
    "titleExtractor": {
      "source": "title",
      "pattern": "#\\d+(?:-\\d+)?\\s+(.+?)\\s*\\[",
      "template": "${1}"
    },
    "showThumbnail": false
  }
}
```

**Legacy note:** In v4, this was `episodeList.titleExtractor`.

---

### Legacy v4 Fields

The following top-level fields are accepted as aliases during v4-to-v5 migration. Use v5 equivalents for new configurations.

| Legacy v4 Field | v5 Equivalent | Migration Notes |
|-----------------|---------------|-----------------|
| `resolverType` | `grouping.by` | Direct rename. |
| `groups` | `grouping.staticClassifiers` | Direct rename. |
| `numberingExtractor` | `grouping.numberingExtractor` | Moved into grouping block. |
| `presentation` | `selector` | `"combined"` = no selector. |
| `groupList` | `groupListing` + `groupItem` | Split: collection settings to `groupListing`, item settings to `groupItem`. |
| `groupList.yearBinding` | `groupListing.sectionBy` | Expanded to support multiple sectioning axes. |
| `groupList.showDateRange` | `groupItem.showDateRange` | Moved from collection to item level. |
| `titleExtractor` (top-level) | `groupItem.titleExtractor` | Moved to where it semantically belongs. |
| `prependSeasonNumber` | `groupItem.prependSeasonNumber` | Moved to where it semantically belongs. |
| `episodeList` | `episodeListing` + `episodeItem` | Split: collection settings to `episodeListing`, item settings to `episodeItem`. |
| `episodeList.titleExtractor` | `episodeItem.titleExtractor` | Moved from collection to item level. |
| `episodeExtractor` | `grouping.numberingExtractor` | Deprecated v3 alias for `numberingExtractor`. |

---

## Grouping Methods

The `grouping.by` field selects how episodes are organized into groups.

### seasonNumber

Groups episodes by season number from feed metadata or title parsing.

- Season numbers come from `grouping.numberingExtractor` (if configured) or RSS metadata (`itunes:season`).
- Each distinct season becomes a separate group.
- Episodes without a season number are left ungrouped.
- Best for podcasts that tag seasons or use consistent title patterns like `S01E01` or `[62-15]`.

```json
{
  "grouping": {
    "by": "seasonNumber",
    "numberingExtractor": {
      "source": "title",
      "pattern": "\\[(\\d+)-(\\d+)\\]",
      "seasonGroup": 1,
      "episodeGroup": 2,
      "fallbackToRss": true
    }
  }
}
```

### year

Groups episodes by publication year.

- Each distinct year becomes a separate group.
- Best for long-running podcasts where yearly grouping is natural.

```json
{
  "grouping": {
    "by": "year"
  }
}
```

### titleDiscovery

Finds recurring patterns in episode titles and groups by them, in the order they first appear in the feed.

- Uses `groupItem.titleExtractor` to derive group names; the template references capture groups via `${N}` (e.g., `${1}` for the first capture).
- Best for podcasts with titled story arcs or recurring guest series.

```json
{
  "grouping": {
    "by": "titleDiscovery"
  },
  "groupItem": {
    "titleExtractor": {
      "source": "title",
      "pattern": "[(.+?)\\s*]",
      "template": "${1}"
    }
  }
}
```

### titleClassifier

Groups episodes by matching titles against patterns defined in `grouping.staticClassifiers`.

- Classifiers are evaluated in order; first match wins.
- A classifier without a `pattern` acts as a catch-all for unmatched episodes.
- Group display names come from the classifier's `displayName` (not from `titleExtractor`).
- Requires `grouping.staticClassifiers`.

```json
{
  "grouping": {
    "by": "titleClassifier",
    "staticClassifiers": [
      {
        "id": "main-series",
        "displayName": "Main Series",
        "pattern": "\\[\\d+-\\d+\\]"
      },
      {
        "id": "extras",
        "displayName": "Extras",
        "pattern": null
      }
    ]
  }
}
```

### Deprecated Aliases

| Alias | Maps To |
|-------|---------|
| `rss` | `seasonNumber` |
| `category` | `titleClassifier` |
| `titleAppearanceOrder` | `titleDiscovery` |

---

## TitleExtractor

Shared type used by `groupItem.titleExtractor`, `episodeItem.titleExtractor`, and `selector.titleExtractor`.

Generates a display name from episode data using a pipeline: read source -> match pattern -> render template -> fallback on failure.

### Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `source` | enum | yes | -- | `"title"`, `"description"`, `"seasonNumber"`, or `"episodeNumber"`. |
| `pattern` | string | no | -- | Regex to match against the source value. Use capture groups `()` to extract parts; reference them in `template` as `${1}`, `${2}`, ... |
| `template` | string | no | -- | Format string. Use `${0}` for the entire match, `${1}`, `${2}`, ... for capture groups. Out-of-range references render as empty. When omitted, behaves as `${0}`. |
| `fallback` | TitleExtractor | no | -- | Tried when the current step fails to match. Supports chaining. |
| `fallbackValue` | string | no | -- | Default name when source is `seasonNumber` or `episodeNumber` and the value is missing or zero. Checked before pattern matching. No effect for `title` or `description` sources. |

### Processing Steps

1. Read the value from `source`.
2. If `fallbackValue` is set and source is `seasonNumber`/`episodeNumber` with a missing or zero value, return `fallbackValue`.
3. If `pattern` is set, match against the value. No match -> try `fallback`.
4. Render `template`: substitute each `${N}` with capture group N (or empty if out of range). When `template` is omitted, use `${0}` (the full match, or the source value when no `pattern`).
5. If any step fails, try the `fallback` extractor (recursive).

### Examples

**Simple template (season labels):**

```json
{
  "source": "seasonNumber",
  "template": "Season ${0}",
  "fallbackValue": "Specials"
}
```

**Regex extraction (strip prefix):**

```json
{
  "source": "title",
  "pattern": "#\\d+\\s+(.+?)\\s*$",
  "template": "${1}"
}
```

**Multi-capture composition (combine number and title):**

```json
{
  "source": "title",
  "pattern": "【[^】]+(\\d+)】\\s*(.+?)#\\d+$",
  "template": "${1}. ${2}"
}
```

For an input title like `【アダム・スミス9】社会の秩序をつくるのは「優しさ」か「正義感」か。スミスが出した答えとは？#150`, this renders as `9. 社会の秩序をつくるのは「優しさ」か「正義感」か。スミスが出した答えとは？`.

**Chained fallback:**

```json
{
  "source": "title",
  "pattern": "COTEN RADIO\\s*([^]+?)\\s*\\d+",
  "template": "${1}",
  "fallback": {
    "source": "title",
    "pattern": "COTEN RADIO\\s*(.+?)\\s*\\d+",
    "template": "${1}"
  },
  "fallbackValue": "Other"
}
```

---

## NumberingExtractor

Shared type used by `grouping.numberingExtractor` and per-classifier overrides in `grouping.staticClassifiers[].numberingExtractor`.

Parses season and episode numbers from episode text using a three-tier approach:

1. **Primary pattern** with capture groups for season and episode.
2. **Fallback pattern** for special episodes with a fixed season number.
3. **RSS feed data** as last resort (episode number only).

### Fields

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `source` | enum | yes | -- | `"title"` or `"description"`. |
| `pattern` | string | yes | -- | Regex with capture groups for season and episode numbers. |
| `seasonGroup` | integer or null | no | `1` | Capture group index for season number. Set to `null` for episode-only mode. |
| `episodeGroup` | integer | no | `2` | Capture group index for episode number. |
| `fallbackSeasonNumber` | integer | no | -- | Season number assigned when only the fallback pattern matches. |
| `fallbackEpisodePattern` | string | no | -- | Backup regex tried when the primary pattern does not match. |
| `fallbackEpisodeCaptureGroup` | integer | no | `1` | Capture group in the fallback pattern for the episode number. |
| `fallbackToRss` | boolean | no | `false` | Use RSS feed episode number when no pattern matches. Episode number only; season is not set. |

### Example

```json
{
  "source": "title",
  "pattern": "\\[(\\d+)-(\\d+)\\]",
  "seasonGroup": 1,
  "episodeGroup": 2,
  "fallbackSeasonNumber": 0,
  "fallbackEpisodePattern": "(\\d+)\\]",
  "fallbackEpisodeCaptureGroup": 1,
  "fallbackToRss": true
}
```

**Processing flow:**

1. Match `pattern` against the source text.
2. If matched, extract season from `seasonGroup` and episode from `episodeGroup`.
3. If not matched and `fallbackEpisodePattern` is set, try it. Assign `fallbackSeasonNumber` as the season.
4. If still no match and `fallbackToRss` is true, use RSS feed episode number.

---

## StaticClassifier

Defines a group within `grouping.staticClassifiers` (required for `titleClassifier` method). In v4, this type was called `GroupDef`.

Each classifier matches episodes by title pattern. Classifiers are evaluated in order; the first matching pattern claims the episode. A classifier without a `pattern` acts as a catch-all for unmatched episodes and should be placed last.

### Fields

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `id` | string | yes | Unique identifier within the playlist. |
| `displayName` | string | yes | Name shown to users. Always used as-is (not overridden by `titleExtractor`). |
| `pattern` | string | no | Case-insensitive regex matched against episode titles. Omit to create a catch-all. |
| `groupItem` | object | no | Per-classifier override for group card display. |
| `episodeListing` | object | no | Per-classifier override for episode arrangement. |
| `episodeItem` | object | no | Per-classifier override for episode row display. |
| `numberingExtractor` | object | no | Per-classifier override for number parsing. |

### Per-Classifier Overrides

Each classifier can override playlist-level defaults for display and behavior. Omitted fields inherit from the playlist-level settings.

**Overridable sections:**

| Section | Overridable Fields |
|---------|--------------------|
| `groupItem` | `showDateRange`, `showThumbnail`, `pinToYear` |
| `episodeListing` | `sort` |
| `episodeItem` | `titleExtractor`, `showThumbnail` |
| `numberingExtractor` | All fields (replaces the playlist-level extractor entirely) |

### Example

```json
{
  "grouping": {
    "by": "titleClassifier",
    "staticClassifiers": [
      {
        "id": "main",
        "displayName": "Main Series",
        "pattern": "\\[\\d+-\\d+\\]",
        "groupItem": { "showDateRange": true },
        "episodeListing": {
          "sort": { "field": "episodeNumber", "order": "ascending" }
        },
        "episodeItem": {
          "titleExtractor": {
            "source": "title",
            "pattern": "\\]\\s*(.+)",
            "template": "${1}"
          }
        }
      },
      {
        "id": "extras",
        "displayName": "Extras",
        "pattern": null,
        "groupItem": { "showDateRange": false },
        "episodeListing": {
          "sort": { "field": "publishedAt", "order": "descending" }
        }
      }
    ]
  }
}
```

### Legacy v4 Fields in GroupDef

The v4 `GroupDef` type contained `display` and `episodeList` sub-objects that map to v5 overrides:

| v4 GroupDef Field | v5 StaticClassifier Field |
|-------------------|---------------------------|
| `display.showDateRange` | `groupItem.showDateRange` |
| `display.yearBinding` | `groupItem.pinToYear` (for `pinToYear` binding) |
| `episodeList.sort` | `episodeListing.sort` |
| `episodeList.titleExtractor` | `episodeItem.titleExtractor` |
| `episodeList.showYearHeaders` | _(not overridable per-classifier in v5)_ |
| `episodeExtractor` | `numberingExtractor` (deprecated v3 alias) |

---

## Sort Rules

### Group Sort (SortRule)

Used in `groupListing.sort`.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `field` | enum | yes | What to sort by. |
| `order` | enum | yes | `"ascending"` or `"descending"`. |

**Sort fields for groups:**

| Field Value | Description |
|-------------|-------------|
| `playlistNumber` | Group's number: season number, year, first-appearance index, or group position. |
| `newestEpisodeDate` | Newest episode date in each group. |
| `alphabetical` | Alphabetical by group name. |

```json
{ "field": "playlistNumber", "order": "ascending" }
```

### Episode Sort (EpisodeSortRule)

Used in `episodeListing.sort` and per-classifier `episodeListing.sort`.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `field` | enum | yes | What to sort by. |
| `order` | enum | yes | `"ascending"` or `"descending"`. |

**Sort fields for episodes:**

| Field Value | Description |
|-------------|-------------|
| `publishedAt` | Episode's published date. |
| `episodeNumber` | Episode number from feed data or numbering extractor. |
| `title` | Alphabetical by episode title. |

```json
{ "field": "publishedAt", "order": "ascending" }
```

### Sort Order Values

| Value | Meaning |
|-------|---------|
| `ascending` | A-Z, oldest first, lowest number first. |
| `descending` | Z-A, newest first, highest number first. |

---

## Pattern ID Derivation

Pattern IDs are semi-deterministic, computed at creation time:

1. If `podcastGuid` is available: `md5(podcastGuid)[0:12]`
2. Otherwise: `md5(feedUrls[0])[0:12]`

The ID is a 12-character hex string derived from the MD5 hash. Once created, the ID is stable and used to construct all file paths:

- `{id}/meta.json` -- pattern metadata
- `{id}/playlists/{playlistId}.json` -- playlist definitions

The ID must match across:
- `presets/meta.json` entry `id`
- `{id}/meta.json` field `id`
- The directory name containing the pattern files
