# audiflow-preset-editor

Local web editor for managing [audiflow](https://github.com/audiflow/audiflow) smart playlist configurations. Edit podcast playlist configs through a browser UI, preview resolver results against live RSS feeds, and save changes directly to your local data repo clone.

## Quick Start

### Prerequisites

- [Rust](https://www.rust-lang.org/tools/install) (edition 2024)
- [Node.js](https://nodejs.org/) 22+
- [pnpm](https://pnpm.io/) 10+

### Setup

1. Clone this repo and a data repo side by side:

```bash
git clone https://github.com/audiflow/audiflow-preset-editor.git
git clone https://github.com/audiflow/audiflow-preset.git
```

2. Install dependencies:

```bash
cd audiflow-preset-editor
make deps
```

3. Start the editor:

```bash
make dev
```

This launches the API server on port 8080 and the Vite dev server (usually http://localhost:5173). Open the URL Vite prints in your browser.

The data directory defaults to `../audiflow-preset`. To use a different one, see [Environment Variables](#environment-variables).

## Environment Variables

### Makefile variables

You can override these on the command line (`make dev DATA_DIR=...`) or set them in the environment (`DATA_DIR=... make dev`).

| Variable | Default | Used by | Description |
|----------|---------|---------|-------------|
| `DATA_DIR` | `../audiflow-preset` | `dev`, `dev-server`, `validate`, `format`, `format-check` | Path to the cloned data repo. It must contain `presets/meta.json` (or the legacy `patterns/meta.json`). |
| `SERVER_PORT` | `8080` | `dev`, `dev-server` | Port the API server listens on. It also sets the default `VITE_API_BASE_URL`. |

### React app (Vite)

| Variable | Default | Description |
|----------|---------|-------------|
| `VITE_API_BASE_URL` | `http://localhost:8080` | Base URL the SPA uses for REST calls and the SSE stream (`/api/events`). The Makefile exports it as `http://localhost:$(SERVER_PORT)`. |

Vite substitutes `VITE_API_BASE_URL` **at build time**. The SPA that `make build` or the Dockerfile produces keeps whatever value was set during the build, and falls back to `http://localhost:8080` if none was set. If you serve the built app on a different host or port, set the variable when you build:

```bash
VITE_API_BASE_URL=http://localhost:9000 make build
./target/release/audiflow-editor serve --data-dir ../audiflow-preset --port 9000
```

You can also put the value in `packages/preset_react/.env.local`, which Vite reads automatically.

Example: run the API server on port 9000 against a custom data repo:

```bash
make dev DATA_DIR=$HOME/src/my-data-repo SERVER_PORT=9000
```

The Rust binary does not read environment variables. Configure it with CLI flags (see [CLI Commands](#cli-commands)).

### Docker

```bash
docker build -t audiflow-editor .
docker run -p 8080:8080 -v /path/to/data-repo:/data audiflow-editor
```

Then open http://localhost:8080. The image serves the SPA from `/app/public` and reads data from `/data`. The built SPA calls `http://localhost:8080`, so keep the host port at `8080`, or rebuild the image with a different `VITE_API_BASE_URL` (see [Environment Variables](#environment-variables)).

## How It Works

The editor reads and writes JSON config files in your locally cloned data repo. You manage git operations (commit, push, PR) yourself.

```
Browser (preset_react)          API server (Rust/axum)
   |                           |
   |<--- HTTP REST API ------->|
   |<--- SSE (file changes) ---|
   |                           |
                         local data repo directory
                         +-- presets/meta.json
                         +-- presets/{id}/meta.json
                         +-- presets/{id}/playlists/{pid}.json
                         +-- .cache/feeds/          (gitignored)
```

Changes you make in the editor are written to disk immediately. The browser receives live updates via SSE when files change on disk.

## Ecosystem

This repo is part of a three-repo ecosystem:

| Repo | Role |
|------|------|
| audiflow-preset-editor (this repo) | Editor UI, JSON Schemas, CLI tools |
| [audiflow-preset](https://github.com/audiflow/audiflow-preset) | Config data for all envs (GitHub Pages) |
| [audiflow](https://github.com/audiflow/audiflow) | Flutter mobile app that fetches configs |

```
editor  <--read/write-->  local data repo  --push-->  GitHub  --CI-->  hosting
                                                                         ^
                                                                    audiflow app
```

## CLI Commands

The binary `audiflow-editor` provides four subcommands:

| Command | Description |
|---------|-------------|
| `serve` | Start the web editor server. Flags: `--data-dir` (default `.`), `--host` (default `127.0.0.1`), `--port` (default `8080`), `--static-dir` (serve the SPA from disk instead of the assets embedded in the binary) |
| `validate` | Validate config files against JSON Schema, and check that `podcastGuid` and `feedUrls` are unique across presets (`--data-dir`, optional file list) |
| `format` | Format and normalize config JSON (`--data-dir`, `--check` for CI, optional file list) |
| `bump-versions` | Bump `dataVersion` fields for presets that changed since a git ref, for use in CI (`<previous-ref>`, `--presets-dir`, `--json`) |

Run `audiflow-editor <command> --help` for details.

## Project Structure

```
audiflow-preset-editor/
├── crates/
│   ├── preset_core/       # Domain models, resolvers, schema validation (pure Rust)
│   ├── preset_server/     # API server (axum, tokio, SSE, feed caching)
│   └── preset_cli/        # CLI binary (serve, validate, format, bump-versions)
└── packages/
    └── preset_react/      # React SPA (TanStack, Zustand, shadcn/ui, CodeMirror)
```

## Config File Structure

Configs are stored as a three-level file hierarchy in data repos:

```
presets/
  meta.json                        # Root: version + preset summaries
  {presetId}/
    meta.json                      # Preset: feedUrls, playlistIds, flags
    playlists/
      {playlistId}.json            # Playlist definition
```

Data repos that still use the legacy v6 layout (`patterns/` instead of `presets/`) are still supported.

The canonical JSON Schema files live in `crates/preset_core/assets/`. See [docs/schema-reference.md](docs/schema-reference.md) for the field reference.

## Development

```bash
make dev          # Start server + React dev server
make dev-server   # Backend only
make dev-ui       # Frontend only
make test         # Run all tests (Rust + React)
make lint         # clippy + oxlint + tsc
make build        # Build React SPA + Rust release binary
make validate     # Validate configs against schema
make format       # Format JSON configs in DATA_DIR
make format-check # Check JSON formatting
make schema-doc   # Regenerate schema HTML docs
```

See `make help` for the full list.

## Contributing

Contributions are welcome! Please read our [Contributing Guide](CONTRIBUTING.md)
before submitting a pull request. All contributors must sign the
[Contributor License Agreement](CLA.md).

## License

This project is licensed under the [GNU Affero General Public License v3.0 or later](LICENSE).
