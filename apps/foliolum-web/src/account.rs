//! Signing in, parishes and shared templates (#60).
//!
//! People sign in through Claviger, the fleet's sign-in service (OIDC client `foliolum-app`, PKCE,
//! no secret): a popup opens Claviger's page, which returns it to `/auth/callback` (a static page
//! that passes the answer to this tab over a same-origin BroadcastChannel and closes), and this tab
//! trades the code for tokens. The editor tab never navigates, so an open document is never at risk.
//! Claviger's page signs people in but doesn't make accounts, so "Create account" posts to
//! Claviger's user collection first and then signs in the same way.
//!
//! The ID token goes to Foliolum's backend (`/api/`, services/foliolum-api), which keeps parishes,
//! who is in them, invite links and the templates a parish shares. The refresh token is kept in
//! this browser (local storage `foliolum.session`) so a reload stays signed in, as chores does.
//!
//! This module holds what can be tested without a browser: the protocol's addresses and answers,
//! the shared [`State`], and the dialogs. `account_web.rs` makes the calls.
use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, OnceLock};

use serde::Deserialize;
use serde_json::Value;

pub const AUTHORIZE: &str = "https://sso.claviger.app/oidc/authorize";
pub const TOKEN: &str = "https://sso.claviger.app/oidc/oauth/token";
/// Claviger's user collection for Foliolum, where "Create account" posts (sign-up is open).
pub const SIGN_UP: &str = "https://sso.claviger.app/api/collections/foliolum_users/records";
pub const CLIENT_ID: &str = "foliolum-app";
pub const SCOPES: &str = "openid profile email offline_access";
/// Claviger accepts exactly this address.
pub const REDIRECT: &str = "https://app.foliolum.app/auth/callback";
/// The channel `/auth/callback` posts its query on.
pub const CHANNEL: &str = "foliolum-auth";
/// Where an invite link points: the editor, with `#invite=<token>`.
pub const APP: &str = "https://app.foliolum.app/";
pub const MIN_PASSWORD: usize = 8;

/// Claviger's sign-in page for one sign-in.
pub fn authorize_url(challenge: &str, state: &str, nonce: &str) -> String {
    let q = form_urlencoded::Serializer::new(String::new())
        .append_pair("response_type", "code")
        .append_pair("client_id", CLIENT_ID)
        .append_pair("redirect_uri", REDIRECT)
        .append_pair("scope", SCOPES)
        .append_pair("state", state)
        .append_pair("nonce", nonce)
        .append_pair("code_challenge", challenge)
        .append_pair("code_challenge_method", "S256")
        .finish();
    format!("{AUTHORIZE}?{q}")
}

/// The token request that trades `code` (form-encoded, as OAuth wants).
pub fn code_request(code: &str, verifier: &str) -> String {
    form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "authorization_code")
        .append_pair("code", code)
        .append_pair("redirect_uri", REDIRECT)
        .append_pair("client_id", CLIENT_ID)
        .append_pair("code_verifier", verifier)
        .finish()
}

/// The token request that renews a sign-in.
pub fn refresh_request(refresh_token: &str) -> String {
    form_urlencoded::Serializer::new(String::new())
        .append_pair("grant_type", "refresh_token")
        .append_pair("refresh_token", refresh_token)
        .append_pair("client_id", CLIENT_ID)
        .finish()
}

/// The code from what `/auth/callback` passed on, checked against the sign-in's `state`.
pub fn read_callback(query: &str, state: &str) -> Result<String, String> {
    let (mut code, mut got, mut error) = (None, None, None);
    for (k, v) in form_urlencoded::parse(query.trim_start_matches('?').as_bytes()) {
        match &*k {
            "code" => code = Some(v.into_owned()),
            "state" => got = Some(v.into_owned()),
            "error" => error = Some(v.into_owned()),
            _ => {}
        }
    }
    if got.as_deref() != Some(state) {
        return Err("That sign-in reply wasn't for this window. Try signing in again.".into());
    }
    if let Some(e) = error {
        return Err(if e == "access_denied" { "Signing in was cancelled.".into() } else { format!("Claviger said: {e}") });
    }
    code.filter(|c| !c.is_empty()).ok_or_else(|| "Claviger didn't send a sign-in code.".into())
}

/// A signed-in session: the ID token the backend checks, when it runs out, and the token that renews it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tokens {
    pub id_token: String,
    /// Seconds since 1970.
    pub expires: i64,
    pub refresh_token: String,
}

