# Preset Data Repo Contract

## Purpose

Documents the file structure and format contract between this editor and the preset data repository (`audiflow-preset`), which holds config data for all environments (prod, staging, dev) on separate versioned branches.

## Scope

This document covers:
- The split config file structure that this editor reads and writes
- Format and naming conventions the editor enforces
- What the editor assumes about the data directory
- How data repos are deployed and consumed downstream

## Responsibilities

- Read and write config files following the split config structure
- Enforce JSON Schema validation on all written files
- Use atomic writes (`.tmp` then rename) to prevent partial reads
- Validate path segments to prevent directory traversal
- Provide `format` CLI command for JSON normalization (supports directory arguments)
- Provide `validate` CLI command for schema validation, cross-pattern uniqueness, and deterministic pattern ID integrity (usable in data repo CI)
- Provide `bump-versions` CLI command for CI-driven dataVersion increments

## Non-responsibilities

- Managing git operations on data repos (user responsibility)
- CI/CD deployment of data repos to hosting (owned by data repo pipelines)
- Content decisions about which podcasts or playlists exist
- Hosting config files for app consumption (owned by GitHub Pages)

## File structure contract

The editor expects and produces this directory layout in the data directory:

```
presets/
  meta.json                         # Root index: dataVersion, schemaVersion, pattern summaries
  {presetId}/
    meta.json                       # Pattern metadata: id, feedUrls, podcastGuid, yearGroupedEpisodes, playlists[]
    playlists/
      {playlistId}.json             # Playlist definition: resolverType, filters, groups, sort, display
```

### File-level schema mapping

| File | Schema | SchemaType enum value |
|------|--------|-----------------------|
| `presets/meta.json` | `preset-index.schema.json` | `PresetIndex` |
| `presets/{id}/meta.json` | `preset-meta.schema.json` | `PresetMeta` |
| `presets/{id}/playlists/{pid}.json` | `playlist-definition.schema.json` | `PlaylistDefinition` |

### Naming rules

- `presetId`: string identifier, used as directory name (alphanumeric, hyphens, underscores only -- enforced by `validate_path_segment`). New presets use a deterministic 12-hex-char ID derived from podcast identity; legacy human-readable IDs are grandfathered.
- `playlistId`: string identifier, used as filename without extension (alphanumeric, hyphens, underscores only)
- All JSON files use 2-space indentation with trailing newline

### Version fields

- `dataVersion` in root `meta.json`: incremented by the editor or `bump-versions` CLI when presets change
- `schemaVersion` in root `meta.json`: tracks which schema version the data conforms to

## Data flow

```
Editor writes -> Local data repo clone -> User pushes -> CI deploys -> GitHub Pages -> App fetches
```

The `main` branch holds infrastructure only. Data and vendored schemas live on env/version branches:

| Branch | Deploy path |
|--------|------------|
| `prod/v{N}` | `/assets/v{N}/` |
| `stg/v{N}` | `/assets-stg/v{N}/` |
| `dev/v{N}` | `/assets-dev/v{N}/` |

Branch flow per version: `dev/v{N}` -> PR -> `stg/v{N}` -> PR -> `prod/v{N}`

## Integration assumptions

- The data directory contains a valid `presets/meta.json` before the server starts (`serve` command validates this)
- External tools or manual edits may modify files while the server is running (FileWatcherService handles this)
- Feed cache is stored in `$dataDir/.cache/feeds/` and is not part of the data contract
- `.tmp` files are transient and should be gitignored
- Cross-pattern uniqueness: no two patterns may share the same podcastGuid or feedUrl

## Related documents

- docs/integration/editor-to-schema.md -- schema definitions that validate this file structure
- docs/architecture/module-boundaries.md -- LocalConfigRepository in preset_server owns file I/O
- docs/overview.md -- overall purpose and concepts

## When to update

Update this document when:
- Split config file structure changes (new levels, renamed files)
- Naming conventions or version field semantics change
- New data repos are added to the ecosystem
- Deployment targets change (e.g., new hosting provider)
