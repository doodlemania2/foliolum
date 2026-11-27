#!/usr/bin/env bash
# Starts the backend image the way compose runs it (read-only, no capabilities, no database) and
# checks that it serves /api/health, refuses a caller who isn't signed in, and says it isn't ready
# without its database. Needs docker only.
#
#   deploy/smoke-api-image.sh <image>
set -euo pipefail
image="${1:?usage: smoke-api-image.sh <image>}"
name="foliolum-api-smoke-$$"
trap 'docker rm -f "$name" >/dev/null 2>&1 || true' EXIT
docker run -d --name "$name" --read-only --cap-drop ALL --security-opt no-new-privileges:true \
  -e FOLIOLUM_DB_HOST=nowhere.invalid -e FOLIOLUM_DB=foliolum \
  -e FOLIOLUM_APP_ROLE=foliolum_app -e FOLIOLUM_APP_PASSWORD=smoke-only-not-a-secret \
  -e FOLIOLUM_OWNER_ROLE=foliolum_owner -e FOLIOLUM_OWNER_PASSWORD=smoke-only-not-a-secret \
  "$image" >/dev/null
# wget exits non-zero for any 4xx or 5xx, which is what some checks expect; read the status line.
code() { { docker exec "$name" wget -q -S -O /dev/null "http://127.0.0.1:8081$1" 2>&1 || true; } | sed -n 's/^ *HTTP\/[0-9.]* \([0-9]*\).*/\1/p' | tail -n1; }
for _ in $(seq 1 30); do [ "$(code /api/health)" = 200 ] && break; sleep 1; done
fails=0
check() { local got; got="$(code "$1")"; [ "$got" = "$2" ] || { echo "FAIL: $1: $got, wanted $2" >&2; fails=$((fails + 1)); }; }
check /api/health 200
check /api/me 401
check /api/ready 503
check /api/nope 404
if [ "$fails" -gt 0 ]; then
  docker logs "$name" >&2 || true
  exit 1
fi
echo "smoke-api-image: ok"