/// The token endpoint's answer. A renewal may leave out the refresh token (then the old one stays).
pub fn read_tokens(status: u16, body: &str, old_refresh: &str) -> Result<Tokens, String> {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    if !(200..300).contains(&status) {
        return Err(match v["error"].as_str() {
            Some("invalid_grant") => "Your sign-in has run out. Sign in again.".into(),
            _ if status == 0 => "Couldn't reach Claviger. Check the internet connection.".into(),
            _ => format!("Claviger didn't sign you in ({status})."),
        });
    }
    let id_token = v["id_token"].as_str().filter(|t| !t.is_empty()).ok_or("Claviger sent no ID token.")?.to_string();
    let expires = token_expiry(&id_token).ok_or("Claviger's ID token has no expiry.")?;
    let refresh_token = v["refresh_token"].as_str().filter(|t| !t.is_empty()).unwrap_or(old_refresh).to_string();
    Ok(Tokens { id_token, expires, refresh_token })
}

/// The `exp` of a JWT (unverified: the backend verifies; this only says when to renew).
pub fn token_expiry(jwt: &str) -> Option<i64> {
    let payload = jwt.split('.').nth(1)?;
    let v: Value = serde_json::from_slice(&crate::pkce::base64url_decode(payload)?).ok()?;
    v["exp"].as_i64()
}

/// Whether to renew before using `t` at `now`: a minute's margin.
pub fn needs_refresh(t: &Tokens, now: i64) -> bool {
    t.expires - now < 60
}

/// Checks a new account's details before anything is sent.
pub fn check_sign_up(email: &str, password: &str, again: &str) -> Result<(), String> {
    let email = email.trim();
    if !crate::sign::looks_like_email(email) {
        return Err("Type your email address.".into());
    }
    if password.chars().count() < MIN_PASSWORD {
        return Err(format!("Choose a password of at least {MIN_PASSWORD} characters."));
    }
    if password != again {
        return Err("The two passwords aren't the same.".into());
    }
    Ok(())
}

/// What Claviger said to a new account: fine, or why not, in plain words.
pub fn read_sign_up(status: u16, body: &str) -> Result<(), String> {
    if (200..300).contains(&status) {
        return Ok(());
    }
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let field = |f: &str| v["data"][f]["code"].as_str().map(str::to_string);
    if let Some(code) = field("email") {
        return Err(if code.contains("unique") || code.contains("exists") {
            "There's already an account with that email. Sign in instead.".into()
        } else {
            "Claviger didn't accept that email address.".into()
        });
    }
    if field("password").is_some() || field("passwordConfirm").is_some() {
        return Err(format!("Claviger didn't accept that password. Use at least {MIN_PASSWORD} characters."));
    }
    Err(if status == 0 {
        "Couldn't reach Claviger. Check the internet connection.".into()
    } else {
        format!("Claviger didn't make the account ({status}).")
    })
}

