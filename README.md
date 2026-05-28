# Registry Forge

Registry Forge is a local Rust CLI for preparing synthetic or non-real registry
source data into a replayable preparation package.

It is the MVP implementation of the Registry Forge Engine spec. There is no UI
yet. The expected operator is Codex or a developer running commands, reading
reports, applying JSON Patch recipe changes, and editing Crosswalk mappings.

For a guided walkthrough, see [TUTORIAL.md](TUTORIAL.md).

For agent-facing operating instructions, see
[`agent-skills/registry-forge-operator/SKILL.md`](agent-skills/registry-forge-operator/SKILL.md).

## What It Does

- Reads local CSV and `.xlsx` sources without modifying them.
- Records source and profile-bundle hashes in `forge.recipe.yaml`.
- Inspects source structure and parser warnings.
- Profiles fields, missingness, distinct counts, duplicate values, top values,
  type hints, candidate identifiers, and candidate code lists.
- Suggests deterministic semantic alignments from a pinned local profile bundle.
- Applies RFC 6902 JSON Patch operations to recipes.
- Generates review-needed Crosswalk mapping scaffolds from accepted alignments.
- Runs Crosswalk previews and writes redacted canonical JSONL samples.
- Validates readiness and blocks false-ready states.
- Exports a portable package without raw source files.

## Supported Inputs

- CSV files.
- `.xlsx` workbooks via `calamine`.

Legacy binary `.xls` is intentionally rejected in the MVP. Convert those files
to `.xlsx` before import.

Recipe paths for the source, mapping, and profile bundle must be relative and
must not contain `..`. `--source-override` follows the same rule.

## Demo Fixtures

All demo data is synthetic.

- `fixtures/demo`: baseline farmer registry happy path.
- `fixtures/demo-households-csv`: clean household registry happy path with a
  separate semantic profile bundle.
- `fixtures/demo-messy-csv`: messy farmer CSV with duplicate headers, blank
  header, uneven rows, missing values, duplicate IDs, and sensitive names.

Generated `reports/`, `patches/`, and `previews/` directories are not committed.

## Happy Path

```sh
cargo run -- check-recipe fixtures/demo/forge.recipe.yaml
cargo run -- inspect-source fixtures/demo/forge.recipe.yaml
cargo run -- profile-source fixtures/demo/forge.recipe.yaml
cargo run -- suggest-alignments fixtures/demo/forge.recipe.yaml
cargo run -- preview-transform fixtures/demo/forge.recipe.yaml
cargo run -- validate-output --require-status ready_candidate fixtures/demo/forge.recipe.yaml
cargo run -- export-package fixtures/demo/forge.recipe.yaml --out target/forge-demo-package
```

Replay the exported package against the original source bytes:

```sh
cargo run -- check-recipe target/forge-demo-package/forge.recipe.yaml
cargo run -- preview-transform \
  --source-override fixtures/demo/data/farmers.csv \
  --out target/replay-canonical-samples.redacted.jsonl \
  target/forge-demo-package/forge.recipe.yaml
cargo run -- validate-output \
  --require-status ready_candidate \
  --source-override fixtures/demo/data/farmers.csv \
  target/forge-demo-package/forge.recipe.yaml
cmp target/forge-demo-package/previews/canonical-samples.redacted.jsonl \
  target/replay-canonical-samples.redacted.jsonl
```

## Verification

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The integration suite covers readiness blockers, path traversal, source hash
checks, package replay, source immutability, deterministic suggestions, mapping
compile failures, and redaction/package leak checks.

## Known MVP Gaps

- Composite identifier detection is not implemented yet.
- XLSX formula diagnostics are pragmatic XML checks, not a full workbook model.
- Workbook XML inspection reopens the verified file, so a narrow TOCTOU window
  remains.
- The implementation is a single crate for the MVP. Split into core, ingest,
  profile, transform, export, and CLI crates when the surface grows.
