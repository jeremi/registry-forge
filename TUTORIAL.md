# Registry Forge Tutorial

This tutorial walks through the synthetic farmer registry demo fixture. It is
written for a developer or Codex-style agent operating Registry Forge as a local
CLI workbench.

The goal is to start with a CSV registry source, inspect and profile it, review
semantic alignment suggestions, run a Crosswalk transformation preview, validate
readiness, export a portable package, and prove that the package can be replayed.

All data in `fixtures/demo` is synthetic.

## Prerequisites

Run every command from the Registry Forge project directory:

```sh
cd /Users/jeremi/Projects/204-programs-delivery-commons/apps/registry-forge
```

You need a working Rust toolchain with `cargo` available.

## Fixture Layout

The demo fixture is intentionally small so the full workflow is easy to inspect:

```text
fixtures/demo/
  data/
    farmers.csv
    farmers.xlsx
  forge.recipe.yaml
  mappings/
    crosswalk.mapping.yaml
  profiles/
    publicschema-demo.bundle.json
```

The committed recipe points at `data/farmers.csv`. Generated outputs are written
under the fixture as `reports/`, `patches/`, and `previews/`. Exported packages
should be written under `target/` so they stay out of the fixture.

## Step 1: Read The Source

Open the source data first:

```sh
sed -n '1,20p' fixtures/demo/data/farmers.csv
```

The source has five columns:

- `Farmer ID`
- `Full Name`
- `District`
- `Registration Status`
- `Registration Date`

Then inspect the recipe:

```sh
sed -n '1,220p' fixtures/demo/forge.recipe.yaml
```

Notice the main control points:

- `source.path` identifies the local file.
- `source.hash.value` pins the exact source bytes.
- `profile_bundle.path` points at the local semantic profile bundle.
- `fields` maps raw source column names to stable field ids.
- `semantic_alignments` records reviewed source-to-target alignments.
- `mappings.file` points at the Crosswalk mapping.
- `validation.required_targets` defines the minimum readiness contract.

## Step 2: Check The Recipe

Validate the recipe before reading source data:

```sh
cargo run -- check-recipe fixtures/demo/forge.recipe.yaml
```

This should exit successfully. It checks recipe shape, path safety, required
sections, known reviewer references, and hash formatting. It does not yet prove
that generated reports and previews are fresh.

## Step 3: Inspect The Source Structure

Run source inspection:

```sh
cargo run -- inspect-source fixtures/demo/forge.recipe.yaml
```

This writes:

```text
fixtures/demo/reports/source-inspection.json
```

Use the report to confirm the parser saw the expected file structure:

```sh
sed -n '1,220p' fixtures/demo/reports/source-inspection.json
```

For CSV sources, the report should include the source format, row count, column
names, and parser warnings. For `.xlsx` sources, the same command family also
surfaces workbook diagnostics such as hidden rows, merged cells, and formula
cells without cached values.

## Step 4: Profile The Source Data

Run profiling:

```sh
cargo run -- profile-source fixtures/demo/forge.recipe.yaml
```

This writes two artifacts:

```text
fixtures/demo/reports/source-profile.json
fixtures/demo/patches/source-profile.patch.json
```

Inspect the profile report:

```sh
sed -n '1,260p' fixtures/demo/reports/source-profile.json
```

The report is the first practical evidence layer. It records field-level facts
such as missing values, distinct counts, duplicate counts, type hints, candidate
identifier signals, candidate code-list signals, sensitivity handling, source
hash, and recipe hash.

Sensitive fields are redacted in profile top values unless the recipe explicitly
marks them as safe to expose. In this fixture, `full_name` is high sensitivity,
so top-value samples should not reveal the names from the source file.

## Step 5: Suggest Semantic Alignments

Run deterministic alignment suggestions against the local profile bundle:

```sh
cargo run -- suggest-alignments fixtures/demo/forge.recipe.yaml
```

This writes:

```text
fixtures/demo/reports/alignment-suggestions.json
fixtures/demo/patches/alignment-suggestions.patch.json
```

Inspect the suggestion report:

```sh
sed -n '1,260p' fixtures/demo/reports/alignment-suggestions.json
```

In this committed fixture, the recipe already contains accepted alignments, so
the generated patch should normally be empty. In a new project, the patch would
contain RFC 6902 operations that add `needs_review` alignment records to the
recipe.

The important product behavior is that AI or heuristic suggestions do not become
trusted configuration silently. They land as reviewable recipe changes.

## Step 6: Review The Mapping

Inspect the committed Crosswalk mapping:

```sh
sed -n '1,220p' fixtures/demo/mappings/crosswalk.mapping.yaml
```

The mapping turns stable source fields into canonical output fields:

```yaml
farmer_identifier: "source.farmer_id"
person_name: "source.full_name"
farmer_location_district: "source.district"
registration_status: "source.registration_status"
registration_date: "source.registration_date"
```

The `x-forge.rules` section carries review metadata for each semantic target.
Readiness validation uses this metadata to block false-ready outputs when a
required mapping is still generated, unreviewed, or missing reviewer evidence.

To generate a fresh mapping scaffold from accepted alignments, write it to
`target/`:

```sh
cargo run -- scaffold-mapping fixtures/demo/forge.recipe.yaml --out target/demo-mapping.yaml --force
```

That scaffold should compile, but it is intentionally marked `needs_review`.
Treat it as a starting point, not as production configuration.

## Step 7: Preview The Transformation

Run the transformation preview:

```sh
cargo run -- preview-transform fixtures/demo/forge.recipe.yaml
```

