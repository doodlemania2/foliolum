#!/usr/bin/env bash
# Makes the image's build context from a trunk build:
#
#   deploy/image/prepare.sh <dist/web> <out dir>
#
# - copies the web build to <out>/html, with a gzip copy of every file over 1 KB (nginx gzip_static);
# - writes <out>/headers.inc, whose Content-Security-Policy allows the page's inline loader script
#   and inline style by their sha256 and nothing else inline;
# - copies the Dockerfile and site.conf beside them.
#
# The page holds exactly one inline <script> (trunk's loader) and one inline <style>. If that ever
# changes this script stops rather than write a policy that blocks the editor or allows too much.
set -euo pipefail
src="${1:?usage: prepare.sh <dist/web> <out dir>}"
out="${2:?usage: prepare.sh <dist/web> <out dir>}"
here="$(cd "$(dirname "$0")" && pwd)"

[ -f "$src/index.html" ] || { echo "no index.html in $src" >&2; exit 1; }
ls "$src"/*.wasm >/dev/null 2>&1 || { echo "no .wasm in $src" >&2; exit 1; }

rm -rf "$out"
mkdir -p "$out/html"
# Everything but trunk's lock file.
(cd "$src" && find . -type f ! -name '.trunk-lock' -print0 | tar --null -T - -cf -) | tar -C "$out/html" -xf -

# sha256 of each inline <script> and <style> body, as the browser hashes it.
hashes="$(node "$here/csp-hashes.js" "$out/html/index.html")"
read -r script_hash style_hash <<<"$hashes"

# The policy. 'wasm-unsafe-eval' lets the page compile its own .wasm; it does not allow eval().
# No other origin: fonts are inside the wasm, documents are opened from the visitor's disk.
csp="default-src 'none'; script-src 'self' ${script_hash} 'wasm-unsafe-eval'; style-src 'self' ${style_hash}; connect-src 'self'; img-src 'self' blob: data:; worker-src 'self' blob:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'"

cat > "$out/headers.inc" <<EOF
# Written by deploy/image/prepare.sh. nginx does not inherit add_header into a location that sets its
# own, so every location includes this file again. Do not loosen the policy to make a change pass.
add_header Content-Security-Policy "${csp}" always;
add_header Strict-Transport-Security "max-age=31536000; includeSubDomains" always;
add_header X-Content-Type-Options "nosniff" always;
add_header Referrer-Policy "strict-origin-when-cross-origin" always;
add_header X-Frame-Options "DENY" always;
add_header Permissions-Policy "camera=(), microphone=(), geolocation=()" always;
EOF

# gzip copies, keeping the originals; -n leaves the name and time out, so the bytes depend only on
# the input.
find "$out/html" -type f \( -name '*.wasm' -o -name '*.js' -o -name '*.html' \) -size +1k -exec gzip -9 -n -k {} +

cp "$here/Dockerfile" "$here/site.conf" "$out/"
echo "context in $out: $(du -sh "$out/html" | cut -f1); CSP script ${script_hash} style ${style_hash}"
