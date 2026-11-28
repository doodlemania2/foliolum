//! The browser half of [`crate::account`] and [`crate::proofread`]: the Claviger popup, the token
//! calls, creating an account, and Foliolum's backend at `/api/`. Results go into
//! [`crate::account::state`]; the shell repaints when they land.
use serde_json::{Value, json};
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::account::{self, Me, State, TemplateInfo, Tokens, Want, state};
use crate::pkce::{base64url, challenge};

const STORE_KEY: &str = "foliolum.session";

fn js(e: JsValue) -> String {
    format!("{e:?}")
}

fn now() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}

fn storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok()?
}

/// Keeps the refresh token in this browser, so a reload stays signed in (chores does the same).
fn remember(t: Option<&Tokens>) {
    if let Some(s) = storage() {
        let _ = match t {
            Some(t) => s.set_item(STORE_KEY, &t.refresh_token),
            None => s.remove_item(STORE_KEY),
        };
    }
}

fn random(n: usize) -> Result<String, String> {
    let crypto = web_sys::window().ok_or("no window")?.crypto().map_err(js)?;
    let mut b = vec![0u8; n];
    crypto.get_random_values_with_u8_array(&mut b).map_err(js)?;
    Ok(base64url(&b))
}

/// One fetch: (status, body). Status 0 means it never got an answer.
async fn fetch(method: &str, url: &str, content_type: Option<&str>, body: Option<String>, bearer: Option<&str>) -> (u16, String) {
    let go = async {
        let init = web_sys::RequestInit::new();
        init.set_method(method);
        init.set_mode(web_sys::RequestMode::Cors);
        let headers = web_sys::Headers::new().map_err(js)?;
        headers.set("Accept", "application/json").map_err(js)?;
        if let Some(ct) = content_type {
            headers.set("Content-Type", ct).map_err(js)?;
        }
        if let Some(t) = bearer {
            headers.set("Authorization", &format!("Bearer {t}")).map_err(js)?;
        }
        init.set_headers(&headers);
        if let Some(b) = &body {
            init.set_body(&JsValue::from_str(b));
        }
        let request = web_sys::Request::new_with_str_and_init(url, &init).map_err(js)?;
        let window = web_sys::window().ok_or("no window")?;
        let response: web_sys::Response = JsFuture::from(window.fetch_with_request(&request)).await.map_err(js)?.dyn_into().map_err(js)?;
        let text = JsFuture::from(response.text().map_err(js)?).await.map_err(js)?.as_string().unwrap_or_default();
        Ok::<_, String>((response.status(), text))
    };
    go.await.unwrap_or_else(|e| {
        log::warn!("account: {method} {url}: {e}");
        (0, String::new())
    })
}

/// A sign-in whose Claviger window is open.
pub struct Pending {
    id: u32,
    verifier: String,
    state: String,
    reply: js_sys::Promise,
    channel: web_sys::BroadcastChannel,
}

/// Opens Claviger's sign-in page in a popup. It has to open in the frame that saw the click, so
/// browsers count it as the person's own action.
pub fn begin_sign_in(id: u32) -> Result<Pending, String> {
    let window = web_sys::window().ok_or("no window")?;
    let (verifier, st, nonce) = (random(32)?, random(16)?, random(16)?);
    let channel = web_sys::BroadcastChannel::new(account::CHANNEL).map_err(js)?;
    let reply = js_sys::Promise::new(&mut |resolve, _| {
        let on_message = wasm_bindgen::closure::Closure::once_into_js(move |e: web_sys::MessageEvent| {
            let _ = resolve.call1(&JsValue::NULL, &e.data());
        });
        channel.set_onmessage(Some(on_message.unchecked_ref()));
    });
    let url = account::authorize_url(&challenge(&verifier), &st, &nonce);
    match window.open_with_url_and_target_and_features(&url, "foliolum_auth", "popup,width=480,height=720") {
        Ok(Some(_)) => Ok(Pending { id, verifier, state: st, reply, channel }),
        _ => {
            channel.close();
            Err("The browser blocked the Claviger window. Allow pop-ups for app.foliolum.app, then try again.".into())
        }
    }
}

/// Waits for `/auth/callback`'s answer, trades the code for tokens, and loads who this is.
pub async fn finish_sign_in(p: Pending, ctx: egui::Context) {
    let data = JsFuture::from(p.reply).await;
    p.channel.close();
    if state().waiting != Some(p.id) {
        return; // cancelled
    }
    let result = async {
        let query = data.map_err(js)?.as_string().unwrap_or_default();
        let code = account::read_callback(&query, &p.state)?;
        let (status, body) =
            fetch("POST", account::TOKEN, Some("application/x-www-form-urlencoded"), Some(account::code_request(&code, &p.verifier)), None).await;
        account::read_tokens(status, &body, "")
    }
    .await;
    {
        let mut st = state();
        st.waiting = None;
        match result {
            Ok(t) => {
                remember(Some(&t));
                st.tokens = Some(t);
                st.message.clear();
            }
            Err(e) => st.message = e,
        }
    }
    load_me(&ctx).await;
}

