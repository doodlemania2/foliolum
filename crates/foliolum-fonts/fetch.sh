#!/usr/bin/env bash
# Fetches the fonts in fonts.lock into fonts/ (gitignored) and checks each against its SHA-256.
# fork/fetch.sh runs this; the crate embeds the files, so it won't build without them.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"
DEST="$HERE/fonts"
if command -v sha256sum >/dev/null; then sum=(sha256sum); else sum=(shasum -a 256); fi

fetched=0
while read -r hash name url; do
  [[ -z "$hash" || "$hash" == \#* ]] && continue
  out="$DEST/$name"
  if [[ -f "$out" ]] && [[ "$("${sum[@]}" "$out" | cut -c1-64)" == "$hash" ]]; then continue; fi
  mkdir -p "$(dirname "$out")"
  curl -fsSL --retry 3 "$url" -o "$out.part"
  got="$("${sum[@]}" "$out.part" | cut -c1-64)"
  if [[ "$got" != "$hash" ]]; then
    rm -f "$out.part"
    echo "!! $name: SHA-256 $got, expected $hash. Refusing." >&2
    exit 1
  fi
  mv "$out.part" "$out"
  fetched=$((fetched + 1))
done < "$HERE/fonts.lock"
echo "==> fonts: $fetched fetched, all $(grep -cv '^#' "$HERE/fonts.lock") checked in $DEST"

# Content-named copies for the web app, which fetches them instead of embedding them (see the crate
# docs, `web_name`): the first 16 hex digits of the SHA-256, then the file name with anything but
# letters, digits, ".", "_" and "-" made "_". Licence texts go beside them, named after their family.
WEB="$HERE/web"
rm -rf "$WEB"
mkdir -p "$WEB"
while read -r hash name _; do
  [[ -z "$hash" || "$hash" == \#* ]] && continue
  file="$(basename "$name" | LC_ALL=C sed 's/[^A-Za-z0-9._-]/_/g')"
  case "$name" in
    *.ttf) cp "$DEST/$name" "$WEB/${hash:0:16}-$file" ;;
    *) cp "$DEST/$name" "$WEB/$(dirname "$name")-$file" ;;
  esac
done < "$HERE/fonts.lock"
echo "==> fonts: web copies in $WEB"
