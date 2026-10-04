# System Overview

## Goal

Provide a local web editor that lets users create and manage smart playlist configurations for the audiflow podcast app, with live preview, schema validation, and file-system reactivity.

## Context

This repository is part of the audiflow podcast ecosystem:
- **audiflow** (Flutter app): Consumes configs from hosted mirrors (GitHub Pages)
- **audiflow-preset** (data repo): Static JSON files for all environments (prod/staging/dev), deployed to GitHub Pages from versioned branches
- **This repo** (editor): Reads/writes config files in a locally cloned data repo

## High-level structure

- **preset_core**: Pure Rust domain library. Models, resolvers, schema validation, services, cross-pattern uniqueness, deterministic pattern ID derivation. No framework dependencies.
- **preset_server**: Axum-based local API server. Config CRUD, feed caching, file watching, SSE, static file serving, pattern identifiers endpoint, pattern ID derivation endpoint.
- **preset_cli**: CLI binary wrapping preset_server. Subcommands: `serve`, `validate`, `format`, `bump-versions`.
- **preset_react**: React 19 SPA. Pattern browsing, config editing with forms, live preview, SSE-driven cache invalidation, inline duplicate detection, auto-derived pattern IDs.

## Main data flow

1. User clones the data repo (`audiflow-preset`) locally
2. User starts the editor: `cargo run -- serve --data-dir /path/to/data-repo`
3. `preset_server` reads split config files from the data directory via `LocalConfigRepository`
4. `preset_react` SPA loads in the browser, fetches config data via REST API
5. User edits configs through forms; `preset_server` writes changes atomically to disk
6. For new patterns, the editor auto-derives a deterministic ID from podcastGuid or first non-empty trimmed feedUrl via `POST /api/configs/derive-pattern-id`
7. `FileWatcherService` detects file changes (including external edits) and broadcasts SSE events
8. `preset_react` receives SSE events and invalidates TanStack Query cache for real-time updates
9. For preview: `preset_server` fetches/caches RSS feeds, runs resolver chain, returns grouped episodes
10. User commits and pushes changes to the data repo themselves

## Primary interfaces

- **Input**: Local JSON config files (split config structure), RSS feed URLs (HTTP/HTTPS)
- **Output**: Modified JSON config files on disk, preview results via API
- **External dependencies**: RSS feed servers (for episode data), local filesystem (for config persistence)
- **Internal API**: REST endpoints on `127.0.0.1` (see `.claude/rules/project/architecture.md` for full route table)

## Design constraints

- **Local-only**: Server binds to `127.0.0.1` by default (`--host 0.0.0.0` for Docker/LAN), no authentication, no remote config storage
- **Atomic writes**: All file writes go through `.tmp` then rename to prevent partial reads
- **Schema at boundaries**: Three JSON Schemas validate config at write time and via CLI
- **Path traversal protection**: `LocalConfigRepository` validates all path segments
- **Feed URL restriction**: Only `http://` and `https://` URLs accepted for feed fetching
- **Embedded assets**: JSON Schemas embedded via `include_str!`; SPA assets via `rust-embed` (with `--static-dir` override)
- **Cross-pattern uniqueness**: Server enforces that no two patterns share the same podcastGuid or feedUrl on create/update
- **Deterministic IDs**: New patterns receive a 12-hex-char ID derived from podcast identity; legacy IDs are grandfathered

## When to update

Update when: crates/packages added or removed, data flow changes, new external dependencies introduced, design constraints modified.
