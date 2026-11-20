#!/usr/bin/env bash
# Copies DesignCraft's [profile.*] and [patch.crates-io] sections (everything from its first
# [profile line to the end of its Cargo.toml) into our root Cargo.toml, between the markers
# "# >>> synced from upstream" and "# <<< synced from upstream". Cargo only honours patches and
# profiles from the root workspace, so ours must match upstream's exactly.
#
#   fork/sync-workspace.sh           rewrite the block
#   fork/sync-workspace.sh --check   exit 1 if the block is out of date (CI)
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
UP="$ROOT/upstream/designcraft/Cargo.toml"
OURS="$ROOT/Cargo.toml"
[[ -f "$UP" ]] || { echo "!! $UP missing; run fork/fetch.sh" >&2; exit 1; }

python3 -I - "$UP" "$OURS" "${1:-}" <<'PY'
import sys, re
up, ours, mode = sys.argv[1], sys.argv[2], sys.argv[3]
src = open(up).read()
i = re.search(r'(?m)^\[profile', src)
if not i:
    sys.exit("!! no [profile section in upstream Cargo.toml; check its layout")
block = src[i.start():].rstrip() + "\n"
begin, end = "# >>> synced from upstream (fork/sync-workspace.sh; do not edit)\n", "# <<< synced from upstream\n"
text = open(ours).read()
if begin not in text or end not in text:
    sys.exit("!! markers missing in Cargo.toml")
head, rest = text.split(begin, 1)
_, tail = rest.split(end, 1)
new = head + begin + block + end + tail
if mode == "--check":
    if new != text:
        sys.exit("!! Cargo.toml synced block is out of date; run fork/sync-workspace.sh")
    print("==> Cargo.toml synced block is current")
elif new != text:
    open(ours, "w").write(new)
    print("==> Cargo.toml synced block updated")
else:
    print("==> Cargo.toml synced block already current")
PY
