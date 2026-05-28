# Registry Forge Demo Fixtures

All fixtures are synthetic and safe for local demos.

- `demo`: baseline farmer registry happy path.
- `demo-households-csv`: clean household registry happy path with a separate
  semantic profile bundle.
- `demo-messy-csv`: messy farmer registry source with duplicate headers, a
  blank header, uneven rows, missing values, duplicate identifiers, and
  sensitive names that must be redacted.

Generated reports, patches, previews, and packages are intentionally not stored
in these fixture directories. Run the CLI commands to regenerate them.

For a full walkthrough of the baseline fixture, see
[`../TUTORIAL.md`](../TUTORIAL.md).

## Quick Demo Commands

From `registry-forge/`:

```sh
cargo run -- check-recipe fixtures/demo/forge.recipe.yaml
cargo run -- inspect-source fixtures/demo/forge.recipe.yaml
cargo run -- profile-source fixtures/demo/forge.recipe.yaml
cargo run -- suggest-alignments fixtures/demo/forge.recipe.yaml
cargo run -- preview-transform fixtures/demo/forge.recipe.yaml
cargo run -- validate-output --require-status ready_candidate fixtures/demo/forge.recipe.yaml
cargo run -- export-package fixtures/demo/forge.recipe.yaml --out target/forge-demo-package
```

Swap `fixtures/demo/forge.recipe.yaml` for
`fixtures/demo-households-csv/forge.recipe.yaml` or
`fixtures/demo-messy-csv/forge.recipe.yaml` to run the other demos.
