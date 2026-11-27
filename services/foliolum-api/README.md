# foliolum-api

Foliolum's backend, served same-origin at `https://app.foliolum.app/api/` (the editor image's nginx
passes `/api/` to it). It does four things:

- **Who is calling.** Claviger ID tokens (`Authorization: Bearer …`, OIDC client `foliolum-app`),
  checked against Claviger's JWKS, issuer and audience (`src/auth.rs`). Claviger says who someone is;
  which parish they're in is ours. Emails aren't verified yet, so nothing is granted by email:
  people join a parish only through an invite link.
- **Parishes.** Whoever starts one is its admin; admins make invite links (14 days, 25 uses, only the
  sha256 is stored); anyone signed in can join with one, as a member.
- **Shared templates.** Members save `.designcraft` documents to their parish and open each other's.
  Saving under a name that exists replaces it; whoever saved it last or an admin can delete it.
- **Proofreading.** A story's text goes to the fleet's AI relay with a key only this service holds,
  under a per-person hourly limit and a size cap (`src/proofread.rs`). The model names each fix by
  the words it changes; offsets are worked out here against the text that was sent. The text is
  never logged and never stored. When the relay is busy or asleep it answers 503 and the editor
  says so.

| Method | Path | Who |
|---|---|---|
| GET | `/api/health` | anyone (liveness) |
| GET | `/api/ready` | anyone (database reachable) |
| GET | `/api/me` | signed in: name, email, parishes, whether proofreading is on |
| POST | `/api/parishes` `{name}` | signed in (up to 5 started each) |
| POST | `/api/parishes/{id}/invites` | the parish's admins → `{token, expires, uses}` |
| DELETE | `/api/parishes/{id}/membership` | a member, to leave |
| POST | `/api/invites/accept` `{token}` | signed in |
| GET, POST | `/api/parishes/{id}/templates` (`{name, document}`, base64, ≤ 8 MB) | members |
| GET, DELETE | `/api/templates/{id}` | members (delete: who saved it last, or an admin) |
| POST | `/api/proofread` `{text}` (≤ 6000 characters) → `{suggestions}` | signed in |

Storage is Postgres (`src/pg.rs`, `migrations/`), as the least-privilege `foliolum_app` role; the
schema's owner `foliolum_owner` runs the migrations at start. Both roles come from
`deploy/doodlessd/db-init/01-roles.sh`. Configuration is environment only (`src/main.rs`).

## Tests

`cargo test` here runs the store-independent tests and the whole HTTP API on an in-memory store,
with ID tokens signed by a key made at test time. `tests/pg.rs` runs the same store against a real
Postgres made by the stack's roles script, and skips without one:

```sh
cid=$(docker create -p 127.0.0.1:55432:5432 -e POSTGRES_PASSWORD=pw -e POSTGRES_DB=foliolum \
  -e FOLIOLUM_DB=foliolum -e FOLIOLUM_OWNER_ROLE=foliolum_owner -e FOLIOLUM_OWNER_PASSWORD=ownerpw \
  -e FOLIOLUM_APP_ROLE=foliolum_app -e FOLIOLUM_APP_PASSWORD=apppw -e POSTGRES_USER=postgres postgres:17-alpine)
docker cp ../../deploy/doodlessd/db-init/01-roles.sh "$cid":/docker-entrypoint-initdb.d/ && docker start "$cid"
FOLIOLUM_TEST_OWNER_URL=postgres://foliolum_owner:ownerpw@127.0.0.1:55432/foliolum \
FOLIOLUM_TEST_APP_URL=postgres://foliolum_app:apppw@127.0.0.1:55432/foliolum cargo test --test pg
```

Its own workspace, so the image builds from this folder alone: `docker build -t foliolum-api .`
