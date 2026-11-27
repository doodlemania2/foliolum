#!/usr/bin/env bash
# Writes the backend's settings into this stack's .env, once, on the primary (doodlessd). The fleet
# coordinator (K3s) runs it with the values from OpenBao secret/apps/foliolum-db and
# secret/apps/foliolum-llm-relay on stdin, one KEY=VALUE per line, so no value is ever an argument,
# a log line or a file in the deploy tar:
#
#   <KEY=VALUE lines> | ssh doodlessd /home/derek/foliolum-app-stack/wire-backend.sh
#
# Keys it takes (anything else is refused):
#   POSTGRES_USER POSTGRES_PASSWORD FOLIOLUM_DB FOLIOLUM_OWNER_ROLE FOLIOLUM_OWNER_PASSWORD
#   FOLIOLUM_APP_ROLE FOLIOLUM_APP_PASSWORD     (secret/apps/foliolum-db)
#   FOLIOLUM_LLM_KEY FOLIOLUM_LLM_URL FOLIOLUM_LLM_MODEL   (secret/apps/foliolum-llm-relay: key,
#                                               url_docker, model)
#
# Role and database names must be lower-case identifiers (db-init/01-roles.sh puts them in SQL), and
# passwords and the key must be URL-safe characters, so neither the shell, compose nor SQL reads
# anything into them. It prints key names only, never values, holds the deploy lock, keeps a copy
# of the old .env as .env.pre-backend (both mode 600), and restarts nothing: the next deploy picks them up.
set -euo pipefail
cd "$(dirname "$(readlink -f "$0")")"
umask 077

declare -A want=()
names=(FOLIOLUM_DB FOLIOLUM_OWNER_ROLE FOLIOLUM_APP_ROLE POSTGRES_USER)
secrets=(POSTGRES_PASSWORD FOLIOLUM_OWNER_PASSWORD FOLIOLUM_APP_PASSWORD FOLIOLUM_LLM_KEY)
plain=(FOLIOLUM_LLM_URL FOLIOLUM_LLM_MODEL)

while IFS= read -r line || [ -n "$line" ]; do
  [ -z "${line// /}" ] && continue
  key="${line%%=*}"
  value="${line#*=}"
  case " ${names[*]} " in *" $key "*)
    [[ "$value" =~ ^[a-z_][a-z0-9_]{0,62}$ ]] || { echo "$key: not a lower-case identifier" >&2; exit 1; }
    want[$key]="$value"; continue ;;
  esac
  case " ${secrets[*]} " in *" $key "*)
    [[ "$value" =~ ^[A-Za-z0-9._~-]{16,256}$ ]] || { echo "$key: use 16 to 256 URL-safe characters" >&2; exit 1; }
    want[$key]="$value"; continue ;;
  esac
  case " ${plain[*]} " in *" $key "*)
    [[ "$value" =~ ^[A-Za-z0-9:/._~-]{1,256}$ ]] || { echo "$key: unexpected characters" >&2; exit 1; }
    want[$key]="$value"; continue ;;
  esac
  echo "refusing unknown key ${key:0:40}" >&2
  exit 1
done

for k in FOLIOLUM_DB FOLIOLUM_OWNER_ROLE FOLIOLUM_APP_ROLE POSTGRES_PASSWORD FOLIOLUM_OWNER_PASSWORD FOLIOLUM_APP_PASSWORD; do
  [ -n "${want[$k]:-}" ] || { echo "missing $k" >&2; exit 1; }
done

exec 9>.deploy.lock
flock -w 600 9 || { echo "a deploy held .deploy.lock for 10 minutes" >&2; exit 1; }
touch .env
cp -p .env .env.pre-backend
tmp="$(mktemp .env.XXXXXX)"
# Every line of the old .env whose key we are not setting, then ours.
awk -F= -v keys=" ${!want[*]} " 'index(keys, " " $1 " ") == 0' .env > "$tmp"
for k in "${!want[@]}"; do
  printf '%s=%s\n' "$k" "${want[$k]}" >> "$tmp"
done
# Always 600: it holds the database passwords (the old .env may have been 644).
chmod 600 "$tmp" .env.pre-backend
mv "$tmp" .env
echo "wrote ${!want[*]} to .env (the old one is .env.pre-backend)"