/// Renews the sign-in when its ID token is about to run out. On failure the person is signed out.
async fn fresh_token() -> Result<String, String> {
    let t = state().tokens.clone().ok_or("Sign in first.")?;
    if !account::needs_refresh(&t, now()) {
        return Ok(t.id_token);
    }
    let (status, body) =
        fetch("POST", account::TOKEN, Some("application/x-www-form-urlencoded"), Some(account::refresh_request(&t.refresh_token)), None).await;
    match account::read_tokens(status, &body, &t.refresh_token) {
        Ok(new) => {
            remember(Some(&new));
            let id = new.id_token.clone();
            state().tokens = Some(new);
            Ok(id)
        }
        Err(e) => {
            if status != 0 {
                // Claviger refused the renewal: the sign-in is over.
                remember(None);
                let mut st = state();
                st.tokens = None;
                st.me = None;
                st.templates = None;
            }
            Err(e)
        }
    }
}

/// A call to Foliolum's backend: the answer's JSON, or its error in words.
pub async fn api(method: &str, path: &str, body: Option<Value>) -> Result<Value, String> {
    let token = fresh_token().await?;
    let (status, text) =
        fetch(method, &format!("/api/{path}"), body.as_ref().map(|_| "application/json"), body.map(|b| b.to_string()), Some(&token)).await;
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if (200..300).contains(&status) {
        return Ok(v);
    }
    Err(match (status, v["error"].as_str()) {
        (_, Some(e)) => e.to_string(),
        (0, _) => "Couldn't reach Foliolum's server. Check the internet connection.".into(),
        (502..=504, _) => "Foliolum's server isn't answering right now. Try again in a minute.".into(),
        _ => format!("Foliolum's server said no ({status})."),
    })
}

async fn load_me(ctx: &egui::Context) {
    if state().tokens.is_none() {
        ctx.request_repaint();
        return;
    }
    match api("GET", "me", None).await {
        Ok(v) => match serde_json::from_value::<Me>(v) {
            Ok(me) => {
                let mut st = state();
                st.me = Some(me);
                st.templates = None;
                if let Some(invite) = st.invite.take() {
                    st.want(Want::AcceptInvite(invite));
                }
            }
            Err(e) => state().message = format!("Foliolum's server sent something unexpected: {e}"),
        },
        Err(e) => state().message = e,
    }
    ctx.request_repaint();
}

/// At start: the invite in the address, and the sign-in kept from last time.
pub fn start(ctx: &egui::Context, query: &str) {
    if let Some(invite) = account::invite_in(query) {
        let mut st = state();
        st.invite = Some(invite);
        st.open = true;
        // Take the token out of the address bar, so it isn't bookmarked or shared by accident.
        if let Some(w) = web_sys::window()
            && let Ok(h) = w.history()
        {
            let _ = h.replace_state_with_url(&JsValue::NULL, "", Some("/"));
        }
    }
    let Some(refresh) = storage().and_then(|s| s.get_item(STORE_KEY).ok().flatten()).filter(|r| !r.is_empty()) else { return };
    // Expired at once, so the first call renews it.
    state().tokens = Some(Tokens { id_token: String::new(), expires: 0, refresh_token: refresh });
    let ctx = ctx.clone();
    wasm_bindgen_futures::spawn_local(async move { load_me(&ctx).await });
}

fn busy(words: &str) {
    state().busy = Some(words.into());
}

fn done(ctx: &egui::Context, result: Result<(), String>) {
    let mut st = state();
    st.busy = None;
    if let Err(e) = result {
        st.message = e;
    }
    ctx.request_repaint();
}

