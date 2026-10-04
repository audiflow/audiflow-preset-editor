---
paths: crates/**/*.rs, packages/**/*.{ts,tsx}, Cargo.toml, package.json
---

# Architecture

Rust workspace with three crates plus a React SPA.

## Package Overview

```
audiflow-preset-editor/
├── crates/
│   ├── preset_core/       # Domain models, resolvers, schema, services (pure Rust)
│   ├── preset_server/     # Local API server (axum)
│   └── preset_cli/        # CLI binary (serve, validate, format commands)
└── packages/
    └── preset_react/      # React SPA web editor (TanStack + Zustand + shadcn/ui)
```

| Crate/Package | Role | Dependencies |
|---------------|------|-------------|
| `preset_core` | Shared domain layer: models, resolvers, schema, services | serde, jsonschema, regex, chrono |
| `preset_server` | Local API server: config CRUD, preview, feed caching, file watching | preset_core, axum 0.8, tokio, notify 7, reqwest, feed-rs, rust-embed |
| `preset_cli` | CLI binary: serve, validate, format subcommands | preset_core, preset_server, clap |
| `preset_react` | Web editor UI: pattern browsing, config editing, preview | React 19, TanStack Query/Router, Zustand, RHF, Zod 4, CodeMirror 6, i18next, dnd-kit |

## Ecosystem Context

This repo is one part of a three-component ecosystem:

```
User clones data repo locally
                |
                v
[audiflow-preset-editor]              Local data repo clone         GitHub (remote)
 (this repo)                  read/write  (on user's machine)  push    (source of truth)
 preset_server + preset_react  <────────────────>  JSON files on disk  ──────>  origin/main
                                                                CI sync
                                                                ──────>  GitHub Pages / GCS
                                                                            ^
                                                                            |
                                                                         audiflow app fetches
```

- **audiflow-preset** (data repo): Static JSON files on GitHub, source of truth
- **GitHub Pages / GCS**: Mirrors the data repo; the mobile app fetches configs from here
- **audiflow app**: Consumes configs via `audiflow_domain` with local caching
- **This repo**: Local web editor that reads/writes files in a cloned data repo

Model serialization (JSON keys, field structure) must stay aligned across all three.

## preset_core

Pure Rust library crate with no framework dependencies. All domain logic lives here.

### Models

Core types use Rust structs with `serde::Serialize`/`Deserialize`. Custom serialization uses `#[serde(...)]` attributes.

| Model | Purpose |
|-------|---------|
| `EpisodeData` | Trait for episode data (id, title, description, season/episode numbers, published_at, image_url) |
| `SimpleEpisodeData` | Concrete `EpisodeData` implementation with serde support |
| `Playlist` | A playlist with id, display_name, sort_key, episode_ids, groups, playlist_structure, year_binding |
| `PlaylistGroup` | A group within a playlist (id, display_name, sort_key, episode_ids, metadata fields) |
| `PlaylistStructure` | Enum: `Split` (default) or `Grouped` |
| `YearBinding` | Enum: `None` (default), `PinToYear`, `SplitByYear` |
| `Grouping` | Resolver output: playlists + ungrouped_episode_ids + resolver_type |
| `PlaylistDefinition` | Per-playlist config: resolver_type, filters, extractors, groups, display settings |
| `PresetConfig` | Per-podcast config: id, podcast_guid, feed_urls, year_grouped_episodes, playlists |
| `PresetMeta` / `PresetSummary` / `RootMeta` | Split config metadata hierarchy |
| `EpisodeFilters` / `EpisodeFilterEntry` | Require/exclude regex filters on title/description |
| `GroupDef` | Static group definition with id, display_name, pattern, display, episode_list, numbering_extractor |
| `GroupListSettings` | year_binding, user_sortable, show_date_range, sort rule |
| `EpisodeListSettings` | show_year_headers, sort rule, title_extractor |
| `TitleExtractor` | Regex-based display name extraction with source, pattern, group, template, fallback chain |
| `NumberingExtractor` | Season/episode number extraction with primary/fallback patterns and RSS fallback |
| `SortRule` / `SortField` / `SortOrder` | Group-level sorting (PlaylistNumber, NewestEpisodeDate, Alphabetical) |
| `EpisodeSortRule` / `EpisodeSortField` | Episode-level sorting (PublishedAt, EpisodeNumber, Title) |
| `PlaylistPreviewResult` | Preview output: definition_id, playlist, claimed_by_others map |
| `PreviewGrouping` | Preview output: playlist_results, ungrouped_episode_ids, resolver_type |

