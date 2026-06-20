#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if rg -n \
  "registry\\.validation\\.report\\.v1|checks\\[\\]\\.(product_report|findings)|auth\\.oidc\\.(jwks_uri|allowed_typ|leeway_seconds)" \
  "$ROOT/README.md" \
  "$ROOT/TUTORIAL.md" \
  "$ROOT/agent-skills" \
  "$ROOT/fixtures" \
  "$ROOT/src" \
  "$ROOT/tests"; then
  echo "stale config vocabulary found" >&2
  exit 1
fi
