//! Send for signature: the open document goes, as a PDF, to TDPdf Sign (sign.tdpdf.com, a Documenso
//! service), which emails each person a link to sign it.
//!
//! The contract is Documenso's API v2, as TDPdf uses it (doodlemania2/TDPdf, `Services/Documenso*.cs`):
//!
//! - `POST /api/v2/envelope/create`, multipart: a `payload` field (JSON: title, type, externalId,
//!   recipients) and the PDF as `files`. With the recipients inline, Sign finds placeholders such as
//!   `{{signature, r1}}` in the PDF's text, whites them out and puts a field there for recipient 1.
//! - `POST /api/v2/envelope/distribute` emails the signers and returns each one's signing link.
//! - `GET /api/v2/envelope/{id}` gives the team whose pages hold the document, for the draft path.
//!
//! Auth is the person's own Sign API token, sent bare in `Authorization` (no `Bearer`). It reaches the
//! whole of their Sign team, so Foliolum keeps it only in memory, for as long as the tab is open,
//! and never writes it anywhere. The person either signs in to Sign in a popup ([`SignIn`], the PKCE
//! connect flow the TDPdf app uses) or pastes a token they made in Sign's settings. Calls go straight from the browser (Sign's API answers CORS with
//! `*`), so the page's CSP allows `connect-src https://sign.tdpdf.com` and nothing else new.
//!
//! When every person has a signature placeholder the document is sent at once. Otherwise it is made
//! as a draft and the dialog links to it in Sign, where the fields are placed by hand and sent.
//! Sign can't be framed (`frame-ancestors 'self'`), so that link opens a new tab.
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

/// TDPdf Sign.
pub const BASE: &str = "https://sign.tdpdf.com";
/// Where a person makes an API token in Sign.
pub const TOKENS_PAGE: &str = "https://sign.tdpdf.com/settings/tokens";
/// Sign's connect page: the person signs in, picks a team and allows Foliolum.
pub const AUTHORIZE: &str = "https://sign.tdpdf.com/connect/tdpdf";
/// Trades the one-use code for a token: POST `{code, code_verifier}`, answers `{token}`.
pub const TOKEN: &str = "https://sign.tdpdf.com/api/connect/token";
/// Where Sign sends the popup back to: a static page (`sign/callback.html`) that hands the query to
/// the editor tab and closes. Sign accepts exactly this address.
pub const REDIRECT: &str = "https://app.foliolum.app/sign/callback";
/// The same-origin channel the callback page posts its query on.
pub const CHANNEL: &str = "foliolum-sign-in";

pub use crate::pkce::{base64url, challenge};

/// The connect page's address for one sign-in.
pub fn authorize_url(challenge: &str, state: &str) -> String {
    let q = form_urlencoded::Serializer::new(String::new())
        .append_pair("code_challenge", challenge)
        .append_pair("state", state)
        .append_pair("redirect_uri", REDIRECT)
        .finish();
    format!("{AUTHORIZE}?{q}")
}

/// What the callback page passed on, checked against the sign-in's `state`: the code, or why not.
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
        return Err(if e == "access_denied" { "Sign-in to TDPdf Sign was cancelled.".into() } else { format!("TDPdf Sign said: {e}") });
    }
    code.filter(|c| !c.is_empty()).ok_or_else(|| "TDPdf Sign didn't send a sign-in code.".into())
}

/// The token from the token endpoint's answer.
pub fn read_token(status: u16, body: &str) -> Result<String, String> {
    if status == 400 {
        return Err("The sign-in code had expired or was already used. Try signing in again.".into());
    }
    if !(200..300).contains(&status) {
        return Err(explain(status, body));
    }
    serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|v| v.get("token")?.as_str().map(str::to_string))
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "TDPdf Sign didn't send a token.".into())
}

/// A finished sign-in: its id, and the token or why not.
pub type SignInSlot = Arc<Mutex<Option<(u32, Result<String, String>)>>>;

