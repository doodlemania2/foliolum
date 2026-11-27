//! The browser half of Send for signature ([`crate::sign`]): Sign's API through `fetch`.
use serde_json::Value;
use wasm_bindgen::JsCast as _;
use wasm_bindgen::JsValue;
use wasm_bindgen_futures::JsFuture;

use crate::sign::{
    BASE, CHANNEL, Outcome, Request, TOKEN, authorize_url, base64url, challenge, create_payload, distribute_body, explain, file_name, owner_page,
    read_callback, read_token,
};

/// A sign-in whose Sign window is open.
pub struct Pending {
    verifier: String,
    state: String,
    reply: js_sys::Promise,
    channel: web_sys::BroadcastChannel,
}

/// Opens Sign's connect page in a popup, listening first on the channel the callback page
/// (`sign/callback.html`) answers on. The popup has to open in the frame that saw the click.
pub fn begin_sign_in() -> Result<Pending, String> {
    let window = web_sys::window().ok_or("no window")?;
    let crypto = window.crypto().map_err(js)?;
    let (mut v, mut s) = ([0u8; 32], [0u8; 16]);
    crypto.get_random_values_with_u8_array(&mut v).map_err(js)?;
    crypto.get_random_values_with_u8_array(&mut s).map_err(js)?;
    let (verifier, state) = (base64url(&v), base64url(&s));
    let channel = web_sys::BroadcastChannel::new(CHANNEL).map_err(js)?;
    let reply = js_sys::Promise::new(&mut |resolve, _| {
        let on_message = wasm_bindgen::closure::Closure::once_into_js(move |e: web_sys::MessageEvent| {
            let _ = resolve.call1(&JsValue::NULL, &e.data());
        });
        channel.set_onmessage(Some(on_message.unchecked_ref()));
    });
    let url = authorize_url(&challenge(&verifier), &state);
    match window.open_with_url_and_target_and_features(&url, "foliolum_sign", "popup,width=520,height=720") {
        Ok(Some(_)) => Ok(Pending { verifier, state, reply, channel }),
        _ => {
            channel.close();
            Err("The browser blocked the TDPdf Sign window. Allow pop-ups for app.foliolum.app, then try again.".into())
        }
    }
}

/// Waits for the callback page's answer and trades its code for a token.
pub async fn finish_sign_in(p: Pending) -> Result<String, String> {
    let data = JsFuture::from(p.reply).await.map_err(js);
    p.channel.close();
    let query = data?.as_string().unwrap_or_default();
    let code = read_callback(&query, &p.state)?;
    let init = web_sys::RequestInit::new();
    init.set_method("POST");
    init.set_mode(web_sys::RequestMode::Cors);
    let headers = web_sys::Headers::new().map_err(js)?;
    headers.set("Content-Type", "application/json").map_err(js)?;
    init.set_headers(&headers);
    init.set_body(&JsValue::from_str(&serde_json::json!({"code": code, "code_verifier": p.verifier}).to_string()));
    let request = web_sys::Request::new_with_str_and_init(TOKEN, &init).map_err(js)?;
    let window = web_sys::window().ok_or("no window")?;
    let response: web_sys::Response = match JsFuture::from(window.fetch_with_request(&request)).await {
        Ok(r) => r.dyn_into().map_err(js)?,
        Err(e) => {
            log::warn!("sign: token: {e:?}");
            return Err(explain(0, ""));
        }
    };
    let text = JsFuture::from(response.text().map_err(js)?).await.map_err(js)?.as_string().unwrap_or_default();
    read_token(response.status(), &text)
}

/// Sends `req` and reports how it went.
pub async fn run(mut req: Request) -> Outcome {
    req.external_id = format!("foliolum-{:016x}", (js_sys::Math::random() * 2f64.powi(53)) as u64);
    match send(&req).await {
        Ok(outcome) => outcome,
        Err(why) => Outcome::Failed(why),
    }
}

async fn send(req: &Request) -> Result<Outcome, String> {
    let form = web_sys::FormData::new().map_err(js)?;
    form.append_with_str("payload", &create_payload(&req.title, &req.external_id, &req.people).to_string()).map_err(js)?;
    let parts = js_sys::Array::of1(&js_sys::Uint8Array::from(req.pdf.as_slice()));
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("application/pdf");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options).map_err(js)?;
    form.append_with_blob_and_filename("files", &blob, &file_name(&req.title)).map_err(js)?;
    let created = call(&req.token, "POST", "envelope/create", Some(&form.into())).await?;
    let id = created.get("id").and_then(Value::as_str).ok_or("TDPdf Sign didn't say which document it made.")?.to_string();
    let page = match call(&req.token, "GET", &format!("envelope/{id}"), None).await {
        Ok(env) => env.pointer("/team/url").and_then(Value::as_str).map(|team| owner_page(team, &id)),
        Err(e) => {
            log::warn!("sign: couldn't read the envelope back: {e}");
            None
        }
    };
    if !req.send {
        return Ok(Outcome::Drafted { page: page.unwrap_or_else(|| format!("{BASE}/documents")) });
    }
    let body = distribute_body(&id, &req.title, &req.message).to_string();
    let sent = call(&req.token, "POST", "envelope/distribute", Some(&JsValue::from_str(&body))).await?;
    let links = sent
        .get("recipients")
        .and_then(Value::as_array)
        .map(|rs| {
            rs.iter()
                .filter_map(|r| {
                    Some((r.get("name").and_then(Value::as_str).unwrap_or_default().to_string(), r.get("signingUrl")?.as_str()?.to_string()))
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(Outcome::Sent { links, page })
}

/// One call to `{BASE}/api/v2/{path}`: a JSON body is sent as JSON, a FormData as multipart.
async fn call(token: &str, method: &str, path: &str, body: Option<&JsValue>) -> Result<Value, String> {
    let init = web_sys::RequestInit::new();
    init.set_method(method);
    init.set_mode(web_sys::RequestMode::Cors);
    let headers = web_sys::Headers::new().map_err(js)?;
    // Documenso takes the token bare, with no "Bearer".
    headers.set("Authorization", token).map_err(js)?;
    headers.set("Accept", "application/json").map_err(js)?;
    if let Some(body) = body {
        if body.is_string() {
            headers.set("Content-Type", "application/json").map_err(js)?;
        }
        init.set_body(body);
    }
    init.set_headers(&headers);
    let request = web_sys::Request::new_with_str_and_init(&format!("{BASE}/api/v2/{path}"), &init).map_err(js)?;
    let window = web_sys::window().ok_or("no window")?;
    let response: web_sys::Response = match JsFuture::from(window.fetch_with_request(&request)).await {
        Ok(r) => r.dyn_into().map_err(js)?,
        Err(e) => {
            log::warn!("sign: {path}: {e:?}");
            return Err(explain(0, ""));
        }
    };
    let text = JsFuture::from(response.text().map_err(js)?).await.map_err(js)?.as_string().unwrap_or_default();
    if !response.ok() {
        return Err(explain(response.status(), &text));
    }
    serde_json::from_str(&text).map_err(|e| format!("TDPdf Sign sent something unexpected: {e}"))
}

fn js(e: JsValue) -> String {
    format!("{e:?}")
}