### Resolver Chain

Resolvers implement the `Resolver` trait and group episodes by different strategies:

```rust
trait Resolver {
    fn resolver_type(&self) -> &str;
    fn default_sort(&self) -> SortRule;
    fn resolve(&self, episodes: &[&dyn EpisodeData], definition: Option<&PlaylistDefinition>) -> Option<Grouping>;
}
```

| Resolver | Strategy |
|----------|----------|
| `RssResolver` | Groups by `seasonNumber` field (resolver type: `seasonNumber`) |
| `CategoryResolver` | Groups by regex patterns against group definitions (resolver type: `titleClassifier`) |
| `YearResolver` | Groups by publication year |
| `TitleAppearanceResolver` | Groups by title pattern, ordered by first appearance (resolver type: `titleDiscovery`) |

`ResolverService` orchestrates the chain:

1. Match podcast by GUID or feed URL against `PresetConfig` list
2. If matched: route episodes through definitions in priority order, filtering by require/exclude regexes
3. Higher-priority definitions claim episodes, preventing lower-priority ones from receiving them
4. If no match: try resolvers in order with no definition (auto-detect mode)
5. Sort all episode IDs by `publishedAt` ascending (nulls last)
6. Return `Grouping` or `None`

Playlist structure determines output shape:
- `Split`: Each resolver playlist becomes a top-level `Playlist`
- `Grouped`: Resolver playlists become `PlaylistGroup` entries inside one parent playlist

### Services

| Service | Purpose |
|---------|---------|
| `ResolverService` | Resolver chain orchestrator with `resolve_smart_playlists()` and `resolve_for_preview()` |
| `ConfigAssembler` | Combines `PresetMeta` + playlist definitions into unified `PresetConfig` |
| `sort_episode_ids_by_published_at` | Three-tier sorting: has date > no date but found > not found |
| `sort_groups` | Sorts groups by `SortRule`, populates earliest_date/latest_date |

### Schema

Three JSON Schema files embedded via `include_str!` from `crates/preset_core/assets/`:

- `preset-index.schema.json` -- root `meta.json`
- `preset-meta.schema.json` -- per-pattern `meta.json`
- `playlist-definition.schema.json` -- playlist definitions

`Validator` struct wraps `jsonschema` crate for runtime validation. Supports `from_embedded()` and `from_dir()` construction. `SchemaType` enum selects which schema to validate against.

## preset_server

Axum-based local API server with tokio async runtime. Runs on localhost only, no authentication required.

### App State

```rust
pub struct AppState {
    pub config_repo: LocalConfigRepository,
    pub feed_cache: DiskFeedCacheService,
    pub validator: Validator,
    pub file_watcher: FileWatcherService,
    pub schema_json: String,
    pub http_client: reqwest::Client,
    pub static_dir: Option<PathBuf>,
}
```

`AppError` provides `bad_request()`, `not_found()`, `internal()`, `bad_gateway()` constructors. Implements `IntoResponse` returning JSON with error message + HTTP status.

### Routes

| Endpoint | Purpose |
|----------|---------|
| `GET /api/health` | Health check |
| `GET /api/schema` | Playlist-definition JSON Schema |
| `GET /api/configs/presets` | List pattern summaries |
| `POST /api/configs/presets` | Create new pattern (requires id, meta) |
| `GET /api/configs/presets/{id}` | Get pattern metadata |
| `DELETE /api/configs/presets/{id}` | Delete pattern and all playlists |
| `PUT /api/configs/presets/{id}/meta` | Update pattern metadata (merges, preserves dataVersion) |
| `GET /api/configs/presets/{id}/assembled` | Assemble full config |
| `GET /api/configs/presets/{id}/playlists/{pid}` | Get playlist definition |
| `PUT /api/configs/presets/{id}/playlists/{pid}` | Save playlist definition |
| `DELETE /api/configs/presets/{id}/playlists/{pid}` | Delete playlist |
| `POST /api/configs/validate` | Validate config against schema |
| `POST /api/configs/preview` | Preview smart playlists from config + feed |
| `GET /api/feeds?url=...` | Fetch and parse RSS feed (http/https only) |
| `GET /api/events` | SSE stream of file change events |
| Static fallback | Serves files from `--static-dir` or embedded assets; SPA fallback for extensionless paths |

