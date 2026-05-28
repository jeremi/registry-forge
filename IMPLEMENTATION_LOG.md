# Registry Forge MVP Implementation Log

## Verification Definition Used

The MVP is treated as complete only when:

- the CLI builds and exposes every command in `registry-forge-mvp-cli-spec.md`;
- integration tests cover the readiness blockers and export/replay flow;
- `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`,
  and `cargo test` pass;
- the full demo command flow reaches `ready_candidate`;
- exported packages contain no raw source files and redacted previews do not
  contain synthetic names from the source fixture.

## Pitfalls And Surprises

- `clap` parses `ValueEnum` variants as kebab-case by default. The spec uses
  `ready_candidate`, so `ReadinessStatus` is explicitly configured with
  snake_case values.
- The first implementation of `check-recipe` emitted a schema but only relied
  on deserialization plus custom checks. It now validates raw YAML against the
  generated JSON Schema before applying domain checks.
- `ready_with_warnings` exits `0` by design, but final demo verification uses
  `--require-status ready_candidate` so warnings cannot pass the final gate.
- Exported package replay needs `--source-override` because raw source files are
  intentionally excluded from packages.
- Generated reports, patches, and previews are not committed in the demo
  fixture. Tests copy the fixture into a temporary directory before generating
  those artifacts.
- The XLSX fixture is synthetic and committed. It is intentionally minimal so
  `calamine` can read it without pulling in a generator at runtime.
- First independent review found false-ready paths around source overrides,
  mapping freshness, required mapping fields, code-list readiness, stale or
  missing profile reports, unknown recipe sections, and empty source hashes.
  Those are now covered by integration tests.
- Second independent review found three more false-ready paths: transform
  diagnostics errors were not readiness blockers, fields could declare a code
  list without a matching accepted crosswalk, and stale profile reports could
  preserve old redaction state. Those are now covered by integration tests.
- CSV parsing originally failed on uneven row lengths before a report could be
  produced. The reader now runs in flexible mode and records
  `inconsistent_row_length` as an inspection/profile warning.
- Workbook diagnostics need to look at XLSX worksheet XML, not only calamine's
  cell grid, because merged cells, hidden rows or columns, and formulas are
  structural workbook features. The CLI now reports those as warnings.
- Multi-sheet XLSX init now records an explicit sheet-selection decision instead
  of silently choosing one. Single-sheet workbooks still auto-select that sheet
  so the basic path stays quick.
- Additional demo fixtures now cover a second clean domain
  (`demo-households-csv`) and a messy CSV source (`demo-messy-csv`). Both are
  synthetic and covered by integration tests.
- The messy CSV fixture exposed a patch-quality issue: blank headers sanitized
  to an empty field id and duplicate headers reused the same stable id. Stable
  field ids are now non-empty and de-duplicated before profiling patches are
  generated.
- The household demo exposed a package export bug where every profile bundle was
  copied as `publicschema-demo.bundle.json`. Export now preserves the source
  bundle file name.
- Exported recipes now point at the packaged mapping and profile-bundle paths,
  instead of preserving project-local paths that are not present in the package.
- Profile bundles are now hash-checked by commands that trust them, so semantic
  suggestions, readiness validation, and package export cannot silently use a
  modified profile bundle.
- Because exported recipes rewrite package-local paths, export also rewrites the
  packaged profile and preview recipe hashes so package replay validation is not
  falsely stale.
- Team review follow-up fixed several readiness and packaging bugs:
  `--require-status not_ready` can now succeed when expected, package manifests
  use a canonical recipe hash plus scalar source hash and `command_versions`,
  draft code-list errors are no longer duplicated, missing profile bundles fail
  during `init`, and legacy `.xls` is explicitly rejected for the MVP.
- Safety hardening from review: source/profile/mapping recipe paths and source
  overrides must be relative without `..`, transform diagnostics no longer
  persist raw Crosswalk error messages, unknown-field profile values default to
  redacted, and YAML atomic writes use `NamedTempFile` instead of deterministic
  `.tmp` paths.
- XLSX formula warnings now target formula cells without cached values instead
  of any XML token beginning with `<f`.
