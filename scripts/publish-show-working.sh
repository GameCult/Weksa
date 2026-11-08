#!/usr/bin/env bash
# Build the "Show her working" page from the real lowerer and scorer, then publish it as a Mind preview.
# Run from the repository root, in a Mind's Self run (it writes only /srv/minds-web/<mind>/).
#
#   scripts/publish-show-working.sh [--no-publish]
#
# Needs cargo, python3 and an Ollama embedder at 127.0.0.1:11434 (qwen3-embedding:0.6b): the scorer is not stubbed.
# publish-mind-preview.sh lives in the gamecult-ops checkout: set GAMECULT_OPS to it (default ../gamecult-ops).
set -euo pipefail
[ -f Cargo.toml ] && [ -d tools/show-working ] || { echo "run from the Weksa repository root" >&2; exit 2; }
out=${SHOW_WORKING_OUT:-tools/show-working/out}
rm -rf "$out"
python3 -I tools/show-working/build.py "$out"
[ "${1:-}" = --no-publish ] && { echo "built $out"; exit 0; }
ops=${GAMECULT_OPS:-../gamecult-ops}
exec "$ops/scripts/publish-mind-preview.sh" "$out" weksa-show-working