/// The invite token in the address the editor was opened with: after the `#` (`#invite=…`), which
/// the browser never sends to a server, so the token stays out of access logs; `?invite=…` is read
/// too. `part` is the location's hash or search.
pub fn invite_in(part: &str) -> Option<String> {
    form_urlencoded::parse(part.trim_start_matches(['?', '#']).as_bytes())
        .find(|(k, _)| k == "invite")
        .map(|(_, v)| v.trim().to_string())
        .filter(|v| !v.is_empty() && v.len() <= 100 && v.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_'))
}

pub fn invite_link(token: &str) -> String {
    format!("{APP}#invite={token}")
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Parish {
    pub id: String,
    pub name: String,
    pub role: String,
}

impl Parish {
    pub fn is_admin(&self) -> bool {
        self.role == "admin"
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct Me {
    pub sub: String,
    #[serde(default)]
    pub email: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub parishes: Vec<Parish>,
    #[serde(default)]
    pub proofreading: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize)]
pub struct TemplateInfo {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub saved_by: String,
    #[serde(default)]
    pub updated: i64,
    #[serde(default)]
    pub size: i64,
}

/// What the dialogs ask the browser half to do; `WebShell` drains these each frame.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Want {
    SignIn,
    SignUp {
        email: String,
        password: String,
    },
    SignOut,
    CreateParish(String),
    Invite(String),
    Leave(String),
    AcceptInvite(String),
    LoadTemplates,
    /// Save the open document to a parish under a name (the shell adds the bytes).
    SaveTemplate {
        parish: String,
        name: String,
    },
    OpenTemplate {
        id: String,
        name: String,
    },
    DeleteTemplate(String),
}

/// Everything the dialogs show and the browser half updates. One per tab ([`state`]).
#[derive(Default)]
pub struct State {
    pub tokens: Option<Tokens>,
    pub me: Option<Me>,
    /// A call is in flight; its words for the person.
    pub busy: Option<String>,
    /// The last thing that went wrong, or a note, for the dialog.
    pub message: String,
    /// The sign-in popup is open (its try's id, so a cancelled one's late answer is ignored).
    pub waiting: Option<u32>,
    pub tries: u32,
    /// An invite from the address, to accept once signed in.
    pub invite: Option<String>,
    /// Invite links made in this tab, by parish id.
    pub links: BTreeMap<String, String>,
    /// Each parish's templates, by parish id; `None` until loaded.
    pub templates: Option<BTreeMap<String, Vec<TemplateInfo>>>,
    /// A template fetched to open: (name, base64 of the .designcraft file).
    pub opened: Option<(String, String)>,
    pub wants: Vec<Want>,
    /// The account dialog is showing.
    pub open: bool,
}

impl State {
    pub fn signed_in(&self) -> bool {
        self.tokens.is_some()
    }

    pub fn want(&mut self, w: Want) {
        if !self.wants.contains(&w) {
            self.wants.push(w);
        }
    }

    pub fn parishes(&self) -> &[Parish] {
        self.me.as_ref().map(|m| m.parishes.as_slice()).unwrap_or_default()
    }
}

/// This tab's account state.
pub fn state() -> MutexGuard<'static, State> {
    static S: OnceLock<Mutex<State>> = OnceLock::new();
    S.get_or_init(Mutex::default).lock().unwrap_or_else(|e| e.into_inner())
}

/// The account dialog's own fields (what is being typed).
#[derive(Default)]
pub struct Dialog {
    creating: bool,
    email: String,
    password: String,
    again: String,
    parish: String,
    local: String,
}

fn field(ui: &mut egui::Ui, label: &str, text: &mut String, password: bool) {
    ui.horizontal(|ui| {
        let l = ui.add_sized([130.0, 20.0], egui::Label::new(label));
        ui.add(egui::TextEdit::singleline(text).password(password).desired_width(240.0).min_size(egui::vec2(240.0, 0.0))).labelled_by(l.id);
    });
}

impl Dialog {
    /// Draws "Your account and parish" while [`State::open`].
    pub fn show(&mut self, ctx: &egui::Context) {
        let mut st = state();
        if !st.open {
            return;
        }
        let mut open = true;
        let at = ctx.content_rect().center_top() + egui::vec2(0.0, 140.0);
        egui::Window::new("Your account and parish")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(440.0)
            .pivot(egui::Align2::CENTER_TOP)
            .default_pos(at)
            .show(ctx, |ui| {
                if st.signed_in() {
                    self.signed_in(ui, &mut st);
                } else {
                    self.signed_out(ui, &mut st);
                }
                if let Some(b) = &st.busy {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(b.as_str());
                    });
                }
                for m in [&st.message, &self.local] {
                    if !m.is_empty() {
                        ui.label(egui::RichText::new(m.as_str()).small());
                    }
                }
            });
        st.open = open;
    }

    fn signed_out(&mut self, ui: &mut egui::Ui, st: &mut State) {
        ui.label("Sign in to share templates with your parish and to proofread with AI. Foliolum works without an account too.");
        if st.invite.is_some() {
            ui.label(egui::RichText::new("You've been invited to a parish. Sign in or create an account to join it.").strong());
        }
        ui.add_space(6.0);
        if let Some(id) = st.waiting {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Finish signing in in the Claviger window…");
                if ui.small_button("Cancel").clicked() && st.waiting == Some(id) {
                    st.waiting = None;
                }
            });
            return;
        }
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.creating, false, "I have an account");
            ui.selectable_value(&mut self.creating, true, "Create an account");
        });
        ui.separator();
        if !self.creating {
            if ui.button("Sign in with Claviger").clicked() {
                st.message.clear();
                st.want(Want::SignIn);
            }
            ui.label(egui::RichText::new("Opens Claviger, our sign-in service, in a small window.").small());
            return;
        }
        field(ui, "Email", &mut self.email, false);
        field(ui, "Password", &mut self.password, true);
        field(ui, "Password again", &mut self.again, true);
        ui.label(
            egui::RichText::new(format!(
                "At least {MIN_PASSWORD} characters. After the account is made, Claviger asks for this password once more to sign you in."
            ))
            .small(),
        );
        if ui.add_enabled(st.busy.is_none(), egui::Button::new("Create account")).clicked() {
            match check_sign_up(&self.email, &self.password, &self.again) {
                Ok(()) => {
                    self.local.clear();
                    st.message.clear();
                    st.want(Want::SignUp { email: self.email.trim().to_string(), password: std::mem::take(&mut self.password) });
                    self.again.clear();
                }
                Err(why) => self.local = why,
            }
        }
    }

    fn signed_in(&mut self, ui: &mut egui::Ui, st: &mut State) {
        let (name, email) = st.me.as_ref().map(|m| (m.name.clone(), m.email.clone())).unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label(if name.is_empty() { "Signed in.".to_string() } else { format!("Signed in as {name}.") });
            if !email.is_empty() && email != name {
                ui.label(egui::RichText::new(email).small());
            }
            if ui.small_button("Sign out").clicked() {
                st.want(Want::SignOut);
            }
        });
        ui.separator();
        let parishes = st.parishes().to_vec();
        if parishes.is_empty() {
            ui.label("You're not in a parish yet. Start one for your parish office, or ask its admin for an invite link.");
        }
        for p in &parishes {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&p.name).strong());
                ui.label(egui::RichText::new(if p.is_admin() { "admin" } else { "member" }).small());
                if p.is_admin() && ui.small_button("Invite people").clicked() {
                    st.want(Want::Invite(p.id.clone()));
                }
                if ui.small_button("Leave").clicked() {
                    st.want(Want::Leave(p.id.clone()));
                }
            });
            if let Some(link) = st.links.get(&p.id).cloned() {
                ui.horizontal(|ui| {
                    let mut shown = link.clone();
                    ui.add(egui::TextEdit::singleline(&mut shown).desired_width(300.0).min_size(egui::vec2(300.0, 0.0)))
                        .widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::TextInput, true, "Invite link"));
                    if ui.small_button("Copy").clicked() {
                        ui.ctx().copy_text(link.clone());
                    }
                });
                ui.label(egui::RichText::new("Send this link to the people who should join. It works for 14 days, for up to 25 people.").small());
            }
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let l = ui.label("Start a parish");
            ui.add(
                egui::TextEdit::singleline(&mut self.parish)
                    .hint_text("St Brigid's, Riverside")
                    .desired_width(220.0)
                    .min_size(egui::vec2(220.0, 0.0)),
            )
            .labelled_by(l.id);
            if ui.add_enabled(!self.parish.trim().is_empty() && st.busy.is_none(), egui::Button::new("Start")).clicked() {
                st.want(Want::CreateParish(std::mem::take(&mut self.parish).trim().to_string()));
            }
        });
    }
}

