//! The [`Store`] in Postgres. The service runs as the least-privilege `foliolum_app` role; the
//! schema is made by `foliolum_owner` ([`migrate`]), which the stack's db-init/01-roles.sh creates.
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::NoTls;
use uuid::Uuid;

use crate::store::{InviteError, Membership, Result, Role, Store, Template, TemplateInfo};

const MIGRATIONS: &[(&str, &str)] = &[("0001_init", include_str!("../migrations/0001_init.sql"))];

trait Detail {
    fn detail(&self) -> String;
}
impl Detail for tokio_postgres::Error {
    // "db error" alone says nothing; the server's message says what went wrong (never row data).
    fn detail(&self) -> String {
        match self.as_db_error() {
            Some(db) => format!("{}: {}", db.code().code(), db.message()),
            None => self.to_string(),
        }
    }
}
impl Detail for deadpool_postgres::PoolError {
    fn detail(&self) -> String {
        match self {
            deadpool_postgres::PoolError::Backend(e) => e.detail(),
            e => e.to_string(),
        }
    }
}
impl Detail for deadpool_postgres::BuildError {
    fn detail(&self) -> String {
        self.to_string()
    }
}

fn err(e: impl Detail) -> String {
    e.detail()
}

/// Applies the migrations not yet applied, as the owner role.
pub async fn migrate(config: &tokio_postgres::Config) -> Result<()> {
    let (client, conn) = config.connect(NoTls).await.map_err(err)?;
    tokio::spawn(async move {
        if let Err(e) = conn.await {
            tracing::warn!("migration connection: {e}");
        }
    });
    client
        .batch_execute("create table if not exists schema_migrations (name text primary key, applied_at timestamptz not null default now())")
        .await
        .map_err(err)?;
    for (name, sql) in MIGRATIONS {
        let done = client.query_opt("select 1 from schema_migrations where name = $1", &[name]).await.map_err(err)?.is_some();
        if done {
            continue;
        }
        client.batch_execute(&format!("begin; {sql}; insert into schema_migrations (name) values ('{name}'); commit;")).await.map_err(err)?;
        tracing::info!("applied migration {name}");
    }
    Ok(())
}

/// Connection settings from parts, so a password needs no URL escaping.
pub fn config(host: &str, db: &str, user: &str, password: &str) -> tokio_postgres::Config {
    let mut c = tokio_postgres::Config::new();
    c.host(host)
        .port(5432)
        .dbname(db)
        .user(user)
        .password(password)
        .application_name("foliolum-api")
        .connect_timeout(std::time::Duration::from_secs(10));
    c
}

pub struct Pg(Pool);

impl Pg {
    pub fn new(config: tokio_postgres::Config) -> Result<Pg> {
        let manager = Manager::from_config(config, NoTls, ManagerConfig { recycling_method: RecyclingMethod::Fast });
        Ok(Pg(Pool::builder(manager).max_size(8).build().map_err(err)?))
    }

    async fn client(&self) -> Result<deadpool_postgres::Object> {
        self.0.get().await.map_err(err)
    }
}

fn role(s: &str) -> Role {
    Role::parse(s).unwrap_or(Role::Member)
}

impl Store for Pg {
    async fn upsert_user(&self, sub: &str, email: &str, name: &str) -> Result<()> {
        self.client()
            .await?
            .execute(
                "insert into users (sub, email, name) values ($1, $2, $3)
                 on conflict (sub) do update set email = excluded.email, name = excluded.name, seen_at = now()",
                &[&sub, &email, &name],
            )
            .await
            .map_err(err)?;
        Ok(())
    }

    async fn memberships(&self, sub: &str) -> Result<Vec<Membership>> {
        let rows = self
            .client()
            .await?
            .query("select p.id, p.name, m.role from memberships m join parishes p on p.id = m.parish_id where m.sub = $1 order by p.name", &[&sub])
            .await
            .map_err(err)?;
        Ok(rows.iter().map(|r| Membership { id: r.get(0), name: r.get(1), role: role(r.get(2)) }).collect())
    }

    async fn role(&self, sub: &str, parish: Uuid) -> Result<Option<Role>> {
        let row =
            self.client().await?.query_opt("select role from memberships where parish_id = $1 and sub = $2", &[&parish, &sub]).await.map_err(err)?;
        Ok(row.map(|r| role(r.get(0))))
    }

    async fn parishes_created(&self, sub: &str) -> Result<i64> {
        let row = self.client().await?.query_one("select count(*) from parishes where created_by = $1", &[&sub]).await.map_err(err)?;
        Ok(row.get(0))
    }

