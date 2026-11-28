//! The HTTP API end to end, on the in-memory store, with ID tokens signed by a key made here.
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use base64::Engine as _;
use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use foliolum_api::auth::Verifier;
use foliolum_api::proofread::{Ai, AiError};
use foliolum_api::store::Memory;
use foliolum_api::{App, router};
use http_body_util::BodyExt as _;
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, encode};
use rsa::pkcs1::EncodeRsaPrivateKey as _;
use rsa::traits::PublicKeyParts as _;
use serde_json::{Value, json};
use tower::ServiceExt as _;

const ISS: &str = "https://sso.example/oidc";
const AUD: &str = "foliolum-app";

fn keys() -> &'static (EncodingKey, DecodingKey) {
    static K: OnceLock<(EncodingKey, DecodingKey)> = OnceLock::new();
    K.get_or_init(|| {
        let private = rsa::RsaPrivateKey::new(&mut rand_core(), 2048).unwrap();
        let pem = private.to_pkcs1_pem(rsa::pkcs8::LineEnding::LF).unwrap();
        let public = private.to_public_key();
        let n = URL_SAFE_NO_PAD.encode(public.n().to_bytes_be());
        let e = URL_SAFE_NO_PAD.encode(public.e().to_bytes_be());
        (EncodingKey::from_rsa_pem(pem.as_bytes()).unwrap(), DecodingKey::from_rsa_components(&n, &e).unwrap())
    })
}

fn rand_core() -> rsa::rand_core::OsRng {
    rsa::rand_core::OsRng
}

fn token(sub: &str, aud: &str, exp_in: i64) -> String {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as i64;
    let mut h = Header::new(Algorithm::RS256);
    h.kid = Some("k1".into());
    let claims = json!({"iss": ISS, "aud": aud, "sub": sub, "exp": now + exp_in, "iat": now, "email": format!("{sub}@example.org"), "name": format!("Person {sub}")});
    encode(&h, &claims, &keys().0).unwrap()
}

/// An AI that fixes "teh" and can be told to be away.
struct Fake {
    away: bool,
}