/// "Save as parish template": which parish and what name.
#[derive(Default)]
pub struct SaveDialog {
    pub open: bool,
    parish: String,
    name: String,
}

impl SaveDialog {
    pub fn open_for(&mut self, title: &str) {
        self.open = true;
        self.name = title.trim_end_matches(".pub").trim_end_matches(".designcraft").to_string();
    }

    /// Draws the dialog; returns (parish id, name) when the person presses Save.
    pub fn show(&mut self, ctx: &egui::Context) -> Option<(String, String)> {
        if !self.open {
            return None;
        }
        let mut st = state();
        let mut open = true;
        let mut out = None;
        egui::Window::new("Share with your parish").open(&mut open).collapsible(false).resizable(false).default_width(420.0).show(ctx, |ui| {
            if !st.signed_in() {
                ui.label("Sign in first: parish templates are shared through your parish.");
                if ui.button("Sign in…").clicked() {
                    st.open = true;
                }
                return;
            }
            let parishes = st.parishes().to_vec();
            if parishes.is_empty() {
                ui.label("Start or join a parish first.");
                if ui.button("Your parish…").clicked() {
                    st.open = true;
                }
                return;
            }
            if !parishes.iter().any(|p| p.id == self.parish) {
                self.parish = parishes[0].id.clone();
            }
            ui.label("Everyone in the parish can open it from the start screen. Saving again under the same name replaces it.");
            ui.add_space(4.0);
            if parishes.len() > 1 {
                ui.horizontal(|ui| {
                    ui.label("Parish");
                    let current = parishes.iter().find(|p| p.id == self.parish).map(|p| p.name.clone()).unwrap_or_default();
                    egui::ComboBox::from_id_salt("template_parish").selected_text(current).show_ui(ui, |ui| {
                        for p in &parishes {
                            ui.selectable_value(&mut self.parish, p.id.clone(), &p.name);
                        }
                    });
                });
            }
            field(ui, "Template name", &mut self.name, false);
            if ui.add_enabled(!self.name.trim().is_empty() && st.busy.is_none(), egui::Button::new("Save to the parish")).clicked() {
                out = Some((self.parish.clone(), self.name.trim().to_string()));
            }
            if let Some(b) = &st.busy {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(b.as_str());
                });
            }
            if !st.message.is_empty() {
                ui.label(egui::RichText::new(st.message.as_str()).small());
            }
        });
        self.open = open;
        out
    }
}

