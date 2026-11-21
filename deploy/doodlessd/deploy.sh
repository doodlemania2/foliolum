#!/usr/bin/env bash
# Run by CI through the deploy gate (`edge-ssh run foliolum-app-stack deploy.sh <image ref>`).
# By hand, from this directory:
#   deploy.sh <image ref>      for example doodlemanage.tail890bc.ts.net:5000/foliolum-app:sha-<commit>
#
# If the new image fails its health check or smoke, the previous image goes back.
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"

image="${1:?usage: deploy.sh <registry/foliolum-app:sha-...>}"

setvar() {
  if grep -q "^$1=" .env 2>/dev/null; then sed -i "s|^$1=.*|$1=$2|" .env; else echo "$1=$2" >> .env; fi
}

healthy() {
  for _ in $(seq 1 90); do
    if [ "$(docker inspect -f '{{.State.Health.Status}}' foliolum-app 2>/dev/null)" = healthy ]; then return 0; fi
    sleep 2
  done
  return 1
}

roll() {
  exec 9>.deploy.lock
  flock -w 600 9 || { echo "another deploy held .deploy.lock for 10 minutes" >&2; exit 1; }
  touch .env
  setvar FOLIOLUM_APP_IMAGE "$1"
  docker compose pull foliolum-app
  docker compose up -d foliolum-app
  flock -u 9
}

previous="$(sed -n 's/^FOLIOLUM_APP_IMAGE=//p' .env 2>/dev/null || true)"
roll "$image"

if healthy && ./smoke.sh foliolum-app; then
  echo "foliolum-app is healthy and serves $image"
  exit 0
fi

echo "foliolum-app failed with $image" >&2
docker compose ps >&2
docker compose logs --tail=60 foliolum-app >&2
if [ -n "$previous" ] && [ "$previous" != "$image" ]; then
  echo "rolling back to $previous" >&2
  roll "$previous"
  healthy || echo "the previous image is not healthy either" >&2
fi
exit 1
