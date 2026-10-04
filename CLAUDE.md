# audiflow-preset-editor

Local-first web editor for smart playlist configuration. Provides a Rust API server
(`preset_server`) and React SPA (`preset_react`) that read/write JSON config files in a
locally cloned data repo. Users manage git operations themselves.

## Ecosystem context

Part of the audiflow podcast ecosystem. This repo owns the editor UX, JSON Schema
definitions (`crates/preset_core/assets/`), and local-first editing workflow. The sibling
repo `audiflow-preset` holds config JSON files for all environments (prod/staging/dev,
deployed via GitHub Pages). The `audiflow` Flutter app consumes configs from hosted mirrors.

## Responsibilities

- Web-based editing of smart playlist configurations
- JSON Schema validation (three schemas in `crates/preset_core/assets/`)
- Episode resolver logic (seasonNumber, titleClassifier, year, titleDiscovery)
- Local API server for config CRUD, feed fetching, and live preview
- CLI tools for validation, formatting, serving, and version bumping
- Cross-pattern uniqueness enforcement (podcastGuid, feedUrls)
- Schema HTML doc generation (`make schema-doc`)

## Non-responsibilities

- Production/dev config data hosting (owned by data repo)
- Git operations on data repos (user responsibility)
- Mobile app behavior or playback logic (owned by `audiflow`)
- CI/CD deployment of config data (owned by data repo CI)

## Workspace structure

```
crates/preset_core/    -- Domain models, resolvers, schema, services (pure Rust)
crates/preset_server/  -- Local API server (axum, tokio)
crates/preset_cli/     -- CLI binary (serve, validate, format, bump-versions)
packages/preset_react/ -- React 19 SPA (Vite + TypeScript)
```

## Validation

```bash
cargo test                         # All Rust tests
cargo clippy -- -W warnings        # Lint (zero warnings required)
cd packages/preset_react && pnpm test -- --run  # React tests
cd packages/preset_react && npx oxlint          # JS lint
cd packages/preset_react && npx tsc -b --noEmit # Type check
make lint                          # All linters
make test                          # All tests
```

## Key references

- docs/overview.md -- Purpose, concepts, entry points
- docs/schema-reference.md -- Complete v5 schema field reference
- docs/architecture/system-overview.md -- Data flow and design constraints
- docs/architecture/module-boundaries.md -- Crate/package boundaries
- docs/integration/editor-to-schema.md -- Schema ownership and update process
- docs/integration/preset-contract.md -- Data repo file structure contract
- docs/development/change-workflow.md -- How to make changes safely
- docs/development/review-checklist.md -- PR review criteria

## When changing this repository

Check whether these also need updates:
- docs/ files listed above (if architecture, schema, or contracts change)
- `preset_react` Zod schema (`src/schemas/config-schema.ts`) if JSON Schema changes
- Sibling repos if schema or file structure changes (see docs/integration/)
- `.claude/rules/project/architecture.md` if crate structure changes