/// Starts what the dialogs asked for. `document` is the open document as .designcraft base64, for
/// SaveTemplate.
pub fn run(want: Want, ctx: &egui::Context, document: Option<String>) {
    let ctx = ctx.clone();
    match want {
        Want::SignIn => {
            let id = {
                let mut st = state();
                st.tries += 1;
                st.tries
            };
            match begin_sign_in(id) {
                Ok(p) => {
                    state().waiting = Some(id);
                    wasm_bindgen_futures::spawn_local(finish_sign_in(p, ctx));
                }
                Err(e) => state().message = e,
            }
        }
        Want::SignOut => {
            remember(None);
            let mut st = state();
            let open = st.open;
            *st = State { open, ..State::default() };
            st.message = "Signed out.".into();
        }
        Want::SignUp { email, password } => {
            busy("Making your account…");
            wasm_bindgen_futures::spawn_local(async move {
                let body = json!({"email": email, "password": password, "passwordConfirm": password}).to_string();
                let (status, text) = fetch("POST", account::SIGN_UP, Some("application/json"), Some(body), None).await;
                match account::read_sign_up(status, &text) {
                    Ok(()) => {
                        {
                            let mut st = state();
                            st.busy = None;
                            st.message = "Your account is made. Sign in with it in the Claviger window.".into();
                            st.want(Want::SignIn);
                        }
                        ctx.request_repaint();
                    }
                    Err(e) => done(&ctx, Err(e)),
                }
            });
        }
        Want::CreateParish(name) => {
            busy("Starting the parish…");
            wasm_bindgen_futures::spawn_local(async move {
                let r = api("POST", "parishes", Some(json!({"name": name}))).await.map(|_| ());
                done(&ctx, r);
                load_me(&ctx).await;
            });
        }
        Want::Invite(parish) => {
            busy("Making an invite link…");
            wasm_bindgen_futures::spawn_local(async move {
                let r = api("POST", &format!("parishes/{parish}/invites"), None).await.and_then(|v| {
                    let token = v["token"].as_str().ok_or("No invite came back.")?;
                    state().links.insert(parish, account::invite_link(token));
                    Ok(())
                });
                done(&ctx, r);
            });
        }
        Want::Leave(parish) => {
            busy("Leaving the parish…");
            wasm_bindgen_futures::spawn_local(async move {
                let r = api("DELETE", &format!("parishes/{parish}/membership"), None).await.map(|_| ());
                done(&ctx, r);
                load_me(&ctx).await;
            });
        }
        Want::AcceptInvite(token) => {
            busy("Joining the parish…");
            wasm_bindgen_futures::spawn_local(async move {
                let r = api("POST", "invites/accept", Some(json!({"token": token}))).await.map(|v| {
                    state().message = format!("You're in {}.", v["name"].as_str().unwrap_or("the parish"));
                });
                done(&ctx, r);
                load_me(&ctx).await;
            });
        }
        Want::LoadTemplates => {
            // An empty map at once, so the start screen doesn't ask again every frame.
            state().templates = Some(Default::default());
            wasm_bindgen_futures::spawn_local(async move {
                let parishes = state().parishes().to_vec();
                let mut all = std::collections::BTreeMap::new();
                for p in parishes {
                    match api("GET", &format!("parishes/{}/templates", p.id), None).await {
                        Ok(v) => {
                            let list: Vec<TemplateInfo> = serde_json::from_value(v["templates"].clone()).unwrap_or_default();
                            all.insert(p.id, list);
                        }
                        Err(e) => state().message = e,
                    }
                }
                state().templates = Some(all);
                ctx.request_repaint();
            });
        }
        Want::SaveTemplate { parish, name } => {
            let Some(document) = document else {
                state().message = "Open the document to share first.".into();
                return;
            };
            busy("Saving to the parish…");
            wasm_bindgen_futures::spawn_local(async move {
                let r = api("POST", &format!("parishes/{parish}/templates"), Some(json!({"name": name, "document": document}))).await.map(|_| {
                    let mut st = state();
                    st.message = format!("Saved. Everyone in the parish can open {name} from the start screen.");
                    st.templates = None;
                });
                done(&ctx, r);
            });
        }
        Want::OpenTemplate { id, name } => {
            busy("Opening the template…");
            wasm_bindgen_futures::spawn_local(async move {
                let r = api("GET", &format!("templates/{id}"), None).await.and_then(|v| {
                    let doc = v["document"].as_str().ok_or("The template came back empty.")?.to_string();
                    state().opened = Some((name, doc));
                    Ok(())
                });
                done(&ctx, r);
            });
        }
        Want::DeleteTemplate(id) => {
            busy("Deleting…");
            wasm_bindgen_futures::spawn_local(async move {
                let r = api("DELETE", &format!("templates/{id}"), None).await.map(|_| state().templates = None);
                done(&ctx, r);
            });
        }
    }
}

/// Sends a story's text to be proofread; the suggestions or the reason why not land in `slot`.
pub fn proofread(
    story: u64,
    text: String,
    slot: std::sync::Arc<std::sync::Mutex<Option<Result<crate::proofread::Proof, String>>>>,
    ctx: egui::Context,
) {
    wasm_bindgen_futures::spawn_local(async move {
        let r = api("POST", "proofread", Some(json!({"text": text}))).await.and_then(|v| {
            let s: Vec<crate::proofread::Suggestion> =
                serde_json::from_value(v["suggestions"].clone()).map_err(|e| format!("Foliolum's server sent something unexpected: {e}"))?;
            Ok(crate::proofread::Proof::new(story, text, s))
        });
        *slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(r);
        ctx.request_repaint();
    });
}
