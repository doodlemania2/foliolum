#!/usr/bin/env bash
# Starts an image the way compose runs it (read-only, no capabilities) and runs the checks deploy.sh
# runs after a rollout. Needs docker only.
#
#   deploy/smoke-image.sh <image>
set -euo pipefail
image="${1:?usage: smoke-image.sh <image>}"
name="foliolum-app-smoke-$$"
trap 'docker rm -f "$name" >/dev/null 2>&1 || true' EXIT
docker run -d --name "$name" --read-only --tmpfs /tmp --cap-drop ALL \
  --security-opt no-new-privileges:true "$image" >/dev/null
docker exec "$name" nginx -t
"$(dirname "$0")/doodlessd/smoke.sh" "$name"
