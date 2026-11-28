//! What the backend keeps: people (by Claviger `sub`), parishes, who belongs to which, invite links
//! and the templates a parish shares. [`Store`] is the contract; [`Memory`] backs the tests and
//! `crate::pg::Pg` the service.
use std::collections::HashMap;
use std::future::Future;
use std::sync::Mutex;

use serde::Serialize;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Admin,
    Member,
}

impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Role::Admin => "admin",
            Role::Member => "member",
        }
    }
    pub fn parse(s: &str) -> Option<Role> {
        match s {
            "admin" => Some(Role::Admin),
            "member" => Some(Role::Member),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Membership {
    pub id: Uuid,
    pub name: String,
    pub role: Role,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TemplateInfo {
    pub id: Uuid,
    pub name: String,
    /// The name of whoever saved it last.
    pub saved_by: String,
    /// Seconds since 1970.
    pub updated: i64,
    pub size: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Template {
    pub parish: Uuid,
    pub name: String,
    pub saved_by_sub: String,
    pub document: Vec<u8>,
}

#[derive(Debug)]
pub enum InviteError {
    /// No such invite, or it has run out or expired.
    Gone,
    Store(String),
}

pub type Result<T> = std::result::Result<T, String>;

/// The backend's storage. Every method is one transaction.
pub trait Store: Send + Sync + 'static {
    fn upsert_user(&self, sub: &str, email: &str, name: &str) -> impl Future<Output = Result<()>> + Send;
    fn memberships(&self, sub: &str) -> impl Future<Output = Result<Vec<Membership>>> + Send;
    fn role(&self, sub: &str, parish: Uuid) -> impl Future<Output = Result<Option<Role>>> + Send;
    fn parishes_created(&self, sub: &str) -> impl Future<Output = Result<i64>> + Send;
    fn create_parish(&self, sub: &str, name: &str) -> impl Future<Output = Result<Membership>> + Send;
    fn create_invite(&self, parish: Uuid, sub: &str, hash: [u8; 32], expires: i64, uses: i32) -> impl Future<Output = Result<()>> + Send;
    /// Joins `sub` to the invite's parish as a member (kept as admin if they already are one) and
    /// uses the invite up by one.
    fn accept_invite(&self, hash: [u8; 32], sub: &str, now: i64) -> impl Future<Output = std::result::Result<Membership, InviteError>> + Send;
    fn leave(&self, sub: &str, parish: Uuid) -> impl Future<Output = Result<()>> + Send;
    fn templates(&self, parish: Uuid) -> impl Future<Output = Result<Vec<TemplateInfo>>> + Send;
    /// Saves a template under `name`, replacing the parish's template of that name.
    fn put_template(&self, parish: Uuid, sub: &str, name: &str, document: &[u8], now: i64) -> impl Future<Output = Result<Uuid>> + Send;
    fn template(&self, id: Uuid) -> impl Future<Output = Result<Option<Template>>> + Send;
    fn delete_template(&self, id: Uuid) -> impl Future<Output = Result<()>> + Send;
    fn ready(&self) -> impl Future<Output = bool> + Send;
}

/// Everything in memory, for the tests.
#[derive(Default)]
pub struct Memory(Mutex<Mem>);

#[derive(Default)]
struct Mem {
    users: HashMap<String, (String, String)>,
    parishes: HashMap<Uuid, (String, String)>,
    members: HashMap<(Uuid, String), Role>,
    invites: HashMap<[u8; 32], (Uuid, i64, i32)>,
    templates: HashMap<Uuid, (Template, i64)>,
}

impl Memory {
    fn with<T>(&self, f: impl FnOnce(&mut Mem) -> T) -> T {
        f(&mut self.0.lock().unwrap_or_else(|e| e.into_inner()))
    }
}

impl Store for Memory {
    async fn upsert_user(&self, sub: &str, email: &str, name: &str) -> Result<()> {
        self.with(|m| m.users.insert(sub.into(), (email.into(), name.into())));
        Ok(())
    }
    async fn memberships(&self, sub: &str) -> Result<Vec<Membership>> {
        Ok(self.with(|m| {
            let mut v: Vec<Membership> = m
                .members
                .iter()
                .filter(|((_, s), _)| s == sub)
                .map(|((p, _), r)| Membership { id: *p, name: m.parishes[p].0.clone(), role: *r })
                .collect();
            v.sort_by(|a, b| a.name.cmp(&b.name));
            v
        }))
    }
    async fn role(&self, sub: &str, parish: Uuid) -> Result<Option<Role>> {
        Ok(self.with(|m| m.members.get(&(parish, sub.to_string())).copied()))
    }
    async fn parishes_created(&self, sub: &str) -> Result<i64> {
        Ok(self.with(|m| m.parishes.values().filter(|(_, by)| by == sub).count() as i64))
    }
    async fn create_parish(&self, sub: &str, name: &str) -> Result<Membership> {
        let id = Uuid::new_v4();
        self.with(|m| {
            m.parishes.insert(id, (name.into(), sub.into()));
            m.members.insert((id, sub.into()), Role::Admin);
        });
        Ok(Membership { id, name: name.into(), role: Role::Admin })
    }
    async fn create_invite(&self, parish: Uuid, _sub: &str, hash: [u8; 32], expires: i64, uses: i32) -> Result<()> {
        self.with(|m| m.invites.insert(hash, (parish, expires, uses)));
        Ok(())
    }
    async fn accept_invite(&self, hash: [u8; 32], sub: &str, now: i64) -> std::result::Result<Membership, InviteError> {
        self.with(|m| {
            let Some((parish, expires, uses)) = m.invites.get_mut(&hash) else { return Err(InviteError::Gone) };
            if *expires <= now || *uses <= 0 {
                return Err(InviteError::Gone);
            }
            *uses -= 1;
            let parish = *parish;
            let role = *m.members.entry((parish, sub.into())).or_insert(Role::Member);
            Ok(Membership { id: parish, name: m.parishes[&parish].0.clone(), role })
        })
    }
    async fn leave(&self, sub: &str, parish: Uuid) -> Result<()> {
        self.with(|m| m.members.remove(&(parish, sub.to_string())));
        Ok(())
    }
    async fn templates(&self, parish: Uuid) -> Result<Vec<TemplateInfo>> {
        Ok(self.with(|m| {
            let mut v: Vec<TemplateInfo> = m
                .templates
                .iter()
                .filter(|(_, (t, _))| t.parish == parish)
                .map(|(id, (t, at))| TemplateInfo {
                    id: *id,
                    name: t.name.clone(),
                    saved_by: m.users.get(&t.saved_by_sub).map(|u| u.1.clone()).unwrap_or_default(),
                    updated: *at,
                    size: t.document.len() as i64,
                })
                .collect();
            v.sort_by(|a, b| a.name.cmp(&b.name));
            v
        }))
    }
    async fn put_template(&self, parish: Uuid, sub: &str, name: &str, document: &[u8], now: i64) -> Result<Uuid> {
        Ok(self.with(|m| {
            let id = m.templates.iter().find(|(_, (t, _))| t.parish == parish && t.name == name).map(|(id, _)| *id).unwrap_or_else(Uuid::new_v4);
            m.templates.insert(id, (Template { parish, name: name.into(), saved_by_sub: sub.into(), document: document.to_vec() }, now));
            id
        }))
    }
    async fn template(&self, id: Uuid) -> Result<Option<Template>> {
        Ok(self.with(|m| m.templates.get(&id).map(|(t, _)| t.clone())))
    }
    async fn delete_template(&self, id: Uuid) -> Result<()> {
        self.with(|m| m.templates.remove(&id));
        Ok(())
    }
    async fn ready(&self) -> bool {
        true
    }
}
