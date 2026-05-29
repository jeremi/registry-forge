---
name: registry-forge-operator
description: Use when preparing, reviewing, transforming, validating, or exporting a Registry Forge package from CSV or XLSX registry source data, including creating or updating forge.recipe.yaml, reviewing profile reports, applying JSON Patch recipe changes, editing Crosswalk mappings, and verifying ready_candidate output.
---

# Registry Forge Operator

Use this skill to operate Registry Forge as a local CLI workbench. The output is
a verified preparation package, not a silent AI-generated conversion.

## Bundled References

Use these references as the syntax source of truth:

- `references/forge-recipe-v1.md`: read before creating or editing
  `forge.recipe.yaml`.
- `references/crosswalk-mapping-for-forge.md`: read before creating or editing
  `mappings/crosswalk.mapping.yaml`.
- `references/json-patch-examples.md`: read before generating or applying
  recipe patches.

## Core Rule

The recipe is the source of truth. The source file is evidence. Reports,
patches, previews, and exported packages are generated artifacts.

Edit `forge.recipe.yaml`, reviewed JSON Patch files, and Crosswalk mappings.
Never edit generated reports or previews to make validation pass.

## Guardrails

- Use only synthetic or non-real data unless Jeremi explicitly approves another
  handling mode.
- Keep source files local or inside the approved controlled environment.
- Do not call cloud AI providers unless the project policy explicitly allows it.
- Do not mutate source files.
- Do not print secrets, connection strings, full environment dumps, or sensitive
  raw values.
- Treat AI output as a proposal. Convert it into recipe entries, JSON Patch, or
  Crosswalk mapping rules, then validate with Forge.
- Do not mark a package ready unless `validate-output --require-status
  ready_candidate` passes and package replay is verified.

## Find The CLI

If working inside the `registry-forge` repo, prefer:

```sh
cargo run -- <command>
```

If a `registry-forge` binary is already installed or present on `PATH`, direct
binary calls are fine:

```sh
registry-forge <command>
```

Run commands from the directory that contains the relevant `forge.recipe.yaml`,
or pass explicit paths.

## Standard Workflow

1. Locate inputs.
   - Find `forge.recipe.yaml`, source CSV/XLSX files, profile bundles, and
     mappings.
   - Use `fd 'forge\.recipe\.yaml|.*\.csv|.*\.xlsx|.*\.bundle\.json'` when
     available.

2. Check or initialize the recipe.
   - Read `references/forge-recipe-v1.md` before creating or editing recipe
     syntax.
   - Existing recipe:
     `cargo run -- check-recipe forge.recipe.yaml`
   - New recipe:
     `cargo run -- init --source data/source.csv --profile-bundle profiles/profile.bundle.json --out forge.recipe.yaml`
   - `init` requires an existing profile bundle and refuses to overwrite unless
     `--force` is passed.

3. Inspect the source.
   - Run `cargo run -- inspect-source forge.recipe.yaml`.
   - Review `reports/source-inspection.json`.
   - Confirm format, headers, row counts, parser warnings, worksheet decisions,
     duplicate or blank headers, merged cells, hidden rows/columns, formula
     warnings, and source hash.

4. Profile the source.
   - Run `cargo run -- profile-source forge.recipe.yaml`.
   - Review `reports/source-profile.json`.
   - Review `patches/source-profile.patch.json` before applying anything.
   - Confirm missingness, distinct counts, duplicate values, top values,
     sensitivity handling, type hints, identifier candidates, code-list
     candidates, source hash, and recipe hash.

5. Update field configuration.
   - Read `references/json-patch-examples.md` before creating a patch.
   - Ensure every source field used by mappings has a `fields` entry.
   - Set stable field id, `source_name`, `role`, `sensitivity`, `type_hint`, and
     `code_list` where needed.
   - Prefer JSON Patch for generated or bulk edits:
     `cargo run -- apply-patch forge.recipe.yaml --patch patches/source-profile.patch.json --out forge.recipe.yaml`
   - Direct YAML edits are acceptable for small, reviewed changes.
   - Always rerun `check-recipe` after recipe edits.

6. Suggest and review semantic alignments.
   - Run `cargo run -- suggest-alignments forge.recipe.yaml`.
   - Review `reports/alignment-suggestions.json` and
     `patches/alignment-suggestions.patch.json`.
   - Suggestions are not trusted until a reviewer accepts them in the recipe.
   - Accepted alignments need `source_field`, `target`, `match_level`, `status:
     accepted`, `confidence`, and `reviewer`.

