# Overview

## Purpose

audiflow-preset-editor is a local-first web editor for creating and managing
smart playlist configurations used by the audiflow podcast app. It consists of a Rust
backend (API server + domain logic + CLI) and a React frontend. The editor reads and
writes JSON config files in a locally cloned data repository; users handle git
operations (commit, push, PR) themselves.

## Responsibilities

- Provide a web UI for browsing, creating, editing, and previewing smart playlist configs
- Define and enforce JSON Schema for all config files (three schemas)
- Implement episode resolver logic that groups episodes into playlists
- Serve a local API for config CRUD, RSS feed fetching/caching, and live preview
- Derive deterministic pattern IDs from podcast identity (podcastGuid or first non-empty feedUrl)
- Provide CLI tools for validation (`validate`), formatting (`format`), serving (`serve`), and version bumping (`bump-versions`)
- Watch the local data directory for external changes and notify the browser via SSE
- Enforce cross-pattern uniqueness of podcast identifiers (podcastGuid, feedUrls)

## Non-responsibilities

- Hosting or deploying config data (owned by data repo CI pipelines)
- Managing git operations on data repos (user responsibility)
- Mobile app playback, caching, or UI (owned by `audiflow` Flutter app)
- Production, staging, or dev data content (owned by `audiflow-preset` repo, branched by environment)

## Main concepts

- **Pattern**: A podcast-specific configuration identified by a unique ID. Contains feed URLs, podcast GUID, flags, and playlist definitions.
- **Deterministic pattern ID**: A 12-character hex string derived from podcast identity (podcastGuid or first non-empty trimmed feedUrl) via MD5. New patterns use deterministic IDs; legacy IDs are grandfathered.
- **Playlist definition**: A JSON config describing how episodes are grouped, filtered, sorted, and displayed.
- **Resolver**: A strategy that groups episodes into playlists. Types: `seasonNumber`, `titleClassifier`, `year`, `titleDiscovery`.
- **Split config**: The three-level file hierarchy (`patterns/meta.json` -> `{id}/meta.json` -> `{id}/playlists/{pid}.json`).
- **Schema**: Three JSON Schema files in `crates/preset_core/assets/` that validate each level of the split config.
- **Claiming**: Higher-priority playlist definitions claim episodes during preview, preventing duplicates in lower-priority definitions.
- **Data repo**: A git repository containing JSON config files (`audiflow-preset` for all environments).
- **Cross-pattern uniqueness**: Validation ensuring no two patterns share the same podcastGuid or feedUrl values.

## Primary entry points

- `crates/preset_cli/src/main.rs`: CLI binary entry point (`serve`, `validate`, `format`, `bump-versions` subcommands)
- `crates/preset_server/src/app.rs`: Axum router and app state construction
- `crates/preset_core/src/lib.rs`: Domain library root (models, resolvers, schema, services)
- `packages/preset_react/src/`: React SPA root (Vite + TanStack Router)

## Key dependencies

- **Rust**: serde, jsonschema, axum 0.8, tokio, reqwest, feed-rs, notify 7, clap, rust-embed, chrono, fancy-regex, sha2, md5
- **React**: React 19, TanStack Query/Router, Zustand, React Hook Form, Zod 4, CodeMirror 6, Tailwind v4, shadcn/ui, dnd-kit, i18next

## Read next

- docs/architecture/system-overview.md
- docs/architecture/module-boundaries.md
- docs/integration/editor-to-schema.md
- docs/integration/preset-contract.md
- docs/development/change-workflow.md

## When to update

Update this document when:
- Repository purpose or scope changes
- New crates or packages are added to the workspace
- Major dependencies are added or removed
- Responsibility boundaries shift between this repo and siblings
