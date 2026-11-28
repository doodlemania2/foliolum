//! Foliolum's backend, served same-origin at app.foliolum.app/api (the app image's nginx passes
//! `/api/` here). See `README.md` beside this crate.
pub mod auth;
pub mod pg;
pub mod proofread;
pub mod store;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use auth::{Caller, Verifier};
use proofread::{Ai, AiError};
use store::{InviteError, Role, Store};

/// Limits, kept small: this serves parish offices, not a public API.
pub mod limits {
    /// Parishes one person may start.
    pub const PARISHES_PER_PERSON: i64 = 5;
    pub const TEMPLATES_PER_PARISH: usize = 200;
    /// A template's .designcraft bytes.
    pub const TEMPLATE_BYTES: usize = 8 * 1024 * 1024;
    pub const NAME_CHARS: usize = 120;
    pub const INVITE_DAYS: i64 = 14;
    pub const INVITE_USES: i32 = 25;
    /// Proofreading requests per person per hour (the relay's own limit is per app).
    pub const PROOFREADS_PER_HOUR: usize = 40;
    /// Proofreading requests in flight at once, matching the relay's limit for this app.
    pub const PROOFREADS_AT_ONCE: usize = 2;
}

pub struct App<S, A> {
    pub store: S,
    pub verifier: Verifier,
    /// None when no relay is configured: proofreading answers 503.
    pub ai: Option<A>,
    proofreads: Mutex<HashMap<String, VecDeque<Instant>>>,
    in_flight: tokio::sync::Semaphore,
}

impl<S: Store, A: Ai> App<S, A> {
    pub fn new(store: S, verifier: Verifier, ai: Option<A>) -> App<S, A> {
        App { store, verifier, ai, proofreads: Mutex::default(), in_flight: tokio::sync::Semaphore::new(limits::PROOFREADS_AT_ONCE) }
    }

    /// One more proofread for `sub` this hour, if they have any left.
    fn proofread_allowed(&self, sub: &str) -> bool {
        let mut all = self.proofreads.lock().unwrap_or_else(|e| e.into_inner());
        let now = Instant::now();
        all.retain(|_, q| q.back().is_some_and(|t| now.duration_since(*t) < Duration::from_secs(3600)));
        let q = all.entry(sub.to_string()).or_default();
        while q.front().is_some_and(|t| now.duration_since(*t) >= Duration::from_secs(3600)) {
            q.pop_front();
        }
        if q.len() >= limits::PROOFREADS_PER_HOUR {
            return false;
        }
        q.push_back(now);
        true
    }
}

pub struct ApiError(StatusCode, String);

impl ApiError {
    fn new(status: StatusCode, msg: impl Into<String>) -> ApiError {
        ApiError(status, msg.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(json!({"error": self.1}))).into_response()
    }
}

fn internal(e: String) -> ApiError {
    tracing::error!("store: {e}");
    ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong on our side. Try again in a minute.")
}

type Res<T> = Result<T, ApiError>;
type St<S, A> = State<Arc<App<S, A>>>;

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or_default()
}

/// The signed-in caller, recorded in `users`.
async fn caller<S: Store, A: Ai>(app: &App<S, A>, headers: &HeaderMap) -> Res<Caller> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "Sign in first."))?;
    let c =
        app.verifier.verify(token.trim()).await.ok_or_else(|| ApiError::new(StatusCode::UNAUTHORIZED, "Your sign-in has run out. Sign in again."))?;
    app.store.upsert_user(&c.sub, &c.email, &c.name).await.map_err(internal)?;
    Ok(c)
}

async fn member<S: Store, A: Ai>(app: &App<S, A>, sub: &str, parish: Uuid) -> Res<Role> {
    app.store.role(sub, parish).await.map_err(internal)?.ok_or_else(|| ApiError::new(StatusCode::NOT_FOUND, "You're not in that parish."))
}

fn name(raw: &str, what: &str) -> Res<String> {
    let n = raw.trim();
    if n.is_empty() || n.chars().count() > limits::NAME_CHARS || n.chars().any(char::is_control) {
        return Err(ApiError::new(StatusCode::BAD_REQUEST, format!("Give the {what} a name of up to {} characters.", limits::NAME_CHARS)));
    }
    Ok(n.to_string())
}

