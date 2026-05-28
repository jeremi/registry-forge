# Forge Recipe V1 Reference

Read this before creating or editing `forge.recipe.yaml`.

The recipe is strict YAML. Unknown fields are rejected. Paths are resolved
relative to the recipe file.

## Minimal Shape

```yaml
version: forge.recipe.v1
project:
  name: demo-registry
  reviewers:
    - id: demo-reviewer
      name: Demo Reviewer
      role: data_steward
source:
  id: source.main
  type: file
  path: data/source.csv
  format: csv
  hash:
    algorithm: sha256
    value: 64_hex_sha256_digest
  parser:
    family: csv
    version: "1"
profile_bundle:
  id: publicschema-demo
  path: profiles/publicschema-demo.bundle.json
  hash:
    algorithm: sha256
    value: 64_hex_sha256_digest
fields: {}
semantic_alignments: []
value_crosswalks: {}
mappings:
  engine: crosswalk
  file: mappings/crosswalk.mapping.yaml
validation:
  required_targets: []
candidates:
  relay:
    status: draft
  manifest:
    status: draft
  witness:
    status: draft
review:
  status: draft
```

## Top-Level Sections

Required:

- `version`
- `project`
- `source`
- `profile_bundle`
- `mappings`
- `validation`
- `candidates`
- `review`

Optional with defaults:

- `fields`, default `{}`
- `semantic_alignments`, default `[]`
- `value_crosswalks`, default `{}`

Do not add unknown top-level sections.

## Project

```yaml
project:
  name: demo-farmer-registry
  reviewers:
    - id: demo-reviewer
      name: Demo Reviewer
      role: data_steward
```

Rules:

- `project.name` is required.
- Reviewer `id`, `name`, and `role` are required for each reviewer.
- Accepted semantic alignments must reference a known reviewer id.

## Source

CSV:

```yaml
source:
  id: source.main
  type: file
  path: data/farmers.csv
  format: csv
  hash:
    algorithm: sha256
    value: 72e497effe7dc0c6de37789b607de153e8e27a39d54b23b61ed7c4e3f46f9fce
  parser:
    family: csv
    version: "1"
```

XLSX:

```yaml
source:
  id: source.main
  type: file
  path: data/farmers.xlsx
  format: xlsx
  hash:
    algorithm: sha256
    value: 64_hex_sha256_digest
  workbook:
    sheet: Farmers
    header_row: 1
  parser:
    family: xlsx
    version: "1"
```

Rules enforced by the CLI:

- `source.type` must be `file`.
- `source.format` must be `csv` or `xlsx`.
- Legacy `.xls` files are rejected by `init`.
- `source.path` must be relative and must not contain `..`.
- `source.hash.algorithm` must be `sha256`.
- `source.hash.value` must be a 64-character hex digest.
- `parser.family` and `parser.version` are required strings.
- `workbook.header_row` is 1-based and defaults to `1` when omitted.
- For multi-sheet XLSX files, set `workbook.sheet` intentionally.

## Profile Bundle

```yaml
profile_bundle:
  id: publicschema-demo
  path: profiles/publicschema-demo.bundle.json
  hash:
    algorithm: sha256
    value: 47b2898c534af25ccb9f0fb41e93a738c6c32041e82ebf3007802736f54cb16a
```

Rules:

- `profile_bundle.path` must be relative and must not contain `..`.
- `profile_bundle.hash.algorithm` should be `sha256`.
- `suggest-alignments` and readiness verification check the bundle hash.

Profile bundle shape used by the MVP:

```json
{
  "id": "publicschema-demo",
  "terms": [
    {
      "id": "publicschema:farmer.identifier",
      "label": "Farmer identifier",
      "aliases": ["farmer id", "farmer identifier"]
    }
  ]
}
```

## Fields