impl Ai for Fake {
    async fn complete(&self, _system: &str, user: &str) -> Result<String, AiError> {
        if self.away {
            return Err(AiError::Unavailable);
        }
        Ok(if user.contains("teh") { r#"{"fixes":[{"find":"teh","replace":"the","why":"spelling"}]}"#.into() } else { r#"{"fixes":[]}"#.into() })
    }
}

fn app(away: bool) -> axum::Router {
    let verifier = Verifier::fixed(ISS, AUD, HashMap::from([("k1".to_string(), keys().1.clone())]));
    router(Arc::new(App::new(Memory::default(), verifier, Some(Fake { away }))))
}

async fn call(app: &axum::Router, method: &str, path: &str, who: Option<&str>, body: Option<Value>) -> (StatusCode, Value) {
    let mut req = Request::builder().method(method).uri(path);
    if let Some(t) = who {
        req = req.header("authorization", format!("Bearer {t}"));
    }
    let req = match body {
        Some(b) => req.header("content-type", "application/json").body(Body::from(b.to_string())).unwrap(),
        None => req.body(Body::empty()).unwrap(),
    };
    let res = app.clone().oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    (status, serde_json::from_slice(&bytes).unwrap_or(Value::Null))
}

#[tokio::test]
async fn sign_in_is_required_and_checked() {
    let app = app(false);
    assert_eq!(call(&app, "GET", "/api/health", None, None).await.0, StatusCode::OK);
    assert_eq!(call(&app, "GET", "/api/me", None, None).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(call(&app, "GET", "/api/me", Some(&token("a", "other-app", 300)), None).await.0, StatusCode::UNAUTHORIZED, "another client's token");
    assert_eq!(call(&app, "GET", "/api/me", Some(&token("a", AUD, -300)), None).await.0, StatusCode::UNAUTHORIZED, "expired");
    assert_eq!(call(&app, "GET", "/api/me", Some("not.a.token"), None).await.0, StatusCode::UNAUTHORIZED);
    // GHSA type confusion (jsonwebtoken < 10.3): an exp or nbf of the wrong JSON type must not
    // count as absent.
    let mut h = Header::new(Algorithm::RS256);
    h.kid = Some("k1".into());
    let far = "99999999999";
    for claims in
        [json!({"iss": ISS, "aud": AUD, "sub": "a", "exp": far}), json!({"iss": ISS, "aud": AUD, "sub": "a", "exp": 9_999_999_999i64, "nbf": far})]
    {
        let t = encode(&h, &claims, &keys().0).unwrap();
        assert_eq!(call(&app, "GET", "/api/me", Some(&t), None).await.0, StatusCode::UNAUTHORIZED, "{claims}");
    }
    let (s, me) = call(&app, "GET", "/api/me", Some(&token("a", AUD, 300)), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(me["sub"], "a");
    assert_eq!(me["name"], "Person a");
    assert_eq!(me["parishes"], json!([]));
}

#[tokio::test]
async fn a_parish_shares_templates_with_the_people_it_invites() {
    let app = app(false);
    let (a, b, c) = (token("a", AUD, 300), token("b", AUD, 300), token("c", AUD, 300));
    let (s, p) = call(&app, "POST", "/api/parishes", Some(&a), Some(json!({"name": " St Brigid's "}))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!((p["name"].as_str(), p["role"].as_str()), (Some("St Brigid's"), Some("admin")));
    let pid = p["id"].as_str().unwrap().to_string();
    assert_eq!(call(&app, "POST", "/api/parishes", Some(&a), Some(json!({"name": "  "}))).await.0, StatusCode::BAD_REQUEST);

    // b isn't in it yet.
    assert_eq!(call(&app, "GET", &format!("/api/parishes/{pid}/templates"), Some(&b), None).await.0, StatusCode::NOT_FOUND);
    let (_, inv) = call(&app, "POST", &format!("/api/parishes/{pid}/invites"), Some(&a), None).await;
    let invite = inv["token"].as_str().unwrap().to_string();
    let (s, joined) = call(&app, "POST", "/api/invites/accept", Some(&b), Some(json!({"token": invite}))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(joined["role"], "member");
    assert_eq!(call(&app, "POST", "/api/invites/accept", Some(&b), Some(json!({"token": "wrong"}))).await.0, StatusCode::GONE);
    // A member can't invite.
    assert_eq!(call(&app, "POST", &format!("/api/parishes/{pid}/invites"), Some(&b), None).await.0, StatusCode::FORBIDDEN);

    let doc = STANDARD.encode(b"{\"designcraft\":1}");
    let (s, t) = call(&app, "POST", &format!("/api/parishes/{pid}/templates"), Some(&b), Some(json!({"name": "Bulletin", "document": doc}))).await;
    assert_eq!(s, StatusCode::OK);
    let tid = t["id"].as_str().unwrap().to_string();
    // Saving the same name again replaces it.
    let (_, t2) = call(&app, "POST", &format!("/api/parishes/{pid}/templates"), Some(&a), Some(json!({"name": "Bulletin", "document": doc}))).await;
    assert_eq!(t2["id"], tid);
    let (_, list) = call(&app, "GET", &format!("/api/parishes/{pid}/templates"), Some(&a), None).await;
    assert_eq!(list["templates"].as_array().unwrap().len(), 1);
    assert_eq!(list["templates"][0]["saved_by"], "Person a");
    let (s, got) = call(&app, "GET", &format!("/api/templates/{tid}"), Some(&b), None).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(got["document"], doc);
    // c, outside the parish, can't see or delete it.
    assert_eq!(call(&app, "GET", &format!("/api/templates/{tid}"), Some(&c), None).await.0, StatusCode::NOT_FOUND);
    assert_eq!(call(&app, "DELETE", &format!("/api/templates/{tid}"), Some(&c), None).await.0, StatusCode::NOT_FOUND);
    // b saved it first, but a saved it last; b is only a member.
    assert_eq!(call(&app, "DELETE", &format!("/api/templates/{tid}"), Some(&b), None).await.0, StatusCode::FORBIDDEN);
    assert_eq!(call(&app, "DELETE", &format!("/api/templates/{tid}"), Some(&a), None).await.0, StatusCode::NO_CONTENT);
    let bad = json!({"name": "X", "document": "not base64!!"});
    assert_eq!(call(&app, "POST", &format!("/api/parishes/{pid}/templates"), Some(&a), Some(bad)).await.0, StatusCode::BAD_REQUEST);

    // b leaves.
    assert_eq!(call(&app, "DELETE", &format!("/api/parishes/{pid}/membership"), Some(&b), None).await.0, StatusCode::NO_CONTENT);
    assert_eq!(call(&app, "GET", "/api/me", Some(&b), None).await.1["parishes"], json!([]));
}

#[tokio::test]
async fn proofreading_suggests_and_degrades() {
    let app = app(false);
    let a = token("a", AUD, 300);
    assert_eq!(call(&app, "POST", "/api/proofread", None, Some(json!({"text": "teh"}))).await.0, StatusCode::UNAUTHORIZED);
    let (s, r) = call(&app, "POST", "/api/proofread", Some(&a), Some(json!({"text": "Join us after teh Mass."}))).await;
    assert_eq!(s, StatusCode::OK);
    assert_eq!(r["suggestions"][0]["start"], 14);
    assert_eq!(r["suggestions"][0]["replacement"], "the");
    let long = "a".repeat(foliolum_api::proofread::MAX_CHARS + 1);
    assert_eq!(call(&app, "POST", "/api/proofread", Some(&a), Some(json!({"text": long}))).await.0, StatusCode::PAYLOAD_TOO_LARGE);
    for _ in 1..foliolum_api::limits::PROOFREADS_PER_HOUR {
        call(&app, "POST", "/api/proofread", Some(&a), Some(json!({"text": "fine"}))).await;
    }
    assert_eq!(call(&app, "POST", "/api/proofread", Some(&a), Some(json!({"text": "fine"}))).await.0, StatusCode::TOO_MANY_REQUESTS);
    let away = self::app(true);
    assert_eq!(call(&away, "POST", "/api/proofread", Some(&a), Some(json!({"text": "teh"}))).await.0, StatusCode::SERVICE_UNAVAILABLE);
}