pub fn router<S: Store, A: Ai>(app: Arc<App<S, A>>) -> Router {
    Router::new()
        .route("/api/health", get(|| async { "ok\n" }))
        .route("/api/ready", get(ready::<S, A>))
        .route("/api/me", get(me::<S, A>))
        .route("/api/parishes", post(create_parish::<S, A>))
        .route("/api/parishes/{id}/invites", post(create_invite::<S, A>))
        .route("/api/parishes/{id}/membership", delete(leave::<S, A>))
        .route("/api/invites/accept", post(accept_invite::<S, A>))
        .route(
            "/api/parishes/{id}/templates",
            get(list_templates::<S, A>).post(put_template::<S, A>).layer(DefaultBodyLimit::max(limits::TEMPLATE_BYTES * 4 / 3 + 64 * 1024)),
        )
        .route("/api/templates/{id}", get(get_template::<S, A>).delete(delete_template::<S, A>))
        .route("/api/proofread", post(proofread::<S, A>).layer(DefaultBodyLimit::max(64 * 1024)))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .with_state(app)
}

async fn ready<S: Store, A: Ai>(State(app): St<S, A>) -> StatusCode {
    if app.store.ready().await { StatusCode::OK } else { StatusCode::SERVICE_UNAVAILABLE }
}

async fn me<S: Store, A: Ai>(State(app): St<S, A>, headers: HeaderMap) -> Res<Json<Value>> {
    let c = caller(&app, &headers).await?;
    let parishes = app.store.memberships(&c.sub).await.map_err(internal)?;
    Ok(Json(json!({"sub": c.sub, "email": c.email, "name": c.name, "parishes": parishes, "proofreading": app.ai.is_some()})))
}

#[derive(Deserialize)]
struct NameBody {
    name: String,
}

async fn create_parish<S: Store, A: Ai>(State(app): St<S, A>, headers: HeaderMap, Json(b): Json<NameBody>) -> Res<Json<Value>> {
    let c = caller(&app, &headers).await?;
    let n = name(&b.name, "parish")?;
    if app.store.parishes_created(&c.sub).await.map_err(internal)? >= limits::PARISHES_PER_PERSON {
        return Err(ApiError::new(StatusCode::CONFLICT, format!("You can start up to {} parishes.", limits::PARISHES_PER_PERSON)));
    }
    let m = app.store.create_parish(&c.sub, &n).await.map_err(internal)?;
    Ok(Json(json!(m)))
}

