#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PATTERN='registry\.validation\.report\.v1|checks\[\]\.(product_report|findings)|auth\.oidc\.(jwks_uri|allowed_typ|leeway_seconds)'

cd "$ROOT"

status=0
git grep -nE "$PATTERN" -- \
  README.md \
  TUTORIAL.md \
  agent-skills \
  fixtures \
  src \
  tests || status=$?

if [[ "$status" -eq 0 ]]; then
  echo "stale config vocabulary found" >&2
  exit 1
elif [[ "$status" -ne 1 ]]; then
  echo "stale config vocabulary search failed with exit code $status" >&2
  exit "$status"
fi
