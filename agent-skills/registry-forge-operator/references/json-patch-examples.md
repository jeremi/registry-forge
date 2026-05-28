# JSON Patch Examples For Forge

Read this before generating or applying recipe patches.

Forge uses RFC 6902 JSON Patch for structured recipe edits. Patches are JSON
arrays. Each operation has an `op`, a `path`, and usually a `value`.

Apply patches with:

```sh
cargo run -- apply-patch forge.recipe.yaml --patch patch.json --out forge.recipe.yaml
```

`apply-patch` validates the original recipe, applies the patch, validates the
updated recipe, and writes YAML atomically.

## Add A Field

```json
[
  {
    "op": "add",
    "path": "/fields/farmer_id",
    "value": {
      "source_name": "Farmer ID",
      "role": "identifier",
      "sensitivity": "low",
      "type_hint": "string"
    }
  }
]
```

## Add A Sensitive Field

```json
[
  {
    "op": "add",
    "path": "/fields/full_name",
    "value": {
      "source_name": "Full Name",
      "role": "attribute",
      "sensitivity": "high",
      "type_hint": "string"
    }
  }
]
```

## Add A Code-List Field

```json
[
  {
    "op": "add",
    "path": "/fields/district",
    "value": {
      "source_name": "District",
      "role": "attribute",
      "sensitivity": "low",
      "type_hint": "string",
      "code_list": "district_codes"
    }
  }
]
```

## Add A Value Crosswalk

```json
[
  {
    "op": "add",
    "path": "/value_crosswalks/district_codes",
    "value": {
      "source_field": "district",
      "target_code_list": "publicschema:district",
      "status": "accepted"
    }
  }
]
```

## Append A Semantic Alignment

Use `/-` to append to the alignment array:

```json
[
  {
    "op": "add",
    "path": "/semantic_alignments/-",
    "value": {
      "source_field": "farmer_id",
      "target": "publicschema:farmer.identifier",
      "match_level": "exact",
      "status": "needs_review",
      "confidence": "high"
    }
  }
]
```

After human or policy review, replace status and reviewer:

```json
[
  {
    "op": "replace",
    "path": "/semantic_alignments/0/status",
    "value": "accepted"
  },
  {
    "op": "add",
    "path": "/semantic_alignments/0/reviewer",
    "value": "demo-reviewer"
  }
]
```

## Add Required Targets

```json
[
  {
    "op": "add",
    "path": "/validation/required_targets/-",
    "value": "publicschema:farmer.identifier"
  },
  {
    "op": "add",
    "path": "/validation/required_targets/-",
    "value": "publicschema:farmer.location.district"
  }
]
```

## Replace Source Hash After Intentional Source Change

Only do this after confirming the source file intentionally changed and after
rerunning inspection and profiling.

```json
[
  {
    "op": "replace",
    "path": "/source/hash/value",
    "value": "64_hex_sha256_digest"
  }
]
```

## Safe Patch Workflow

1. Read the current recipe.
2. Generate the smallest patch that expresses the intended change.
3. Apply the patch to a temporary output first when the change is non-trivial:

   ```sh
   cargo run -- apply-patch forge.recipe.yaml --patch patch.json --out target/forge.recipe.preview.yaml
   cargo run -- check-recipe target/forge.recipe.preview.yaml
   ```

4. Review the YAML diff.
5. Apply to the real recipe only when the preview is correct:

   ```sh
   cargo run -- apply-patch forge.recipe.yaml --patch patch.json --out forge.recipe.yaml
   cargo run -- check-recipe forge.recipe.yaml
   ```

6. Regenerate affected reports or previews.

## Patch Guardrails

- Do not patch generated reports, previews, or package manifests.
- Do not patch source hashes just to bypass a mismatch.
- Do not accept semantic alignments without a reviewer.
- Do not add unknown keys. Recipe deserialization rejects them.
- Avoid broad `replace` operations on whole sections when targeted operations
  are possible.
- Keep generated patch files committed only when they are useful review
  artifacts. Otherwise regenerate them from CLI commands.