```yaml
fields:
  farmer_id:
    source_name: Farmer ID
    role: identifier
    sensitivity: low
    type_hint: string
  full_name:
    source_name: Full Name
    role: attribute
    sensitivity: high
    type_hint: string
  district:
    source_name: District
    role: attribute
    sensitivity: low
    type_hint: string
    code_list: district_codes
```

Rules:

- Field map keys are stable field ids used in mappings as `source.<field_id>`.
- `source_name` must match the raw source column name.
- `role`, `sensitivity`, and `type_hint` are required strings.
- `code_list` is optional.
- The CLI does not currently enforce enum values for `role`, `sensitivity`, or
  `type_hint`, but use the conventions below for consistency.

Recommended `role` values:

- `identifier`
- `attribute`
- `status`
- `date`
- `measure`
- `note`

Recommended `sensitivity` values:

- `low`
- `medium`
- `high`

Recommended `type_hint` values:

- `string`
- `integer`
- `decimal`
- `boolean`
- `date`
- `datetime`

## Semantic Alignments

```yaml
semantic_alignments:
  - source_field: farmer_id
    target: publicschema:farmer.identifier
    match_level: exact
    status: accepted
    confidence: high
    reviewer: demo-reviewer
```

Rules:

- `source_field` references a key in `fields`.
- `target` is a semantic profile term id.
- `status` is required.
- `confidence` is required.
- `match_level` and `reviewer` are optional in schema, but accepted alignments
  must include `reviewer`.
- Accepted alignment reviewer ids must exist in `project.reviewers`.

Recommended `status` values:

- `suggested`
- `needs_review`
- `accepted`
- `rejected`

Recommended `match_level` values:

- `exact`
- `close`
- `lossy`
- `uncertain`
- `needs_review`

Recommended `confidence` values:

- `low`
- `medium`
- `high`

## Value Crosswalks

```yaml
value_crosswalks:
  district_codes:
    source_field: district
    target_code_list: publicschema:district
    status: accepted
```

Rules:

- The map key must match `fields.<field_id>.code_list`.
- `source_field` must match the field declaring that `code_list`.
- `target_code_list` identifies the target code list.
- `status` must be `accepted` for `ready_candidate`.
- MVP validation does not enumerate every observed source value. Manually review
  profile top values for coverage when code-list correctness matters.

## Mappings

```yaml
mappings:
  engine: crosswalk
  file: mappings/crosswalk.mapping.yaml
```

Rules:

- `mappings.engine` must be `crosswalk`.
- `mappings.file` must be relative and must not contain `..`.
- The mapping file must compile before readiness can pass.

## Validation

```yaml
validation:
  required_targets:
    - publicschema:farmer.identifier
    - publicschema:farmer.location.district
```

Rules:

- Each required target must have a canonical mapping field named by taking the
  last part of the target and replacing separators with underscores. For
  example, `publicschema:farmer.location.district` maps to
  `farmer_location_district`.
- Each required target must have metadata under `x-forge.rules` in the
  Crosswalk mapping.
- Required target metadata cannot be generated-only or `needs_review`.

## Candidates And Review

```yaml
candidates:
  relay:
    status: draft
  manifest:
    status: draft
  witness:
    status: draft
review:
  status: draft
```

Rules:

- `relay`, `manifest`, and `witness` are required.
- Candidate artifacts exported by the MVP are draft placeholders.
- Keep `review.status` honest. Do not mark review complete unless project
  governance has actually completed.

## Hash And Path Commands

From the recipe directory:

```sh
shasum -a 256 data/source.csv
shasum -a 256 profiles/publicschema-demo.bundle.json
```

After updating source or profile-bundle hashes intentionally, rerun:

```sh
cargo run -- check-recipe forge.recipe.yaml
cargo run -- inspect-source forge.recipe.yaml
cargo run -- profile-source forge.recipe.yaml
cargo run -- preview-transform forge.recipe.yaml
cargo run -- validate-output --require-status ready_candidate forge.recipe.yaml
```

Do not update hashes merely to silence a mismatch. First confirm that the source
or profile bundle changed intentionally.