    async fn create_parish(&self, sub: &str, name: &str) -> Result<Membership> {
        let id = Uuid::new_v4();
        let mut client = self.client().await?;
        let tx = client.transaction().await.map_err(err)?;
        tx.execute("insert into parishes (id, name, created_by) values ($1, $2, $3)", &[&id, &name, &sub]).await.map_err(err)?;
        tx.execute("insert into memberships (parish_id, sub, role) values ($1, $2, 'admin')", &[&id, &sub]).await.map_err(err)?;
        tx.commit().await.map_err(err)?;
        Ok(Membership { id, name: name.into(), role: Role::Admin })
    }

    async fn create_invite(&self, parish: Uuid, sub: &str, hash: [u8; 32], expires: i64, uses: i32) -> Result<()> {
        self.client()
            .await?
            .execute(
                "insert into invites (token_hash, parish_id, created_by, expires_at, uses_left) values ($1, $2, $3, to_timestamp($4::bigint), $5)",
                &[&hash.as_slice(), &parish, &sub, &expires, &uses],
            )
            .await
            .map_err(err)?;
        Ok(())
    }

    async fn accept_invite(&self, hash: [u8; 32], sub: &str, now: i64) -> std::result::Result<Membership, InviteError> {
        let store = |e: tokio_postgres::Error| InviteError::Store(e.detail());
        let mut client = self.client().await.map_err(InviteError::Store)?;
        let tx = client.transaction().await.map_err(store)?;
        let row = tx
            .query_opt(
                "update invites set uses_left = uses_left - 1
                 where token_hash = $1 and uses_left > 0 and expires_at > to_timestamp($2::bigint)
                 returning parish_id",
                &[&hash.as_slice(), &now],
            )
            .await
            .map_err(store)?;
        let Some(row) = row else { return Err(InviteError::Gone) };
        let parish: Uuid = row.get(0);
        tx.execute("insert into memberships (parish_id, sub, role) values ($1, $2, 'member') on conflict do nothing", &[&parish, &sub])
            .await
            .map_err(store)?;
        let row = tx
            .query_one(
                "select p.name, m.role from memberships m join parishes p on p.id = m.parish_id where m.parish_id = $1 and m.sub = $2",
                &[&parish, &sub],
            )
            .await
            .map_err(store)?;
        tx.commit().await.map_err(store)?;
        Ok(Membership { id: parish, name: row.get(0), role: role(row.get(1)) })
    }

    async fn leave(&self, sub: &str, parish: Uuid) -> Result<()> {
        self.client().await?.execute("delete from memberships where parish_id = $1 and sub = $2", &[&parish, &sub]).await.map_err(err)?;
        Ok(())
    }

    async fn templates(&self, parish: Uuid) -> Result<Vec<TemplateInfo>> {
        let rows = self
            .client()
            .await?
            .query(
                "select t.id, t.name, coalesce(u.name, ''), extract(epoch from t.updated_at)::bigint, length(t.document)::bigint
                 from templates t left join users u on u.sub = t.saved_by where t.parish_id = $1 order by t.name",
                &[&parish],
            )
            .await
            .map_err(err)?;
        Ok(rows.iter().map(|r| TemplateInfo { id: r.get(0), name: r.get(1), saved_by: r.get(2), updated: r.get(3), size: r.get(4) }).collect())
    }

    async fn put_template(&self, parish: Uuid, sub: &str, name: &str, document: &[u8], now: i64) -> Result<Uuid> {
        let row = self
            .client()
            .await?
            .query_one(
                "insert into templates (id, parish_id, name, saved_by, updated_at, document) values ($1, $2, $3, $4, to_timestamp($5::bigint), $6)
                 on conflict (parish_id, name) do update set saved_by = excluded.saved_by, updated_at = excluded.updated_at, document = excluded.document
                 returning id",
                &[&Uuid::new_v4(), &parish, &name, &sub, &now, &document],
            )
            .await
            .map_err(err)?;
        Ok(row.get(0))
    }

    async fn template(&self, id: Uuid) -> Result<Option<Template>> {
        let row =
            self.client().await?.query_opt("select parish_id, name, saved_by, document from templates where id = $1", &[&id]).await.map_err(err)?;
        Ok(row.map(|r| Template { parish: r.get(0), name: r.get(1), saved_by_sub: r.get(2), document: r.get(3) }))
    }

    async fn delete_template(&self, id: Uuid) -> Result<()> {
        self.client().await?.execute("delete from templates where id = $1", &[&id]).await.map_err(err)?;
        Ok(())
    }

    async fn ready(&self) -> bool {
        match self.client().await {
            Ok(c) => c.query_one("select 1", &[]).await.is_ok(),
            Err(_) => false,
        }
    }
}