/// The start screen's parish section: sign in, or the parish's shared templates.
pub fn start_section(ui: &mut egui::Ui) {
    let mut st = state();
    let t = designcraft_ui_egui::theme::Tokens::get(ui.ctx());
    ui.add_space(16.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("Your parish's templates").size(15.0).strong().color(t.text_strong));
        let label = if st.signed_in() { "Your parish…" } else { "Sign in…" };
        if ui.small_button(label).clicked() {
            st.open = true;
        }
    });
    if !st.signed_in() {
        ui.label(egui::RichText::new("Sign in to open the templates your parish shares.").small());
        return;
    }
    let parishes = st.parishes().to_vec();
    if parishes.is_empty() {
        ui.label(egui::RichText::new("Start or join a parish to share templates.").small());
        return;
    }
    let Some(all) = st.templates.clone() else {
        st.want(Want::LoadTemplates);
        ui.spinner();
        return;
    };
    let mut any = false;
    for p in &parishes {
        let list = all.get(&p.id).cloned().unwrap_or_default();
        if list.is_empty() {
            continue;
        }
        any = true;
        if parishes.len() > 1 {
            ui.label(egui::RichText::new(&p.name).strong());
        }
        ui.horizontal_wrapped(|ui| {
            for t in &list {
                let by = if t.saved_by.is_empty() { String::new() } else { format!("Saved by {}", t.saved_by) };
                let r = ui.button(&t.name).on_hover_text(by);
                if r.clicked() {
                    st.want(Want::OpenTemplate { id: t.id.clone(), name: t.name.clone() });
                }
                if (p.is_admin() || st.me.as_ref().is_some_and(|m| m.name == t.saved_by))
                    && ui.small_button("Delete").on_hover_text(format!("Delete {}", t.name)).clicked()
                {
                    st.want(Want::DeleteTemplate(t.id.clone()));
                }
            }
        });
    }
    if !any {
        ui.label(egui::RichText::new("No shared templates yet. Open a document and choose File > Share with parish.").small());
    }
    if ui.small_button("Refresh").clicked() {
        st.want(Want::LoadTemplates);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jwt(exp: i64) -> String {
        let p = crate::pkce::base64url(format!(r#"{{"sub":"a","exp":{exp}}}"#).as_bytes());
        format!("eyJhbGciOiJSUzI1NiJ9.{p}.sig")
    }

    #[test]
    fn the_sign_in_addresses() {
        let url = authorize_url("ch", "st", "no");
        assert!(url.starts_with("https://sso.claviger.app/oidc/authorize?response_type=code&client_id=foliolum-app&redirect_uri=https%3A%2F%2Fapp.foliolum.app%2Fauth%2Fcallback"));
        assert!(url.contains("&scope=openid+profile+email+offline_access&state=st&nonce=no&code_challenge=ch&code_challenge_method=S256"));
        assert_eq!(
            code_request("c1", "v1"),
            "grant_type=authorization_code&code=c1&redirect_uri=https%3A%2F%2Fapp.foliolum.app%2Fauth%2Fcallback&client_id=foliolum-app&code_verifier=v1"
        );
        assert_eq!(refresh_request("r+1"), "grant_type=refresh_token&refresh_token=r%2B1&client_id=foliolum-app");
        assert_eq!(read_callback("?code=c&state=s", "s"), Ok("c".into()));
        assert!(read_callback("?code=c&state=x", "s").is_err());
        assert!(read_callback("?error=access_denied&state=s", "s").unwrap_err().contains("cancelled"));
    }

    #[test]
    fn tokens_are_read_and_renewed_in_time() {
        let t = read_tokens(200, &format!(r#"{{"id_token":"{}","refresh_token":"r2","access_token":"a"}}"#, jwt(1000)), "r1").unwrap();
        assert_eq!((t.expires, t.refresh_token.as_str()), (1000, "r2"));
        let kept = read_tokens(200, &format!(r#"{{"id_token":"{}"}}"#, jwt(1000)), "r1").unwrap();
        assert_eq!(kept.refresh_token, "r1", "a renewal without a new refresh token keeps the old one");
        assert!(read_tokens(400, r#"{"error":"invalid_grant"}"#, "").unwrap_err().contains("run out"));
        assert!(read_tokens(200, r#"{"access_token":"a"}"#, "").is_err());
        assert!(!needs_refresh(&t, 900));
        assert!(needs_refresh(&t, 950));
        assert_eq!(token_expiry("not-a-jwt"), None);
    }

    #[test]
    fn sign_up_is_checked_and_answered_in_plain_words() {
        assert!(check_sign_up("a@b.org", "12345678", "12345678").is_ok());
        assert!(check_sign_up("nope", "12345678", "12345678").unwrap_err().contains("email"));
        assert!(check_sign_up("a@b.org", "short", "short").unwrap_err().contains("8"));
        assert!(check_sign_up("a@b.org", "12345678", "12345679").unwrap_err().contains("same"));
        assert_eq!(read_sign_up(200, "{}"), Ok(()));
        let taken = r#"{"status":400,"message":"Failed to create record.","data":{"email":{"code":"validation_not_unique","message":"Value must be unique."}}}"#;
        assert!(read_sign_up(400, taken).unwrap_err().contains("already an account"));
        let weak = r#"{"data":{"password":{"code":"validation_length_out_of_range"}}}"#;
        assert!(read_sign_up(400, weak).unwrap_err().contains("password"));
    }

    #[test]
    fn invite_links_round_trip_and_junk_is_ignored() {
        let link = invite_link("Ab_9-x");
        assert_eq!(link, "https://app.foliolum.app/#invite=Ab_9-x");
        assert_eq!(invite_in(link.split_once('#').map(|(_, q)| q).unwrap_or_default()), Some("Ab_9-x".into()));
        assert_eq!(invite_in("#invite=Ab_9-x"), Some("Ab_9-x".into()));
        assert_eq!(invite_in("?invite=Ab_9-x"), Some("Ab_9-x".into()), "older links still work");
        assert_eq!(invite_in("?demo=bulletin&invite=%3Cscript%3E"), None);
        assert_eq!(invite_in("?demo=bulletin"), None);
    }

    #[test]
    fn the_dialogs_draw_with_named_fields_of_a_usable_width() {
        let mut d = Dialog { creating: true, ..Default::default() };
        state().open = true;
        let widths = crate::profile::tests::field_widths(|ctx| d.show(ctx));
        assert_eq!(widths.len(), 3, "{widths:?}");
        assert!(widths[0] >= 235.0, "{widths:?}");
        {
            let mut st = state();
            st.tokens = Some(Tokens { id_token: jwt(9_999_999_999), expires: 9_999_999_999, refresh_token: "r".into() });
            st.me = Some(Me {
                sub: "a".into(),
                name: "Ann".into(),
                parishes: vec![Parish { id: "p1".into(), name: "St Brigid's".into(), role: "admin".into() }],
                ..Default::default()
            });
            st.links.insert("p1".into(), invite_link("tok"));
        }
        let widths = crate::profile::tests::field_widths(|ctx| d.show(ctx));
        assert_eq!(widths.len(), 2, "the invite link and the new parish's name: {widths:?}");
        let mut save = SaveDialog::default();
        save.open_for("Bulletin.designcraft");
        assert_eq!(save.name, "Bulletin");
        let widths = crate::profile::tests::field_widths(|ctx| {
            save.show(ctx);
        });
        assert_eq!(widths.len(), 1, "{widths:?}");
        *state() = State::default();
    }
}
