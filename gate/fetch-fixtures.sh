#!/usr/bin/env bash
# Fetches the gate's public development set into fixtures/ (gitignored, never committed):
#
#   fixtures/pubsmith/   FuncularLabs/pubsmith's tests/fixtures/pub at the pin below: 142 .pub files made
#                        with Publisher by that project, each with Publisher's layout export in truth/.
#
# pubsmith is MIT, but its THIRD-PARTY-NOTICES.md says these files carry Microsoft material outside that
# licence, so we fetch them for local testing and never vendor them. Their truth JSON scores imports
# only; it is never used to work out the file format.
#
# Then: cargo xtask gate --set pubsmith --names fixtures/pubsmith
set -euo pipefail

PUBSMITH_REPO=https://github.com/FuncularLabs/pubsmith.git
PUBSMITH_COMMIT=0ccdf27fc3f381f0897515e226ea485165c699b2

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
CLONE="$ROOT/fixtures/.pubsmith-src"
DEST="$ROOT/fixtures/pubsmith"

if [[ ! -d "$CLONE/.git" ]]; then
  echo "==> cloning pubsmith (fixtures only)"
  mkdir -p "$ROOT/fixtures"
  git clone --quiet --filter=blob:none --no-checkout --sparse "$PUBSMITH_REPO" "$CLONE"
  git -C "$CLONE" sparse-checkout set tests/fixtures/pub
fi
git -C "$CLONE" cat-file -e "$PUBSMITH_COMMIT^{commit}" 2>/dev/null || git -C "$CLONE" fetch --quiet origin "$PUBSMITH_COMMIT"
git -C "$CLONE" -c advice.detachedHead=false checkout --quiet --force "$PUBSMITH_COMMIT"
actual="$(git -C "$CLONE" rev-parse HEAD)"
[[ "$actual" == "$PUBSMITH_COMMIT" ]] || { echo "!! pubsmith is at $actual, expected $PUBSMITH_COMMIT" >&2; exit 1; }

ln -sfn "$CLONE/tests/fixtures/pub" "$DEST"
echo "==> $(find -L "$DEST" -maxdepth 1 -name '*.pub' | wc -l | tr -d ' ') .pub files, $(find -L "$DEST/truth" -name '*.json' | wc -l | tr -d ' ') truth exports in $DEST"