### Services

| Service | Purpose |
|---------|---------|
| `LocalConfigRepository` | Read/write split config files with atomic writes and path traversal protection |
| `DiskFeedCacheService` | Disk-based feed cache with SHA-256 URL hashing, configurable TTL, `error_for_status()` on HTTP responses |
| `FileWatcherService` | Watch data directory via `notify` crate, debounced event batching, broadcast channel for SSE; ignores `.tmp` and `.cache` |
| `FeedParser` | RSS feed parsing via `feed-rs` crate |
| `atomic_write_str` | Write to `.tmp` then rename; Windows-safe with remove-before-rename |

### Local-First Architecture

- Server accepts `--data-dir` flag pointing to a cloned data repo
- Binds to `127.0.0.1` only
- No authentication required
- File changes trigger SSE events to connected browsers (via `notify` crate)
- Feed cache stored in `$dataDir/.cache/feeds/`
- Static files served via `--static-dir` flag or `rust-embed` fallback
- Path segments validated to prevent directory traversal

## preset_cli

CLI binary (`audiflow-editor`) built with `clap`. Provides subcommands:

| Command | Purpose |
|---------|---------|
| `serve` | Start the API server (`--data-dir .`, `--port 8080`, `--static-dir`); validates `presets/meta.json` exists |
| `validate` | Validate configs against JSON schema; exits 0 (valid), 1 (errors), 2 (file not found); auto-detects schema type from path |
| `format` | Format/normalize JSON files with `--check` mode for CI |

`config_walker` walks the pattern directory tree calling a callback per file with detected `SchemaType`.

## preset_react

React 19 SPA built with Vite + TypeScript.

### Tech Stack

- **Routing**: TanStack Router (file-based)
- **Server state**: TanStack Query (caching, refetching)
- **Local state**: Zustand (editor-store)
- **Forms**: React Hook Form + Zod 4 (zodResolver)
- **Styling**: Tailwind CSS v4 + shadcn/ui (new-york style)
- **JSON editing**: CodeMirror 6
- **Drag and drop**: dnd-kit
- **i18n**: i18next + react-i18next (en, ja)
- **Linting**: oxlint
- **Testing**: Vitest + @testing-library/react

### Routes

| Route | Screen |
|-------|--------|
| `/browse` | Pattern listing |
| `/editor` | Create new config |
| `/editor/$id` | Edit existing pattern |
| `/feeds` | Feed browser |

### Key Components

- `ApiClient`: Simple HTTP wrapper for API calls (no auth)
- Stores (Zustand): `editor-store` (UI state)
- `useFileEvents`: SSE hook for real-time TanStack Query cache invalidation
- Query hooks: `usePresets`, `useAssembledConfig`, `useFeed`, `usePreviewMutation`, `useSavePlaylist`, `useSavePresetMeta`, `useDeletePlaylist`, `useDeletePattern`, `useCreatePattern`, etc.

## Split Config Structure

Configs are stored as a three-level file hierarchy:

```
patterns/
  meta.json                             # Root: version + pattern summaries
  {presetId}/
    meta.json                           # Pattern: feedUrls, playlistIds, flags
    playlists/
      {playlistId}.json                 # PlaylistDefinition
```

`LocalConfigRepository` reads/writes each level with atomic writes.
`ConfigAssembler` combines pattern meta + playlist files into a unified `PresetConfig`.

## Key Design Decisions

- **Serde for JSON serialization**: Custom `#[serde(...)]` attributes for field mapping and defaults
- **Plain structs**: Domain models are immutable structs, cloned when modified
- **Trait-based abstractions**: `EpisodeData` trait, `Resolver` trait
- **Local-first**: Server reads/writes local files, no remote API calls for config operations
- **Atomic file writes**: Write to `.tmp` then rename to prevent partial reads; Windows-safe
- **SSE for reactivity**: `FileWatcherService` broadcasts debounced file changes to connected browsers
- **Schema validation at boundaries**: Three embedded JSON Schemas validated via `jsonschema` crate
- **Claiming semantics**: Higher-priority definitions claim episodes in preview, preventing duplicates
- **Path traversal protection**: `LocalConfigRepository` validates path segments; feed endpoint restricts to http/https
