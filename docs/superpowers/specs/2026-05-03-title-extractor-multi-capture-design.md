# TitleExtractor: Multi-capture template syntax

Date: 2026-05-03
Status: Approved (design)
Target version: v6 (additional breaking change within v6)

## Problem

The current `TitleExtractor` schema cannot compose multiple regex capture groups
into a single output string. A user can pick exactly one capture group via the
`group` integer field, then format it with `template` (using the `{value}`
placeholder).

Example title:
```
【アダム・スミス9】社会の秩序をつくるのは「優しさ」か「正義感」か。スミスが出した答えとは？#150
```
Desired output:
```
9. 社会の秩序をつくるのは「優しさ」か「正義感」か。スミスが出した答えとは？
```

This requires combining two capture groups (the number `9` and the trailing
title text) into one rendered string. The current schema cannot express this:
`group` is a single integer, and `{value}` resolves to that one group only.

## Decision summary

- Drop `TitleExtractor.group`. Group selection happens inside `template`.
- Replace the `{value}` placeholder with `${N}` references where `N` is a
  capture group index (`${0}` = full match, `${1}` = first capture, ...).
- `template` omitted → behaves as `${0}`.
- `pattern` omitted → source value is treated as `${0}`; `${1}+` are empty.
- Out-of-range references → empty string (lenient).
- No named captures (`${name}`) in this change. Numeric only.
- No automatic data migration. Users edit existing JSON by hand.
- All changes ride on v6 (already bumped, not yet shipped).

## Schema change

File: `crates/sp_core/assets/playlist-definition.schema.json`

`TitleExtractor` becomes:

```json
{
  "type": "object",
  "required": ["source"],
  "additionalProperties": false,
  "properties": {
    "source": { "...": "unchanged" },
    "pattern": { "...": "unchanged" },
    "template": {
      "type": "string",
      "description": "How to format the extracted text. Use ${0} for the full match, ${1}, ${2}, ... for capture groups. Example: '${1}. ${2}' joins the first and second capture groups with a period."
    },
    "fallback": { "...": "unchanged" },
    "fallbackValue": { "...": "unchanged" }
  }
}
```

The `group` field is removed. `additionalProperties: false` is kept; old
configs still containing `group` will fail validation and must be edited.

`fallback` is a recursive `$ref` to `TitleExtractor`, so the new template
semantics apply transitively to every link in the fallback chain. Each
fallback step has its own `pattern` and `template` and is rendered
independently with the rules below.

## Rendering semantics

1. `fallback_value` early-return for null/zero `seasonNumber` is unchanged.
2. Read `source` value. If missing, try `fallback`.
3. If `pattern` is set: run regex. No match → try `fallback`.
4. Render `template`:
   - `template` is `Some(t)`: replace each `${N}` token in `t`.
     - With `pattern`: substitute `captures.get(N)` if present, otherwise empty
       string.
     - Without `pattern`: `${0}` substitutes the source value; `${1}+`
       substitute empty string.
   - `template` is `None`:
     - With `pattern`: equivalent to template `${0}` (the full match).
     - Without `pattern`: the raw source value.

### `${N}` parser

Single-pass scan over `template`:

- On `$`, peek next char.
  - If `{`, consume up to the matching `}` and parse the inside as an integer.
    - On valid integer → substitute that capture group.
    - On parse failure or missing `}` → emit the original characters
      literally.
  - Otherwise emit the `$` literally.

This means literal `$` outside `${...}` round-trips unchanged. There is no
explicit escape syntax for `${`; users who need a literal `${` would have to
break it across two strings, which is unsupported in this design (deferred).

## Rust implementation

File: `crates/sp_core/src/models/title_extractor.rs`

Struct change:
```rust
pub struct TitleExtractor {
    pub source: String,
    pub pattern: Option<String>,
    pub template: Option<String>,
    pub fallback: Option<Box<TitleExtractor>>,
    pub fallback_value: Option<String>,
}
```
(`group: u32` removed, plus its `is_zero` helper.)

`CompiledTitleExtractor::extract` flow:

```text
if let Some(captures) = regex_captures_or_source_only:
    return render(template, captures)
else:
    return fallback.extract(...)
```

A small `render(template: Option<&str>, captures: Captures)` helper centralizes
template handling. With no `pattern`, "captures" is a degenerate single-element
view of the source value (a tiny enum or function param avoids allocating a
fake captures struct).

## React/UI changes

Path prefix: `packages/sp_react/src/`

- `components/editor/title-extractor-form.tsx`:
  - Remove the `group` input (current label `titleExtractorGroup`).
  - Update the `template` input help/placeholder to describe `${N}` and show
    a multi-capture example.
- `schemas/config-schema.ts`: drop `group` from the Zod schema.
- `locales/{ja,en}/editor.json`: remove `titleExtractorGroup` key.
- `locales/{ja,en}/hints.json`: remove `titleExtractorGroup` key; rewrite
  `titleExtractorTemplate` to describe `${N}`.

The frontend does not render display names locally; the preview API renders
on the server. No client-side template engine is needed.

## Tests

Rust (`title_extractor.rs`):

- New: multi-capture join `${1}. ${2}` against the user's example title.
- New: out-of-range reference (`${5}` with two capture groups) → empty string,
  not a fallback trigger.
- New: `pattern` omitted, template `${0}` → source value.
- New: literal `$` mixed with `${1}` (`'${1} - $'`).
- New: fallback chain where the primary step has `template: "${1}. ${2}"`
  and the fallback step has its own pattern + template — confirms each link
  renders with the new semantics independently.
- Updated: existing tests using `group: N` are rewritten as `template: "${N}"`
  with equivalent semantics.

React: remove assertions referencing the `group` input from
`title-extractor-form` tests.

## Documentation

- `docs/schema-reference.md`: rewrite the `TitleExtractor` section. Add a
  multi-capture example.
- `docs/integration/editor-to-schema.md`: update if any `group:` examples
  appear.
- `docs/integration/preset-contract.md`: same — scrub `group:` examples
  if present.
- Any embedded fixture/example JSON using `group:` is converted to
  `template: "${N}"`.

## Migration

None. Existing JSON files containing `group: N` will fail schema validation.
Users edit them by hand: delete `group: N`, prepend or insert `${N}` into
`template` as needed.

A migration tool can be added later if real-world breakage motivates it.

## Out of scope

- Named capture groups (`${name}`). Deferred; revisit when a user need appears.
- Literal `${` escape syntax. Deferred; current rule is "malformed `${...}` is
  emitted literally".
- Automatic migration. Deferred (see above).