7. Review code-list crosswalks.
   - If a field declares `code_list`, the recipe must define a matching
     `value_crosswalks` entry.
   - For MVP readiness, crosswalk status must be `accepted` and the source field
     must match the field declaring the code list.
   - Current MVP validation does not enumerate every observed source value, so
     manually inspect profile top values when code-list coverage matters.

8. Scaffold or edit Crosswalk mappings.
   - Read `references/crosswalk-mapping-for-forge.md` before editing mapping
     syntax.
   - To create a scaffold:
     `cargo run -- scaffold-mapping forge.recipe.yaml --out mappings/crosswalk.mapping.yaml`
   - Use `--force` only when intentionally replacing the existing mapping.
   - The scaffold compiles but marks rules as `needs_review`.
   - Review or edit `mappings/crosswalk.mapping.yaml`.
   - Required mapping metadata lives under `x-forge.rules`.
   - Required targets must have canonical fields and reviewed metadata.

9. Preview transformation.
   - Run `cargo run -- preview-transform forge.recipe.yaml`.
   - Review `previews/canonical-samples.redacted.jsonl`.
   - Review `reports/transform-diagnostics.json`.
   - Sensitive output fields should be redacted in previews.
   - Fix compile or runtime diagnostics in the mapping, then rerun preview.

10. Validate readiness.
    - Readiness expects current generated artifacts. In a fresh working
      directory, run `inspect-source`, `profile-source`, and `preview-transform`
      before requiring `ready_candidate`.
    - Run `cargo run -- validate-output --require-status ready_candidate forge.recipe.yaml`.
    - Review `reports/readiness-report.json`.
    - If status is `not_ready`, fix the underlying recipe, source, profile, or
      mapping issue. Do not edit the readiness report.

11. Export package.
    - Run `cargo run -- export-package forge.recipe.yaml --out target/forge-package`.
    - Inspect `target/forge-package/package-manifest.json`.
    - Confirm raw source files are absent from the package.
    - Candidate Relay, Manifest, and Notary outputs are draft artifacts.

12. Replay package.
    - Run `cargo run -- check-recipe target/forge-package/forge.recipe.yaml`.
    - Replay against the original source bytes:
      `cargo run -- preview-transform --source-override data/source.csv --out target/replay-canonical-samples.redacted.jsonl target/forge-package/forge.recipe.yaml`
    - Validate through the package:
      `cargo run -- validate-output --require-status ready_candidate --source-override data/source.csv target/forge-package/forge.recipe.yaml`
    - Compare packaged and replayed previews with `cmp`.

## Recipe Checklist

Before export, confirm:

- `version` is `forge.recipe.v1`.
- `project.name` is set.
- Reviewers have stable `id` values.
- Source path, format, parser, and hash are set.
- Source, mapping, and profile-bundle paths are relative and do not contain
  `..`.
- Profile bundle path and hash are set.
- Field ids are stable and match mapping source references.
- Sensitive fields are classified.
- Required semantic alignments are accepted and reviewed.
- Code-list fields have accepted value crosswalks.
- Crosswalk mapping compiles.
- Required targets have canonical fields and reviewed mapping metadata.
- Generated profile and preview hashes match the current recipe, mapping, and
  source.
- `validate-output --require-status ready_candidate` passes.

## Failure Handling

- Recipe error: fix the reported recipe path, then rerun `check-recipe`.
- Source hash mismatch: stop and confirm whether the source intentionally
  changed. Do not update the hash just to pass.
- Parser warning: inspect the source and record the intended recipe decision.
- Mapping compile error: fix the Crosswalk mapping and rerun preview.
- Transform diagnostics: inspect the diagnostic report, fix mapping logic, and
  rerun preview.
- Sensitive value leak: stop export review, fix sensitivity or redaction rules,
  regenerate previews, and rerun validation.
- `not_ready`: read `reports/readiness-report.json`, fix each listed error,
  and rerun validation.

## Final Report

When finished, report:

- source file and recipe used;
- commands run and whether they passed;
- readiness status;
- package path;
- package replay result;
- important warnings or residual risks;
- whether raw source data was excluded from export;
- whether candidate artifacts remain draft.

Keep the report concise, but include any warning that affects trust in the
demo.