fn hash(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

async fn create_invite<S: Store, A: Ai>(State(app): St<S, A>, headers: HeaderMap, Path(id): Path<Uuid>) -> Res<Json<Value>> {
    let c = caller(&app, &headers).await?;
    if member(&app, &c.sub, id).await? != Role::Admin {
        return Err(ApiError::new(StatusCode::FORBIDDEN, "Only the parish's admins can invite people."));
    }
    let token = URL_SAFE_NO_PAD.encode(rand::random::<[u8; 24]>());
    let expires = now() + limits::INVITE_DAYS * 86_400;
    app.store.create_invite(id, &c.sub, hash(&token), expires, limits::INVITE_USES).await.map_err(internal)?;
    Ok(Json(json!({"token": token, "expires": expires, "uses": limits::INVITE_USES})))
}

#[derive(Deserialize)]
struct TokenBody {
    token: String,
}

async fn accept_invite<S: Store, A: Ai>(State(app): St<S, A>, headers: HeaderMap, Json(b): Json<TokenBody>) -> Res<Json<Value>> {
    let c = caller(&app, &headers).await?;
    match app.store.accept_invite(hash(b.token.trim()), &c.sub, now()).await {
        Ok(m) => Ok(Json(json!(m))),
        Err(InviteError::Gone) => Err(ApiError::new(StatusCode::GONE, "That invite link has expired or been used up. Ask for a new one.")),
        Err(InviteError::Store(e)) => Err(internal(e)),
    }
}

async fn leave<S: Store, A: Ai>(State(app): St<S, A>, headers: HeaderMap, Path(id): Path<Uuid>) -> Res<StatusCode> {
    let c = caller(&app, &headers).await?;
    member(&app, &c.sub, id).await?;
    app.store.leave(&c.sub, id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_templates<S: Store, A: Ai>(State(app): St<S, A>, headers: HeaderMap, Path(id): Path<Uuid>) -> Res<Json<Value>> {
    let c = caller(&app, &headers).await?;
    member(&app, &c.sub, id).await?;
    Ok(Json(json!({"templates": app.store.templates(id).await.map_err(internal)?})))
}

#[derive(Deserialize)]
struct TemplateBody {
    name: String,
    /// The .designcraft file, base64.
    document: String,
}

async fn put_template<S: Store, A: Ai>(
    State(app): St<S, A>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(b): Json<TemplateBody>,
) -> Res<Json<Value>> {
    let c = caller(&app, &headers).await?;
    member(&app, &c.sub, id).await?;
    let n = name(&b.name, "template")?;
    let doc = STANDARD.decode(b.document.as_bytes()).map_err(|_| ApiError::new(StatusCode::BAD_REQUEST, "The template didn't arrive whole."))?;
    if doc.is_empty() || doc.len() > limits::TEMPLATE_BYTES {
        return Err(ApiError::new(StatusCode::PAYLOAD_TOO_LARGE, format!("A template can be up to {} MB.", limits::TEMPLATE_BYTES >> 20)));
    }
    let existing = app.store.templates(id).await.map_err(internal)?;
    if !existing.iter().any(|t| t.name == n) && existing.len() >= limits::TEMPLATES_PER_PARISH {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            format!("A parish can keep up to {} templates. Delete one first.", limits::TEMPLATES_PER_PARISH),
        ));
    }
    let tid = app.store.put_template(id, &c.sub, &n, &doc, now()).await.map_err(internal)?;
    Ok(Json(json!({"id": tid, "name": n})))
}

async fn get_template<S: Store, A: Ai>(State(app): St<S, A>, headers: HeaderMap, Path(id): Path<Uuid>) -> Res<Json<Value>> {
    let c = caller(&app, &headers).await?;
    let not_found = || ApiError::new(StatusCode::NOT_FOUND, "That template isn't there any more.");
    let t = app.store.template(id).await.map_err(internal)?.ok_or_else(not_found)?;
    if app.store.role(&c.sub, t.parish).await.map_err(internal)?.is_none() {
        return Err(not_found());
    }
    Ok(Json(json!({"id": id, "name": t.name, "document": STANDARD.encode(&t.document)})))
}

async fn delete_template<S: Store, A: Ai>(State(app): St<S, A>, headers: HeaderMap, Path(id): Path<Uuid>) -> Res<StatusCode> {
    let c = caller(&app, &headers).await?;
    let not_found = || ApiError::new(StatusCode::NOT_FOUND, "That template isn't there any more.");
    let t = app.store.template(id).await.map_err(internal)?.ok_or_else(not_found)?;
    match app.store.role(&c.sub, t.parish).await.map_err(internal)? {
        None => return Err(not_found()),
        Some(Role::Member) if t.saved_by_sub != c.sub => {
            return Err(ApiError::new(StatusCode::FORBIDDEN, "Only whoever saved it last, or a parish admin, can delete it."));
        }
        _ => {}
    }
    app.store.delete_template(id).await.map_err(internal)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct TextBody {
    text: String,
}

async fn proofread<S: Store, A: Ai>(State(app): St<S, A>, headers: HeaderMap, Json(b): Json<TextBody>) -> Res<Json<Value>> {
    let c = caller(&app, &headers).await?;
    let unavailable = || ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Proofreading isn't available right now. Try again later.");
    let ai = app.ai.as_ref().ok_or_else(unavailable)?;
    if b.text.chars().count() > proofread::MAX_CHARS {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("Proofread up to {} characters at a time: select one text box.", proofread::MAX_CHARS),
        ));
    }
    if !app.proofread_allowed(&c.sub) {
        return Err(ApiError::new(StatusCode::TOO_MANY_REQUESTS, "That's a lot of proofreading for one hour. Try again a little later."));
    }
    let _slot =
        app.in_flight.try_acquire().map_err(|_| ApiError::new(StatusCode::SERVICE_UNAVAILABLE, "Proofreading is busy. Try again in a moment."))?;
    match proofread::proofread(ai, &b.text).await {
        Ok(s) => Ok(Json(json!({"suggestions": s}))),
        Err(AiError::Unavailable) => Err(unavailable()),
        Err(AiError::Bad(why)) => {
            tracing::warn!("proofread: unusable answer ({why})");
            Err(ApiError::new(StatusCode::BAD_GATEWAY, "The proofreader's answer didn't make sense. Try again."))
        }
    }
}
