#!/bin/sh
# Daily at BACKUP_AT (UTC, HH:MM). A loop rather than a host timer, so the whole thing lives in the
# stack and fails over with it. Writes a heartbeat the healthcheck reads.
set -u
at="${BACKUP_AT:-08:45}"
while true; do
  date +%s > /tmp/backup.beat
  if [ "$(date -u +%H:%M)" = "$at" ]; then
    /usr/local/bin/backup.sh || echo "backup failed" >&2
    sleep 61
  fi
  sleep 20
done
