#!/usr/bin/env bash
# Checks a RUNNING foliolum-app container the way the edge reaches it. Used by deploy.sh after every
# rollout and by deploy/smoke-image.sh before an image is pushed.
#
#   smoke.sh <container name>
#
# curl runs in its own small container that shares the app container's network, so this works from a
# runner that is itself a container and cannot reach a published port.
set -euo pipefail
name="${1:?usage: smoke.sh <container name>}"

CURL_IMAGE="curlimages/curl:8.22.0"
BASE="http://127.0.0.1:8080"
HOST="app.foliolum.app"

c() { docker run --rm --network "container:${name}" "$CURL_IMAGE" -sS --max-time 20 -H "Host: $HOST" "$@"; }

# get <path> [curl args]  ->  sets $status, $headers, $body
get() {
  local path="$1"; shift
  headers="$(c -o /dev/null -D - "$@" "${BASE}${path}" | tr -d '\r' || true)"
  status="$(printf '%s\n' "$headers" | sed -n 's/^HTTP\/[0-9.]* \([0-9]*\).*/\1/p' | tail -n1)"
}

fails=0
fail() { echo "FAIL: $*" >&2; fails=$((fails + 1)); }
expect_status() { [ "${status:-}" = "$1" ] || fail "$2: status ${status:-none}, wanted $1"; }
expect_header() { grep -qi -- "$1" <<<"$headers" || fail "$2: no header matching $1"; }

for _ in $(seq 1 30); do
  get /healthz
  [ "${status:-}" = 200 ] && break
  sleep 1
done
expect_status 200 /healthz

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT
c "${BASE}/" > "$tmp/page" || true
page="$(cat "$tmp/page")"
get /
expect_status 200 /
expect_header '^cache-control: no-cache, no-transform' /
expect_header "^content-security-policy: default-src 'none'; script-src 'self' 'sha256-" /
expect_header "'wasm-unsafe-eval'" /
expect_header '^x-content-type-options: nosniff' /
grep -qF 'Based on DesignCraft by the ArtCraft team' <<<"$page" || fail "/: no DesignCraft credit"

# The policy's script hash must match the loader the page carries, or the editor never starts.
# The exact bytes between the tags, as the browser hashes them. Hashed inside the app container
# (BusyBox awk, sha256sum, xxd, base64), so the host needs nothing but docker.
# shellcheck disable=SC2016  # awk's own program, expanded by nobody
loader='BEGIN { RS = "\001" }
{ o = "<script type=\"module\">"; i = index($0, o); if (!i) exit 1
  r = substr($0, i + length(o)); printf "%s", substr(r, 1, index(r, "</script>") - 1) }'
# shellcheck disable=SC2016  # $1 and $(...) belong to the container's shell
want="'sha256-$(docker exec -i "$name" sh -c 'awk "$1" | sha256sum | cut -d" " -f1 | xxd -r -p | base64' sh "$loader" < "$tmp/page")'"
grep -qF -- "$want" <<<"$headers" || fail "/: the CSP does not hold the loader's hash $want"

wasm="$(grep -o '[A-Za-z0-9_.-]*_bg\.wasm' <<<"$page" | head -n1)"
if [ -z "$wasm" ]; then
  fail "/: names no .wasm"
else
  get "/$wasm"
  expect_status 200 "/$wasm"
  expect_header '^content-type: application/wasm' "/$wasm"
  expect_header '^cache-control: public, max-age=31536000, immutable' "/$wasm"
  get "/$wasm" -H 'Accept-Encoding: gzip'
  expect_header '^content-encoding: gzip' "/$wasm (gzip)"
fi

# The fonts the editor fetches after it starts: content-named, so cached for good.
font="$(docker exec "$name" sh -c 'ls /usr/share/nginx/html/fonts | grep "\.ttf$" | head -n1')"
if [ -z "$font" ]; then
  fail "no fonts in the image"
else
  get "/fonts/$font" -H 'Accept-Encoding: gzip'
  expect_status 200 "/fonts/$font"
  expect_header '^content-type: font/ttf' "/fonts/$font"
  expect_header '^cache-control: public, max-age=31536000, immutable' "/fonts/$font"
  expect_header '^content-encoding: gzip' "/fonts/$font (gzip)"
fi

# The tab icon: linked from the page, and at the fixed names browsers ask for.
grep -qF 'href="favicon.svg"' <<<"$page" || fail "/: no favicon link"
get /favicon.ico
expect_status 200 /favicon.ico
expect_header '^content-type: image/x-icon' /favicon.ico
expect_header '^cache-control: public, max-age=86400' /favicon.ico
get /favicon.svg
expect_status 200 /favicon.svg
expect_header '^content-type: image/svg+xml' /favicon.svg
expect_header "^content-security-policy: default-src 'none'" /favicon.svg

get /no-such-file.js
expect_status 404 /no-such-file.js
# A missing file is never cached, or it stays missing at Cloudflare after it is added (#29).
expect_header '^cache-control: no-store' /no-such-file.js
grep -qi '^cache-control:.*immutable' <<<"$headers" && fail "/no-such-file.js: a 404 marked immutable"
expect_header "^content-security-policy: default-src 'none'" /no-such-file.js

if [ "$fails" -gt 0 ]; then echo "$fails check(s) failed" >&2; exit 1; fi
echo "smoke ok"