/// Signing in to Sign from the dialog. The browser half (`sign_web::sign_in`) writes the result to
/// [`Dialog::sign_in_slot`], tagged with the try's id so a late answer to a cancelled one is ignored.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SignIn {
    #[default]
    Idle,
    Waiting(u32),
    Failed(String),
}

/// One person to sign, in the order the placeholders number them (`r1` is the first).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Person {
    pub name: String,
    pub email: String,
}

/// The placeholders found in the document: for each recipient number (1-based), the kinds of field
/// marked for it (`signature`, `date`, `name`, …), in lower case.
pub type Marks = BTreeMap<usize, BTreeSet<String>>;

/// Finds `{{kind, rN}}` placeholders (spaces optional, any case, extra options after a second comma
/// allowed) in `text`, as Sign does in the PDF.
pub fn find_marks(text: &str, marks: &mut Marks) {
    let mut rest = text;
    while let Some(open) = rest.find("{{") {
        rest = &rest[open + 2..];
        let Some(close) = rest.find("}}") else { break };
        let inner = &rest[..close];
        let mut parts = inner.split(',').map(str::trim);
        if let (Some(kind), Some(who)) = (parts.next(), parts.next())
            && !kind.is_empty()
            && kind.chars().all(|c| c.is_ascii_alphabetic() || c == '_' || c == '-')
            && let Some(n) = who.strip_prefix(['r', 'R']).and_then(|n| n.parse::<usize>().ok())
            && n > 0
        {
            marks.entry(n).or_default().insert(kind.to_ascii_lowercase());
            rest = &rest[close + 2..];
        }
    }
}

/// Every placeholder in the document's stories and table cells.
pub fn document_marks(doc: &designcraft_engine::doc::Document) -> Marks {
    let mut marks = Marks::new();
    for story in doc.stories.values() {
        find_marks(&story.text, &mut marks);
        for table in story.tables.values() {
            for cell in &table.cells {
                find_marks(&cell.text.text, &mut marks);
            }
        }
    }
    marks
}

/// A plausible email address: something@something.something, no spaces.
pub fn looks_like_email(s: &str) -> bool {
    let s = s.trim();
    let Some((user, host)) = s.split_once('@') else { return false };
    !user.is_empty()
        && host.contains('.')
        && !host.starts_with('.')
        && !host.ends_with('.')
        && !s.contains(char::is_whitespace)
        && !host.contains('@')
}

/// What the dialog can do with what's filled in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Every person has a signature placeholder: create and send.
    Send,
    /// Someone has no signature placeholder: create a draft to finish in Sign. Names who.
    Draft(Vec<usize>),
    /// Can't go yet, and why.
    NotReady(String),
}

pub fn plan(people: &[Person], marks: &Marks, title: &str, token: &str) -> Plan {
    if title.trim().is_empty() {
        return Plan::NotReady("Give the document a title.".into());
    }
    if people.is_empty() {
        return Plan::NotReady("Add the people who sign.".into());
    }
    for (i, p) in people.iter().enumerate() {
        if !looks_like_email(&p.email) {
            return Plan::NotReady(format!("Person {} needs an email address.", i + 1));
        }
    }
    if let Some(n) = marks.keys().find(|n| **n > people.len()) {
        return Plan::NotReady(format!(
            "The document marks r{n}, but there {} only {}.",
            if people.len() == 1 { "is" } else { "are" },
            count(people.len())
        ));
    }
    if token.trim().is_empty() {
        return Plan::NotReady("Sign in to TDPdf Sign, or paste an API token.".into());
    }
    let unmarked: Vec<usize> = (1..=people.len()).filter(|n| !marks.get(n).is_some_and(|k| k.contains("signature"))).collect();
    if unmarked.is_empty() { Plan::Send } else { Plan::Draft(unmarked) }
}

fn count(n: usize) -> String {
    if n == 1 { "1 person".into() } else { format!("{n} people") }
}

