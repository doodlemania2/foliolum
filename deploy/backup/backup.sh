#!/bin/sh
# One backup: a custom-format dump straight into restic (no dump file on disk, nothing to clean
# up), then a retention pass on Sundays. Restic encrypts on this side, so the bucket only ever holds
# ciphertext. Custom format, so pg_restore can bring back one table (the templates, say) on its own.
#
# Settings: backup.env (RESTIC_REPOSITORY, RESTIC_PASSWORD, AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY,
# BACKUP_AT, OTEL_EXPORTER_OTLP_ENDPOINT, OTEL_AUTH_HEADER), from OpenBao secret/apps/foliolum-backup.
set -eu

: "${RESTIC_REPOSITORY:?}" "${RESTIC_PASSWORD:?}" "${PGPASSWORD:?}"
host="${PGHOST:-foliolum-db}"
db="${PGDATABASE:-foliolum}"
export RESTIC_CACHE_DIR=/tmp/restic-cache

notify() { # level message: a notice must never be the reason a backup fails
  [ -n "${OTEL_EXPORTER_OTLP_ENDPOINT:-}" ] || return 0
  sev=9; [ "$1" = ERROR ] && sev=17
  now="$(date +%s)000000000"
  curl -sS --max-time 10 -o /dev/null -X POST "${OTEL_EXPORTER_OTLP_ENDPOINT}/v1/logs" \
    -H "Content-Type: application/json" ${OTEL_AUTH_HEADER:+-H "$OTEL_AUTH_HEADER"} \
    -d "{\"resourceLogs\":[{\"resource\":{\"attributes\":[{\"key\":\"service.name\",\"value\":{\"stringValue\":\"foliolum-backup\"}}]},\"scopeLogs\":[{\"logRecords\":[{\"timeUnixNano\":\"$now\",\"severityNumber\":$sev,\"severityText\":\"$1\",\"body\":{\"stringValue\":\"$2\"}}]}]}]}" \
    || true
}
trap 'notify ERROR "foliolum backup FAILED"; exit 1' EXIT

restic cat config >/dev/null 2>&1 || restic init
pg_dump -h "$host" -U "${PGUSER:?}" -d "$db" -Fc --no-owner \
  | restic backup --stdin --stdin-filename foliolum.dump --tag foliolum --host foliolum

# Prove the snapshot is readable, not merely written.
restic snapshots --tag foliolum --latest 1 >/dev/null

if [ "$(date +%u)" = 7 ]; then
  restic forget --tag foliolum --keep-daily 14 --keep-weekly 8 --keep-monthly 12 --prune
fi

trap - EXIT
notify INFO "foliolum backup ok"
echo "backup ok $(date -u +%FT%TZ)"
