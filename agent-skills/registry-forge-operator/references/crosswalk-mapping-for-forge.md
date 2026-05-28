# Crosswalk Mapping For Forge

Read this before creating or editing `mappings/crosswalk.mapping.yaml`.

Forge uses Crosswalk to transform normalized source rows into canonical preview
records. Readiness validation also reads Forge review metadata from the mapping.

## Minimal Reviewed Mapping

```yaml
version: "0.1"
name: demo-farmer-registry
errors:
  mode: collect
x-forge:
  rules:
    publicschema:farmer.identifier:
      quality: exact
      reviewer: demo-reviewer
    publicschema:farmer.location.district:
      quality: exact
      reviewer: demo-reviewer
records:
  canonical:
    fields:
      farmer_identifier: "source.farmer_id"
      farmer_location_district: "source.district"
```

## Source Field Access

Forge normalizes each source row to stable field ids from `forge.recipe.yaml`.
Access values as:

```yaml
source.farmer_id
source.registration_status
source.registration_date
```

Do not access raw column labels such as `source["Farmer ID"]`. Use the stable
field ids from the recipe.

## Canonical Output Fields

Forge readiness expects required targets to appear under:

```yaml
records:
  canonical:
    fields:
      <canonical_field>: "<expression>"
```

Target-to-field examples:

- `publicschema:farmer.identifier` -> `farmer_identifier`
- `publicschema:farmer.location.district` -> `farmer_location_district`
- `publicschema:registration.status` -> `registration_status`
- `publicschema:registration.date` -> `registration_date`

Use the same canonical field names in preview review and readiness reasoning.

## Forge Rule Metadata

Every required target in `validation.required_targets` should have a matching
metadata entry:

```yaml
x-forge:
  rules:
    publicschema:registration.status:
      quality: exact
      reviewer: demo-reviewer
      on_missing: error
```

Supported `quality` values for readiness:

- `exact`
- `close`
- `lossy`
- `uncertain`
- `needs_review`

Supported `on_missing` values for readiness:

- `error`
- `skip`
- `use_default`
- `use_null`

Readiness blockers:

- Missing metadata for a required target.
- Missing canonical output field for a required target.
- `quality: needs_review` on a required target.
- `generated_by: scaffold-mapping` on a required target.
- Missing `reviewer` on a required target.
- Invalid `quality` or `on_missing` value.

Best practice:

- Add `x-forge.rules` metadata for every accepted semantic alignment, not only
  the required targets. The MVP readiness gate enforces metadata on required
  targets, but complete metadata makes review, audit, and later package
  promotion easier.
- Required targets are the hard gate. Non-required accepted alignments can still
  transform successfully without metadata, but that should be treated as
  incomplete review evidence.

## Scaffolded Mappings

`scaffold-mapping` generates a compilable starting point from accepted
alignments:

```sh
cargo run -- scaffold-mapping forge.recipe.yaml --out mappings/crosswalk.mapping.yaml
```

The scaffold marks rules as generated or needing review. That is intentional.
Do not expect a fresh scaffold to pass `ready_candidate`. Review the expressions,
set real `quality`, add `reviewer`, and remove generated-only markers when the
mapping is actually reviewed.

Use `--force` only when intentionally replacing an existing mapping:

```sh
cargo run -- scaffold-mapping forge.recipe.yaml --out mappings/crosswalk.mapping.yaml --force
```

## Expression Examples

Direct field copy:

```yaml
farmer_identifier: "source.farmer_id"
```

String composition:

```yaml
person_name: "source.first_name + ' ' + source.last_name"
```

Date parsing, when supported by the Crosswalk runtime:

```yaml
registration_date: "date.parse(source.registration_date, 'yyyy-MM-dd')"
```

Guarded record emission:

```yaml
records:
  canonical:
    emit: "present(source.farmer_id)"
    fields:
      farmer_identifier: "source.farmer_id"
```

Keep expressions simple for MVP demos. If an expression fails, inspect
`reports/transform-diagnostics.json`, fix the mapping, and rerun:

```sh
cargo run -- preview-transform forge.recipe.yaml
```

## Redaction Implications

Forge redacts preview fields that originate from high-sensitivity source fields
through accepted semantic alignments. For example, if `full_name` is high
sensitivity and aligned to `publicschema:person.name`, the canonical field
`person_name` should be redacted in `canonical-samples.redacted.jsonl`.

Do not treat redacted preview output as evidence that the underlying mapping is
wrong. Inspect source and mapping locally when allowed, but do not print
sensitive values in final reports.

## Mapping Verification Loop

After each mapping edit, first run a preview:

```sh
cargo run -- preview-transform forge.recipe.yaml
```

If the working directory is fresh, readiness also needs current inspection and
profile artifacts:

```sh
cargo run -- inspect-source forge.recipe.yaml
cargo run -- profile-source forge.recipe.yaml
cargo run -- preview-transform forge.recipe.yaml
```

Then validate:

```sh
cargo run -- validate-output forge.recipe.yaml
```

Before export, require the final status explicitly:

```sh
cargo run -- validate-output --require-status ready_candidate forge.recipe.yaml
```
