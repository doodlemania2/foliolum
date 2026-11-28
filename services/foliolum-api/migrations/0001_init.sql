-- Run by foliolum_owner (crate::pg::migrate). foliolum_app, the role the service runs as, gets
-- only row access.
create table if not exists users (
  sub        text primary key,
  email      text not null default '',
  name       text not null default '',
  created_at timestamptz not null default now(),
  seen_at    timestamptz not null default now()
);

create table if not exists parishes (
  id         uuid primary key,
  name       text not null check (length(name) between 1 and 120),
  created_by text not null references users (sub),
  created_at timestamptz not null default now()
);

create table if not exists memberships (
  parish_id uuid not null references parishes (id) on delete cascade,
  sub       text not null references users (sub) on delete cascade,
  role      text not null check (role in ('admin', 'member')),
  joined_at timestamptz not null default now(),
  primary key (parish_id, sub)
);
create index if not exists memberships_sub on memberships (sub);

-- Only the sha256 of an invite token is kept; the token itself is in the link.
create table if not exists invites (
  token_hash bytea primary key,
  parish_id  uuid not null references parishes (id) on delete cascade,
  created_by text not null references users (sub),
  expires_at timestamptz not null,
  uses_left  integer not null
);

create table if not exists templates (
  id         uuid primary key,
  parish_id  uuid not null references parishes (id) on delete cascade,
  name       text not null check (length(name) between 1 and 120),
  saved_by   text not null references users (sub),
  updated_at timestamptz not null default now(),
  document   bytea not null,
  unique (parish_id, name)
);

grant select, insert, update, delete on users, parishes, memberships, invites, templates to foliolum_app;
