# Change Workflow

## Before making changes

- Read docs/overview.md for repository purpose and concepts
- Read docs/architecture/module-boundaries.md to understand crate/package boundaries
- If the change involves schemas, read docs/integration/editor-to-schema.md
- If the change involves file structure, read docs/integration/preset-contract.md
- Identify whether the change is localized to one crate/package or crosses boundaries

## During implementation

- Keep changes within documented module boundaries (see docs/architecture/module-boundaries.md)
- preset_core must remain a pure library with no I/O or framework dependencies
- Domain model changes in preset_core require corresponding Zod schema updates in preset_react
- Add or update tests for all changed behavior
- Follow the branching policy in `.claude/rules/project/branching.md`
- If adding podcast identifier fields, update cross-pattern uniqueness validation in preset_core and preset_server

## Schema change checklist

When modifying JSON Schema or related models:

1. Update schema files in `crates/preset_core/assets/`
2. Update preset_core Rust models and serde attributes
3. Update preset_core tests (`crates/preset_core/tests/schema_tests.rs`, `model_tests.rs`)
4. Update preset_react Zod schema (`packages/preset_react/src/schemas/config-schema.ts`)
5. Update preset_react form components if field names or types changed
6. Regenerate schema HTML docs (`make schema-doc`)
7. Notify consumer repos to update their vendored schema copies
8. Update docs/integration/editor-to-schema.md if schema structure changed
9. Update docs/schema-reference.md if field semantics changed

## Required updates

Update documentation when:
- Architecture changes (new crates, changed boundaries) -> docs/architecture/*, `.claude/rules/project/architecture.md`
- Schema or config format changes -> docs/integration/editor-to-schema.md, docs/integration/preset-contract.md, docs/schema-reference.md
- New API endpoints -> `.claude/rules/project/architecture.md` (route table)
- New concepts or entry points -> docs/overview.md
- Process changes -> this document, docs/development/review-checklist.md

## Validation checklist

```bash
cargo test                                      # Rust tests pass
cargo clippy -- -W warnings                     # Zero clippy warnings
cd packages/preset_react && pnpm test -- --run      # React tests pass
cd packages/preset_react && npx oxlint              # JS lint passes
cd packages/preset_react && npx tsc -b --noEmit     # TypeScript compiles
```

All checks must pass before submitting a PR. Use `make lint` and `make test` as shortcuts.

## Useful Makefile targets

```bash
make test          # All tests (Rust + React)
make lint          # All linters (clippy + oxlint + tsc)
make schema-doc    # Regenerate schema HTML docs
make validate      # Validate configs in data directory
make format        # Format JSON configs in data directory
```

## When to update

Update this document when: validation commands change, new quality gates are added, the schema change process evolves.
