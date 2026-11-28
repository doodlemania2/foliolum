//! Who is calling: the Claviger ID token in `Authorization: Bearer …`, checked against Claviger's
//! JWKS (RS256), its issuer and Foliolum's client id. Claviger says who someone is; which parish
//! they belong to is ours ([`crate::store`]). The email is not verified (no mail is sent yet), so
//! nothing here grants anything by email.
use std::collections::HashMap;
use std::time::{Duration, Instant};

use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use tokio::sync::RwLock;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Caller {
    pub sub: String,
    pub email: String,
    pub name: String,
}

#[derive(Deserialize)]
struct Claims {
    sub: String,
    #[serde(default)]
    email: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    preferred_username: String,
}

#[derive(Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Deserialize)]
struct Jwk {
    #[serde(default)]
    kid: String,
    #[serde(default)]
    kty: String,
    #[serde(default)]
    n: String,
    #[serde(default)]
    e: String,
}

pub struct Verifier {
    issuer: String,
    client_id: String,
    jwks_url: Option<String>,
    http: reqwest::Client,
    keys: RwLock<(HashMap<String, DecodingKey>, Option<Instant>)>,
}

/// How often an unknown key id may send us back to the JWKS.
const REFETCH: Duration = Duration::from_secs(60);

impl Verifier {
    pub fn new(issuer: &str, client_id: &str, jwks_url: &str) -> Verifier {
        Verifier {
            issuer: issuer.into(),
            client_id: client_id.into(),
            jwks_url: Some(jwks_url.into()),
            http: reqwest::Client::builder().timeout(Duration::from_secs(10)).build().unwrap_or_default(),
            keys: RwLock::new((HashMap::new(), None)),
        }
    }

    /// A verifier that knows only `keys` and never fetches (the tests).
    pub fn fixed(issuer: &str, client_id: &str, keys: HashMap<String, DecodingKey>) -> Verifier {
        Verifier { issuer: issuer.into(), client_id: client_id.into(), jwks_url: None, http: reqwest::Client::new(), keys: RwLock::new((keys, None)) }
    }

    async fn key(&self, kid: &str) -> Option<DecodingKey> {
        if let Some(k) = self.keys.read().await.0.get(kid) {
            return Some(k.clone());
        }
        let url = self.jwks_url.as_ref()?;
        let mut keys = self.keys.write().await;
        if keys.1.is_some_and(|at| at.elapsed() < REFETCH) {
            return keys.0.get(kid).cloned();
        }
        keys.1 = Some(Instant::now());
        match self.http.get(url).send().await.and_then(|r| r.error_for_status()) {
            Ok(r) => match r.json::<Jwks>().await {
                Ok(jwks) => {
                    keys.0 = jwks
                        .keys
                        .iter()
                        .filter(|k| k.kty == "RSA")
                        .filter_map(|k| Some((k.kid.clone(), DecodingKey::from_rsa_components(&k.n, &k.e).ok()?)))
                        .collect();
                    tracing::info!("loaded {} signing keys from the JWKS", keys.0.len());
                }
                Err(e) => tracing::warn!("JWKS: {e}"),
            },
            Err(e) => tracing::warn!("JWKS: {e}"),
        }
        keys.0.get(kid).cloned()
    }

    /// The caller a bearer token names, or None if it isn't a valid Foliolum ID token.
    pub async fn verify(&self, token: &str) -> Option<Caller> {
        let header = decode_header(token).ok()?;
        if header.alg != Algorithm::RS256 {
            return None;
        }
        let key = self.key(header.kid.as_deref().unwrap_or("")).await?;
        let mut v = Validation::new(Algorithm::RS256);
        v.set_issuer(&[&self.issuer]);
        v.set_audience(&[&self.client_id]);
        v.leeway = 30;
        // Not-before too (off by default), so a token that isn't valid yet, or whose nbf is malformed, is refused.
        v.validate_nbf = true;
        let c = decode::<Claims>(token, &key, &v).ok()?.claims;
        if c.sub.is_empty() {
            return None;
        }
        let name = if !c.name.is_empty() {
            c.name
        } else if !c.preferred_username.is_empty() {
            c.preferred_username
        } else {
            c.email.clone()
        };
        Some(Caller { sub: c.sub, email: c.email, name: name.chars().take(120).collect() })
    }
}