/// The `payload` field of `envelope/create`: every person a signer, in placeholder order.
pub fn create_payload(title: &str, external_id: &str, people: &[Person]) -> Value {
    let recipients: Vec<Value> = people
        .iter()
        .enumerate()
        .map(|(i, p)| json!({"email": p.email.trim(), "name": p.name.trim(), "role": "SIGNER", "signingOrder": i}))
        .collect();
    json!({"title": title.trim(), "type": "DOCUMENT", "externalId": external_id, "recipients": recipients})
}

/// The body of `envelope/distribute`.
pub fn distribute_body(envelope: &str, subject: &str, message: &str) -> Value {
    let mut meta = json!({"subject": subject.trim(), "distributionMethod": "EMAIL"});
    if !message.trim().is_empty() {
        meta["message"] = json!(message.trim());
    }
    json!({"envelopeId": envelope, "meta": meta})
}

/// Where the sender sees the document in Sign.
pub fn owner_page(team_url: &str, envelope: &str) -> String {
    format!("{BASE}/t/{team_url}/documents/{envelope}")
}

/// A file name for the PDF from the title: letters, digits, spaces, dashes and underscores only.
pub fn file_name(title: &str) -> String {
    let stem: String = title.trim().chars().map(|c| if c.is_alphanumeric() || matches!(c, ' ' | '-' | '_') { c } else { '_' }).take(80).collect();
    format!("{}.pdf", if stem.trim().is_empty() { "document" } else { stem.trim() })
}

/// Why Sign said no, in words for the dialog.
pub fn explain(status: u16, body: &str) -> String {
    let detail = serde_json::from_str::<Value>(body).ok().and_then(|v| v.get("message").and_then(Value::as_str).map(str::to_owned));
    match status {
        401 | 403 => "TDPdf Sign didn't accept the token. Check it, or make a new one in Sign's settings.".into(),
        429 => "TDPdf Sign is busy. Try again in a minute.".into(),
        0 => "Couldn't reach TDPdf Sign. Check the connection and try again.".into(),
        _ => match detail {
            Some(d) => format!("TDPdf Sign said: {d} (HTTP {status})"),
            None => format!("TDPdf Sign answered HTTP {status}."),
        },
    }
}

/// What to send, once the dialog has it all.
#[derive(Clone, Debug)]
pub struct Request {
    pub token: String,
    pub title: String,
    pub message: String,
    pub people: Vec<Person>,
    pub pdf: Vec<u8>,
    /// Send now (every person has a signature placeholder), or leave a draft to finish in Sign.
    pub send: bool,
    pub external_id: String,
}

/// How it went.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Sent: each person's name and signing link, and the sender's page.
    Sent {
        links: Vec<(String, String)>,
        page: Option<String>,
    },
    /// A draft in Sign, to finish there.
    Drafted {
        page: String,
    },
    Failed(String),
}

#[derive(Clone, Debug, Default)]
enum Stage {
    #[default]
    Editing,
    Working,
    Done(Outcome),
}

/// The dialog's state. The token lives here and nowhere else.
#[derive(Default)]
pub struct Dialog {
    pub open: bool,
    title: String,
    message: String,
    people: Vec<Person>,
    token: String,
    /// The token came from signing in, not from the paste box.
    signed_in: bool,
    sign_in: SignIn,
    sign_ins: u32,
    sign_in_result: SignInSlot,
    /// Set when Sign in is pressed; the shell takes it and starts the popup.
    sign_in_wanted: Option<u32>,
    stage: Stage,
    result: Arc<Mutex<Option<Outcome>>>,
}

impl Dialog {
    /// Opens the dialog for the document titled `title`, keeping the token and people from last time.
    pub fn open(&mut self, title: &str) {
        self.open = true;
        self.title = title.trim_end_matches(".pub").trim_end_matches(".designcraft").to_string();
        if self.people.is_empty() {
            self.people.push(Person::default());
        }
        self.stage = Stage::Editing;
    }

