# Review Checklist

## Behavior

- Is the intended behavior clear from the PR description?
- Is there test coverage for the changed behavior?
- If resolver logic changed, do resolver tests (`crates/preset_core/tests/resolver_tests.rs`) cover the new cases?
- If model serialization changed, do model tests (`crates/preset_core/tests/model_tests.rs`) verify JSON round-tripping?

## Boundaries

- Does the change respect module boundaries (see docs/architecture/module-boundaries.md)?
- Is preset_core still free of I/O and framework dependencies?
- If domain models changed in preset_core, was the preset_react Zod schema also updated?
- Has any responsibility shifted between crates/packages or between repos?

## Schema and contracts

- If JSON Schema files changed, were all consumers listed in docs/integration/editor-to-schema.md notified?
- If file structure changed, was docs/integration/preset-contract.md updated?
- Do schema conformance tests pass (`crates/preset_core/tests/schema_tests.rs`)?

## Uniqueness and identifiers

- If podcast identifier fields changed, was cross-pattern uniqueness validation updated in preset_core?
- Does preset_server still enforce uniqueness on pattern create/update?
- Does `validate` CLI still check cross-pattern uniqueness?

## Security

- Are new API endpoints protected against path traversal?
- Do new feed-related endpoints restrict to http/https URLs only?
- Are file writes using atomic write (`.tmp` then rename)?

## Documentation

- Are relevant docs updated (overview, architecture, integration, development)?
- If `.claude/rules/project/architecture.md` content is affected, was it updated?
- Are new concepts defined in docs/overview.md?

## Quality gates

- `cargo test` passes
- `cargo clippy -- -W warnings` has zero warnings
- `pnpm test -- --run` passes (preset_react)
- `npx oxlint` passes (preset_react)
- `npx tsc -b --noEmit` passes (preset_react)

## When to update

Update when: new review criteria emerge, quality gates change, new security concerns are identified.
