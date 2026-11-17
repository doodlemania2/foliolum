#!/usr/bin/env bash
# Puts DesignCraft at the commit in fork/pins.env into upstream/designcraft, with fork/patches applied.
# Our Cargo workspace builds against that checkout by path. See docs/architecture.md.
#
#   fork/fetch.sh            fetch (or reuse) the checkout, reset it to the pin, apply the patches
#   fork/fetch.sh --check    also fail if the root Cargo.toml's synced block is out of date
#
# Every input is pinned: the commit is checked after checkout, and each patch is checked against
# fork/patches/SHA256SUMS before anything is applied. Local edits in upstream/designcraft are
# discarded, so make patch work there deliberately (see fork/README.md).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FORK="$ROOT/fork"
DEST="$ROOT/upstream/designcraft"
# shellcheck source=fork/pins.env
source "$FORK/pins.env"
: "${UPSTREAM_REPO:?}" "${UPSTREAM_COMMIT:?}"

if [[ ! -d "$DEST/.git" ]]; then
  echo "==> cloning $UPSTREAM_REPO"
  mkdir -p "$(dirname "$DEST")"
  git clone --quiet --filter=blob:none --no-checkout "$UPSTREAM_REPO" "$DEST"
fi

if ! git -C "$DEST" cat-file -e "$UPSTREAM_COMMIT^{commit}" 2>/dev/null; then
  echo "==> fetching ${UPSTREAM_REF:-main}"
  git -C "$DEST" fetch --quiet origin "${UPSTREAM_REF:-main}"
  git -C "$DEST" cat-file -e "$UPSTREAM_COMMIT^{commit}" 2>/dev/null \
    || git -C "$DEST" fetch --quiet origin "$UPSTREAM_COMMIT"
fi

echo "==> resetting to $UPSTREAM_COMMIT"
git -C "$DEST" -c advice.detachedHead=false checkout --quiet --force "$UPSTREAM_COMMIT"
git -C "$DEST" clean --quiet -fdx -e target
actual="$(git -C "$DEST" rev-parse HEAD)"
if [[ "$actual" != "$UPSTREAM_COMMIT" ]]; then
  echo "!! checkout is at $actual, expected $UPSTREAM_COMMIT. Refusing." >&2
  exit 1
fi

shopt -s nullglob
patches=("$FORK"/patches/*.patch)
if (( ${#patches[@]} )); then
  echo "==> verifying and applying ${#patches[@]} patch(es)"
  if command -v sha256sum >/dev/null; then sum=(sha256sum); else sum=(shasum -a 256); fi
  (cd "$FORK/patches" && "${sum[@]}" --check --strict SHA256SUMS)
  for p in "${patches[@]}"; do
    echo "    $(basename "$p")"
    git -C "$DEST" apply --check "$p"
    git -C "$DEST" apply "$p"
  done
else
  echo "==> no patches"
fi

"$FORK/sync-workspace.sh" ${1:+"$1"}
"$ROOT/crates/foliolum-fonts/fetch.sh"
echo "==> ready: $DEST"