    /// Where the finished outcome goes; the sending task writes it there.
    pub fn result_slot(&self) -> Arc<Mutex<Option<Outcome>>> {
        self.result.clone()
    }

    /// Where a sign-in's token (or why not) goes, tagged with the sign-in's id.
    pub fn sign_in_slot(&self) -> SignInSlot {
        self.sign_in_result.clone()
    }

    /// A sign-in the person asked for, once: the shell opens the popup for it.
    pub fn take_sign_in(&mut self) -> Option<u32> {
        self.sign_in_wanted.take()
    }

    fn start_sign_in(&mut self) {
        self.sign_ins += 1;
        self.sign_in = SignIn::Waiting(self.sign_ins);
        self.sign_in_wanted = Some(self.sign_ins);
    }

    fn pick_up_sign_in(&mut self) {
        let Some((id, result)) = self.sign_in_result.lock().unwrap_or_else(|e| e.into_inner()).take() else { return };
        if self.sign_in != SignIn::Waiting(id) {
            return; // cancelled, or an older try
        }
        match result {
            Ok(token) => {
                self.token = token;
                self.signed_in = true;
                self.sign_in = SignIn::Idle;
            }
            Err(why) => self.sign_in = SignIn::Failed(why),
        }
    }

    /// Draws the dialog. Returns a request when the person presses Send or Create draft; the caller
    /// fills in `pdf` and starts it.
    pub fn show(&mut self, ctx: &egui::Context, marks: &Marks) -> Option<Request> {
        if !self.open {
            return None;
        }
        if let Some(outcome) = self.result.lock().unwrap_or_else(|e| e.into_inner()).take() {
            self.stage = Stage::Done(outcome);
        }
        self.pick_up_sign_in();
        let mut open = self.open;
        let mut request = None;
        egui::Window::new("Send for signature")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(460.0)
            .pivot(egui::Align2::CENTER_TOP)
            .default_pos(ctx.content_rect().center_top() + egui::vec2(0.0, 140.0))
            .show(ctx, |ui| match self.stage.clone() {
                Stage::Editing => request = self.form(ui, marks),
                Stage::Working => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Sending to TDPdf Sign…");
                    });
                }
                Stage::Done(outcome) => self.done(ui, &outcome),
            });
        self.open = open;
        if request.is_some() {
            self.stage = Stage::Working;
        }
        request
    }

    fn form(&mut self, ui: &mut egui::Ui, marks: &Marks) -> Option<Request> {
        ui.label("Foliolum sends this document as a PDF to TDPdf Sign, which emails each person a link to sign it.");
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Mark where each person signs by typing {{signature, r1}} in a text box: r1 is the first person below, r2 the second. {{date, r1}} and {{name, r1}} work too. Keep each one on a single line.").small());
        ui.add_space(4.0);
        if marks.is_empty() {
            ui.label(egui::RichText::new("No placeholders in this document yet.").small().italics());
        } else {
            for (n, kinds) in marks {
                ui.label(egui::RichText::new(format!("r{n}: {}", kinds.iter().cloned().collect::<Vec<_>>().join(", "))).small());
            }
        }
        ui.separator();
        egui::Grid::new("sign_form").num_columns(2).spacing([8.0, 6.0]).show(ui, |ui| {
            let l = ui.label("Title");
            ui.add(egui::TextEdit::singleline(&mut self.title).desired_width(300.0).min_size(egui::vec2(300.0, 0.0))).labelled_by(l.id);
            ui.end_row();
            let l = ui.label("Message");
            ui.add(
                egui::TextEdit::multiline(&mut self.message)
                    .desired_rows(2)
                    .desired_width(300.0)
                    .min_size(egui::vec2(300.0, 0.0))
                    .hint_text("Optional, for the email"),
            )
            .labelled_by(l.id);
            ui.end_row();
        });
        ui.add_space(6.0);
        ui.label(egui::RichText::new("Who signs").strong());
        let mut remove = None;
        egui::Grid::new("sign_people").num_columns(4).spacing([6.0, 4.0]).show(ui, |ui| {
            for (i, p) in self.people.iter_mut().enumerate() {
                ui.label(format!("r{}", i + 1));
                let (name, email) = (format!("Name of person r{}", i + 1), format!("Email of person r{}", i + 1));
                ui.add(egui::TextEdit::singleline(&mut p.name).hint_text("Name").desired_width(140.0).min_size(egui::vec2(140.0, 0.0)))
                    .widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::TextInput, true, &name));
                ui.add(egui::TextEdit::singleline(&mut p.email).hint_text("Email").desired_width(180.0).min_size(egui::vec2(180.0, 0.0)))
                    .widget_info(|| egui::WidgetInfo::labeled(egui::accesskit::Role::TextInput, true, &email));
                if ui.small_button("Remove").clicked() {
                    remove = Some(i);
                }
                ui.end_row();
            }
        });
        if let Some(i) = remove {
            self.people.remove(i);
        }
        if ui.button("Add a person").clicked() {
            self.people.push(Person::default());
        }
        ui.separator();
        self.account(ui);
        ui.add_space(8.0);
        let plan = plan(&self.people, marks, &self.title, &self.token);
        let mut go = None;
        match &plan {
            Plan::NotReady(why) => {
                ui.label(egui::RichText::new(why).small());
                ui.add_enabled(false, egui::Button::new("Send"));
            }
            Plan::Send => {
                if ui.button("Send").clicked() {
                    go = Some(true);
                }
            }
            Plan::Draft(who) => {
                let who: Vec<String> = who.iter().map(|n| format!("r{n}")).collect();
                ui.label(
                    egui::RichText::new(format!(
                        "No {{{{signature}}}} placeholder for {}. Foliolum will make a draft in TDPdf Sign, where you place the fields and send it.",
                        who.join(", ")
                    ))
                    .small(),
                );
                if ui.button("Make a draft in TDPdf Sign").clicked() {
                    go = Some(false);
                }
            }
        }
        go.map(|send| Request {
            token: self.token.trim().to_string(),
            title: self.title.trim().to_string(),
            message: self.message.clone(),
            people: self.people.clone(),
            pdf: Vec::new(),
            send,
            external_id: String::new(),
        })
    }

    /// Signing in to Sign, or pasting a token.
    fn account(&mut self, ui: &mut egui::Ui) {
        if self.signed_in && !self.token.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Signed in to TDPdf Sign.");
                if ui.small_button("Sign out").clicked() {
                    self.token.clear();
                    self.signed_in = false;
                }
            });
            ui.label(egui::RichText::new("Foliolum keeps the sign-in only until you close this tab.").small());
            return;
        }
        match self.sign_in.clone() {
            SignIn::Waiting(_) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label("Finish signing in in the TDPdf Sign window…");
                    if ui.small_button("Cancel").clicked() {
                        self.sign_in = SignIn::Idle;
                    }
                });
            }
            state => {
                ui.horizontal(|ui| {
                    if ui.button("Sign in to TDPdf Sign").clicked() {
                        self.start_sign_in();
                    }
                    ui.label(egui::RichText::new("Opens a TDPdf Sign window.").small());
                });
                if let SignIn::Failed(why) = state {
                    ui.label(egui::RichText::new(why).small());
                }
            }
        }
        ui.collapsing("Or paste an API token", |ui| {
            ui.horizontal(|ui| {
                let l = ui.label("API token");
                ui.add(egui::TextEdit::singleline(&mut self.token).password(true).desired_width(220.0)).labelled_by(l.id);
                ui.hyperlink_to("Get one", TOKENS_PAGE);
            });
        });
        ui.label(egui::RichText::new("Foliolum keeps the token only until you close this tab.").small());
    }

    fn done(&mut self, ui: &mut egui::Ui, outcome: &Outcome) {
        match outcome {
            Outcome::Sent { links, page } => {
                ui.label("Sent. TDPdf Sign has emailed each person their link:");
                for (name, url) in links {
                    ui.hyperlink_to(if name.is_empty() { url.as_str() } else { name.as_str() }, url);
                }
                if let Some(page) = page {
                    ui.hyperlink_to("Follow it in TDPdf Sign", page);
                }
            }
            Outcome::Drafted { page } => {
                ui.label("The draft is in TDPdf Sign. Open it to place the fields and send it.");
                ui.hyperlink_to("Open the draft in TDPdf Sign", page);
            }
            Outcome::Failed(why) => {
                ui.label(why);
                if ui.button("Back").clicked() {
                    self.stage = Stage::Editing;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn marks(text: &str) -> Marks {
        let mut m = Marks::new();
        find_marks(text, &mut m);
        m
    }

    #[test]
    fn placeholders_are_found_as_sign_finds_them() {
        let m = marks("Signed: {{signature, r1}}  Date {{ DATE ,R1 }} and {{signature,r2, required=true}} {{nope}} {{x, q1}} {{signature, r0}}");
        assert_eq!(m.len(), 2);
        assert_eq!(m[&1], BTreeSet::from(["signature".to_string(), "date".to_string()]));
        assert_eq!(m[&2], BTreeSet::from(["signature".to_string()]));
        assert!(marks("{{signature, r1").is_empty(), "unclosed");
    }

    fn people(n: usize) -> Vec<Person> {
        (1..=n).map(|i| Person { name: format!("P{i}"), email: format!("p{i}@example.org") }).collect()
    }

    #[test]
    fn it_sends_only_when_every_person_has_a_signature() {
        assert_eq!(plan(&people(2), &marks("{{signature, r1}} {{signature, r2}}"), "Bulletin", "tok"), Plan::Send);
        assert_eq!(plan(&people(2), &marks("{{signature, r1}} {{date, r2}}"), "Bulletin", "tok"), Plan::Draft(vec![2]));
        assert_eq!(plan(&people(1), &Marks::new(), "Bulletin", "tok"), Plan::Draft(vec![1]));
    }

    #[test]
    fn it_says_what_is_missing() {
        let m = marks("{{signature, r3}}");
        assert!(matches!(plan(&people(2), &m, "T", "tok"), Plan::NotReady(w) if w.contains("r3")));
        assert!(matches!(plan(&people(1), &Marks::new(), " ", "tok"), Plan::NotReady(w) if w.contains("title")));
        assert!(matches!(plan(&[], &Marks::new(), "T", "tok"), Plan::NotReady(w) if w.contains("people")));
        let bad = vec![Person { name: "A".into(), email: "a@b".into() }];
        assert!(matches!(plan(&bad, &Marks::new(), "T", "tok"), Plan::NotReady(w) if w.contains("email")));
        assert!(matches!(plan(&people(1), &Marks::new(), "T", "  "), Plan::NotReady(w) if w.contains("token")));
    }

    #[test]
    fn the_payload_lists_every_person_as_a_signer_in_order() {
        let p = create_payload(" Bulletin ", "foliolum-1", &people(2));
        assert_eq!(p["title"], "Bulletin");
        assert_eq!(p["type"], "DOCUMENT");
        assert_eq!(p["externalId"], "foliolum-1");
        assert_eq!(p["recipients"][1], json!({"email": "p2@example.org", "name": "P2", "role": "SIGNER", "signingOrder": 1}));
        let d = distribute_body("env_1", "Bulletin", "");
        assert_eq!(d, json!({"envelopeId": "env_1", "meta": {"subject": "Bulletin", "distributionMethod": "EMAIL"}}));
        assert_eq!(distribute_body("e", "s", " hi ")["meta"]["message"], "hi");
    }

    #[test]
    fn names_links_and_errors() {
        assert_eq!(file_name("Parish / Bulletin: May"), "Parish _ Bulletin_ May.pdf");
        assert_eq!(file_name("  "), "document.pdf");
        assert_eq!(owner_page("personal-abc", "envelope_x"), "https://sign.tdpdf.com/t/personal-abc/documents/envelope_x");
        assert!(explain(401, "").contains("token"));
        assert!(explain(400, r#"{"message":"Invalid email"}"#).contains("Invalid email"));
        assert!(looks_like_email("a.b@c.org") && !looks_like_email("a b@c.org") && !looks_like_email("a@.org") && !looks_like_email("@c.org"));
    }

    #[test]
    fn the_fields_are_wide_enough_to_type_in() {
        let mut d = Dialog::default();
        d.open("Bulletin");
        let marks = Marks::new();
        let widths = crate::profile::tests::field_widths(|ctx| {
            d.show(ctx, &marks);
        });
        assert!(widths.len() >= 4, "{widths:?}");
        assert!(widths[0] >= 135.0, "{widths:?}");
    }

    #[test]
    fn pkce_matches_the_rfc_example() {
        // RFC 7636 appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(challenge(verifier), "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM");
        assert_eq!(base64url(&[0xff; 32]).len(), 43);
        assert_eq!(base64url(b"f"), "Zg");
        assert_eq!(base64url(b"fo"), "Zm8");
        assert_eq!(base64url(b"foo"), "Zm9v");
    }

    #[test]
    fn the_sign_in_address_and_reply() {
        let url = authorize_url("abc", "s1");
        assert_eq!(
            url,
            "https://sign.tdpdf.com/connect/tdpdf?code_challenge=abc&state=s1&redirect_uri=https%3A%2F%2Fapp.foliolum.app%2Fsign%2Fcallback"
        );
        assert_eq!(read_callback("?code=c1&state=s1", "s1"), Ok("c1".into()));
        assert!(read_callback("?code=c1&state=other", "s1").unwrap_err().contains("wasn't for this window"));
        assert!(read_callback("?error=access_denied&state=s1", "s1").unwrap_err().contains("cancelled"));
        assert!(read_callback("?state=s1", "s1").is_err());
        assert_eq!(read_token(200, r#"{"token":"api_x"}"#), Ok("api_x".into()));
        assert!(read_token(400, r#"{"error":"invalid_grant"}"#).unwrap_err().contains("expired"));
        assert!(read_token(200, "{}").is_err());
    }

    #[test]
    fn a_sign_in_fills_the_token_and_a_cancelled_one_is_ignored() {
        let mut d = Dialog::default();
        d.start_sign_in();
        let first = d.take_sign_in().unwrap();
        assert_eq!(d.take_sign_in(), None, "taken once");
        d.sign_in = SignIn::Idle; // Cancel
        *d.sign_in_slot().lock().unwrap() = Some((first, Ok("late".into())));
        d.pick_up_sign_in();
        assert!(d.token.is_empty() && !d.signed_in);
        d.start_sign_in();
        let second = d.take_sign_in().unwrap();
        *d.sign_in_slot().lock().unwrap() = Some((second, Ok("api_ok".into())));
        d.pick_up_sign_in();
        assert_eq!(d.token, "api_ok");
        assert!(d.signed_in && d.sign_in == SignIn::Idle);
        d.start_sign_in();
        let third = d.take_sign_in().unwrap();
        *d.sign_in_slot().lock().unwrap() = Some((third, Err("nope".into())));
        d.pick_up_sign_in();
        assert_eq!(d.sign_in, SignIn::Failed("nope".into()));
    }

    #[test]
    fn placeholders_in_the_document_are_found() {
        use designcraft_engine::doc::{Document, ParaFormat, SpreadRef, build::NewDocument};
        let mut d = Document::new(&NewDocument { pages: 1, ..Default::default() });
        let layer = d.default_layer();
        let rect = designcraft_geom::Rect::new(10.0, 10.0, 200.0, 40.0);
        d.add_text_frame(SpreadRef::Doc(0), rect, layer, "Sign here {{signature, r1}}", ParaFormat::default()).unwrap();
        assert!(document_marks(&d)[&1].contains("signature"));
    }
}
