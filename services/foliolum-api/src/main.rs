//! foliolum-api: configuration comes from the environment (the stack's .env, written by
//! deploy/doodlessd/wire-backend.sh), never from arguments.
//!
//! - `FOLIOLUM_DB_HOST` (default `foliolum-db`) and `FOLIOLUM_DB`: the database.
//! - `FOLIOLUM_APP_ROLE`, `FOLIOLUM_APP_PASSWORD`: the role it serves as (rows only).
//! - `FOLIOLUM_OWNER_ROLE`, `FOLIOLUM_OWNER_PASSWORD`: the schema's owner, for the migrations at start.
//! - `FOLIOLUM_OIDC_ISSUER`, `FOLIOLUM_OIDC_CLIENT_ID`, `FOLIOLUM_OIDC_JWKS_URL`: Claviger.
//! - `FOLIOLUM_LLM_URL` (the relay's base address, or its full chat-completions address),
//!   `FOLIOLUM_LLM_KEY`, `FOLIOLUM_LLM_MODEL`: the AI relay. Without them
//!   proofreading answers "not available" and everything else works.
//! - `FOLIOLUM_API_LISTEN` (default `0.0.0.0:8081`).
use std::sync::Arc;
use std::time::Duration;

use foliolum_api::auth::Verifier;
use foliolum_api::pg::{Pg, config, migrate};
use foliolum_api::proofread::Relay;
use foliolum_api::{App, router};

fn var(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn need(name: &str) -> String {
    var(name).unwrap_or_else(|| {
        eprintln!("foliolum-api: set {name}");
        std::process::exit(2)
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into())).init();

    let host = var("FOLIOLUM_DB_HOST").unwrap_or_else(|| "foliolum-db".into());
    let db = need("FOLIOLUM_DB");
    let store = Pg::new(config(&host, &db, &need("FOLIOLUM_APP_ROLE"), &need("FOLIOLUM_APP_PASSWORD"))).unwrap_or_else(|e| {
        eprintln!("foliolum-api: database: {e}");
        std::process::exit(2)
    });
    let issuer = var("FOLIOLUM_OIDC_ISSUER").unwrap_or_else(|| "https://sso.claviger.app/oidc".into());
    let jwks = var("FOLIOLUM_OIDC_JWKS_URL").unwrap_or_else(|| format!("{issuer}/keys"));
    let verifier = Verifier::new(&issuer, &var("FOLIOLUM_OIDC_CLIENT_ID").unwrap_or_else(|| "foliolum-app".into()), &jwks);
    let ai = match (var("FOLIOLUM_LLM_URL"), var("FOLIOLUM_LLM_KEY")) {
        (Some(url), Some(key)) => Some(Relay::new(&url, &key, &var("FOLIOLUM_LLM_MODEL").unwrap_or_else(|| "local:default".into()))),
        _ => {
            tracing::warn!("no AI relay configured: proofreading is off");
            None
        }
    };

    // The database may start after us: keep trying the migrations in the background, and answer
    // /api/ready with 503 until it is up.
    let owner = config(&host, &db, &need("FOLIOLUM_OWNER_ROLE"), &need("FOLIOLUM_OWNER_PASSWORD"));
    tokio::spawn(async move {
        loop {
            match migrate(&owner).await {
                Ok(()) => break,
                Err(e) => {
                    tracing::warn!("migrations: {e}; retrying");
                    tokio::time::sleep(Duration::from_secs(5)).await;
                }
            }
        }
    });

    let listen = var("FOLIOLUM_API_LISTEN").unwrap_or_else(|| "0.0.0.0:8081".into());
    let listener = tokio::net::TcpListener::bind(&listen).await.unwrap_or_else(|e| {
        eprintln!("foliolum-api: listen on {listen}: {e}");
        std::process::exit(1)
    });
    tracing::info!("listening on {listen}");
    let app = router(Arc::new(App::new(store, verifier, ai)));
    let shutdown = async {
        let mut term = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("SIGTERM handler");
        tokio::select! { _ = term.recv() => {}, _ = tokio::signal::ctrl_c() => {} }
    };
    if let Err(e) = axum::serve(listener, app).with_graceful_shutdown(shutdown).await {
        eprintln!("foliolum-api: {e}");
        std::process::exit(1)
    }
}
