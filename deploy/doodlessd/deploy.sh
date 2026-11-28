#!/usr/bin/env bash
# Run by CI through the deploy gate (`edge-ssh run foliolum-app-stack deploy.sh <app ref> <api ref>`).
# By hand, from this directory:
#   deploy.sh <app ref> <api ref> [<backup ref>]
#     for example doodlemanage.tail890bc.ts.net:5000/foliolum-app:sha-<commit>-r<run>-1
#             and doodlemanage.tail890bc.ts.net:5000/foliolum-api:sha-<commit>-r<run>-1
#
# The database's settings come from wire-backend.sh, once; this script only moves the image tags.
# If the new images fail their health checks or the smoke, the previous ones go back. The database
# is never rolled back or recreated here: its schema only ever grows (the api's migrations).
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"

app="${1:?usage: deploy.sh <registry/foliolum-app:sha-...> <registry/foliolum-api:sha-...>}"
api="${2:?usage: deploy.sh <registry/foliolum-app:sha-...> <registry/foliolum-api:sha-...>}"
# The backup image only changes its tag; it runs once backup.env and COMPOSE_PROFILES=backup are set.
backup="${3:-}"

setvar() {
  if grep -q "^$1=" .env 2>/dev/null; then sed -i "s|^$1=.*|$1=$2|" .env; else echo "$1=$2" >> .env; fi
}

healthy() {
  local c
  for _ in $(seq 1 90); do
    local all=1
    for c in foliolum-db foliolum-api foliolum-app; do
      [ "$(docker inspect -f '{{.State.Health.Status}}' "$c" 2>/dev/null)" = healthy ] || all=0
    done
    [ "$all" = 1 ] && return 0
    sleep 2
  done
  return 1
}

roll() {
  exec 9>.deploy.lock
  flock -w 600 9 || { echo "another deploy held .deploy.lock for 10 minutes" >&2; exit 1; }
  touch .env
  setvar FOLIOLUM_APP_IMAGE "$1"
  setvar FOLIOLUM_API_IMAGE "$2"
  if [ -n "$backup" ]; then setvar FOLIOLUM_BACKUP_IMAGE "$backup"; fi
  docker compose pull foliolum-app foliolum-api foliolum-db
  if [ -n "$backup" ]; then docker compose --profile backup pull foliolum-backup; fi
  docker compose up -d
  flock -u 9
}

previous_app="$(sed -n 's/^FOLIOLUM_APP_IMAGE=//p' .env 2>/dev/null || true)"
previous_api="$(sed -n 's/^FOLIOLUM_API_IMAGE=//p' .env 2>/dev/null || true)"
roll "$app" "$api"

if healthy && ./smoke.sh foliolum-app with-api; then
  echo "foliolum is healthy and serves $app with $api"
  exit 0
fi

echo "foliolum failed with $app and $api" >&2
docker compose ps >&2
docker compose logs --tail=60 foliolum-app foliolum-api >&2
if [ -n "$previous_app" ] && [ -n "$previous_api" ] && { [ "$previous_app" != "$app" ] || [ "$previous_api" != "$api" ]; }; then
  echo "rolling back to $previous_app and $previous_api" >&2
  roll "$previous_app" "$previous_api"
  healthy || echo "the previous images are not healthy either" >&2
fi
exit 1
