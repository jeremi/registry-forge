# Contributing

Registry Forge is a local Rust CLI for preparing registry-like source files into
reviewable, replayable preparation packages. Contributions should preserve the
local-first, source-immutable, auditable workflow.

## Local Setup

Run:

```sh
cargo build --workspace
```

## Development Checks

Run focused checks while iterating, then the full local gate when practical:

```sh
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Pull Requests

Keep pull requests focused. Include tests or explain why the change is docs,
configuration, or tooling only.

Do not commit secrets, production data, private operator notes, or internal
planning documents. Fixture data must remain synthetic or explicitly approved
for public release.