This writes:

```text
fixtures/demo/previews/canonical-samples.redacted.jsonl
fixtures/demo/reports/transform-diagnostics.json
```

Inspect the redacted preview:

```sh
sed -n '1,20p' fixtures/demo/previews/canonical-samples.redacted.jsonl
```

The preview is JSON Lines. Each line contains:

- `_forge.source_row`
- `_forge.recipe_hash`
- `_forge.mapping_hash`
- `_forge.source_hash`
- `record`

The `person_name` output is derived from a high-sensitivity source field, so it
should be redacted in the preview.

Check diagnostics too:

```sh
sed -n '1,220p' fixtures/demo/reports/transform-diagnostics.json
```

For the happy-path demo, diagnostics should not contain errors.

## Step 8: Validate Readiness

Run readiness validation and require the expected status:

```sh
cargo run -- validate-output --require-status ready_candidate fixtures/demo/forge.recipe.yaml
```

This writes:

```text
fixtures/demo/reports/readiness-report.json
```

Inspect the report:

```sh
sed -n '1,220p' fixtures/demo/reports/readiness-report.json
```

For this fixture, the status should be:

```json
"ready_candidate"
```

Readiness validation checks the source hash, profile freshness, preview
freshness, mapping compile status, required target coverage, reviewer metadata,
code-list crosswalk acceptance, and transform diagnostics.

## Step 9: Export A Portable Package

Export the reviewed package:

```sh
cargo run -- export-package fixtures/demo/forge.recipe.yaml --out target/forge-demo-package
```

The package should include reviewed artifacts and candidates, but no raw source
file:

```text
target/forge-demo-package/
  candidates/
  forge.recipe.yaml
  mappings/
  package-manifest.json
  previews/
  profile-bundles/
  reports/
```

Inspect the package manifest:

```sh
sed -n '1,260p' target/forge-demo-package/package-manifest.json
```

The manifest records the packaged recipe hash, source hash, command versions,
and artifact hashes. The exported recipe rewrites mapping and profile-bundle
paths so the package can travel without depending on the original fixture
directory.

Confirm that the raw source was not copied:

```sh
find target/forge-demo-package -type f | sort
```

You should see reports, previews, mappings, profile bundles, candidates, the
recipe, and the package manifest. You should not see `farmers.csv` or
`farmers.xlsx`.

## Step 10: Replay The Package

Validate that the exported package is internally consistent:

```sh
cargo run -- check-recipe target/forge-demo-package/forge.recipe.yaml
```

Replay the packaged recipe against the original source bytes:

```sh
cargo run -- preview-transform \
  --source-override fixtures/demo/data/farmers.csv \
  --out target/replay-canonical-samples.redacted.jsonl \
  target/forge-demo-package/forge.recipe.yaml
```

Validate readiness through the package:

```sh
cargo run -- validate-output \
  --require-status ready_candidate \
  --source-override fixtures/demo/data/farmers.csv \
  target/forge-demo-package/forge.recipe.yaml
```

Compare the packaged preview to the replayed preview:

```sh
cmp target/forge-demo-package/previews/canonical-samples.redacted.jsonl \
  target/replay-canonical-samples.redacted.jsonl
```

If `cmp` exits successfully, the package replay is byte-for-byte equivalent for
the redacted canonical samples.

## Step 11: Try The Other Fixtures

The same workflow works with the other synthetic fixtures:

```sh
cargo run -- check-recipe fixtures/demo-households-csv/forge.recipe.yaml
cargo run -- inspect-source fixtures/demo-households-csv/forge.recipe.yaml
cargo run -- profile-source fixtures/demo-households-csv/forge.recipe.yaml
cargo run -- suggest-alignments fixtures/demo-households-csv/forge.recipe.yaml
cargo run -- preview-transform fixtures/demo-households-csv/forge.recipe.yaml
cargo run -- validate-output --require-status ready_candidate fixtures/demo-households-csv/forge.recipe.yaml
```

For messy-source behavior:

```sh
cargo run -- check-recipe fixtures/demo-messy-csv/forge.recipe.yaml
cargo run -- inspect-source fixtures/demo-messy-csv/forge.recipe.yaml
cargo run -- profile-source fixtures/demo-messy-csv/forge.recipe.yaml
cargo run -- suggest-alignments fixtures/demo-messy-csv/forge.recipe.yaml
cargo run -- preview-transform fixtures/demo-messy-csv/forge.recipe.yaml
cargo run -- validate-output --require-status ready_candidate fixtures/demo-messy-csv/forge.recipe.yaml
```

The messy fixture is useful for checking duplicate headers, blank headers,
uneven rows, missing values, duplicate identifiers, and redaction of sensitive
names.

## Agent Operating Pattern

When using Registry Forge through Codex or another local agent, keep the loop
explicit:

1. Inspect committed source, recipe, profile bundle, and mapping.
2. Run `check-recipe`.
3. Run `inspect-source` and read `reports/source-inspection.json`.
4. Run `profile-source` and read `reports/source-profile.json`.
5. Run `suggest-alignments` and review the report and patch.
6. Apply only reviewed recipe patches.
7. Review or edit Crosswalk mappings.
8. Run `preview-transform` and inspect preview plus diagnostics.
9. Run `validate-output --require-status ready_candidate`.
10. Export and replay the package before presenting it as verified.

Do not mark a preparation package ready just because a transformation produced
rows. Ready means the report status is `ready_candidate`, the package excludes
raw source data, and replay against the original source bytes produces the same
redacted canonical samples.
