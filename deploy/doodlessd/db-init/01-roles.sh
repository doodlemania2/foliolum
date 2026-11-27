#!/bin/sh
# Runs once, on the first start of an empty data directory (the postgres image's
# docker-entrypoint-initdb.d contract). Makes the database's two roles and locks it down, the
# chores pattern: foliolum_owner owns the schema and runs the api's migrations at start;
# foliolum_app, which the api serves as, gets row access only (migrations/0001_init.sql grants it).
# Names and passwords arrive as environment variables from .env (wire-backend.sh).
set -eu

psql -v ON_ERROR_STOP=1 --username "${POSTGRES_USER}" --dbname postgres <<SQL
create role ${FOLIOLUM_OWNER_ROLE} login password '${FOLIOLUM_OWNER_PASSWORD}' connection limit 5;
create role ${FOLIOLUM_APP_ROLE} login password '${FOLIOLUM_APP_PASSWORD}' connection limit 20;
alter database ${FOLIOLUM_DB} owner to ${FOLIOLUM_OWNER_ROLE};
revoke connect on database ${FOLIOLUM_DB} from public;
grant connect on database ${FOLIOLUM_DB} to ${FOLIOLUM_OWNER_ROLE}, ${FOLIOLUM_APP_ROLE};
SQL

psql -v ON_ERROR_STOP=1 --username "${POSTGRES_USER}" --dbname "${FOLIOLUM_DB}" <<SQL
revoke all on schema public from public;
alter schema public owner to ${FOLIOLUM_OWNER_ROLE};
grant usage on schema public to ${FOLIOLUM_APP_ROLE};
SQL
