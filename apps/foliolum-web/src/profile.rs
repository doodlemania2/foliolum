//! Organization profile (#53): the name, address, phone, email and website a parish or group types
//! once, kept in this browser, and inserted into any document as fields that stay current.
//!
//! Each detail is a DesignCraft custom text variable named for it ("Organization name", …): Insert
//! places the variable at the text cursor, and saving the profile redefines every one of them the
//! open document already uses, so each place it appears updates at once.
use designcraft_engine::Session;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// Where the browser keeps the profile.
pub const STORAGE_KEY: &str = "foliolum.profile";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Profile {
    pub name: String,
    pub address: String,
    pub phone: String,
    pub email: String,
    pub website: String,
}

/// The details, in the order the dialog shows them: (label, variable name).
pub const FIELDS: [(&str, &str); 5] = [
    ("Name", "Organization name"),
    ("Address", "Organization address"),
    ("Phone", "Organization phone"),
    ("Email", "Organization email"),
    ("Website", "Organization website"),
];

impl Profile {
    pub fn get(&self, i: usize) -> &str {
        [&self.name, &self.address, &self.phone, &self.email, &self.website][i]
    }

    pub fn get_mut(&mut self, i: usize) -> &mut String {
        match i {
            0 => &mut self.name,
            1 => &mut self.address,
            2 => &mut self.phone,
            3 => &mut self.email,
            _ => &mut self.website,
        }
    }

    pub fn decode(stored: Option<&str>) -> Profile {
        stored.and_then(|s| serde_json::from_str(s).ok()).unwrap_or_default()
    }
}

/// The names of the text variables the open document defines.
fn defined(s: &mut Session) -> Vec<String> {
    s.execute("variables.list", &json!({}))
        .ok()
        .and_then(|v| v.as_array().map(|a| a.iter().filter_map(|x| x["name"].as_str().map(str::to_string)).collect()))
        .unwrap_or_default()
}

fn define(s: &mut Session, name: &str, text: &str) -> Result<(), String> {
    // An empty detail still needs a value, or the field would vanish; a space keeps its place.
    let text = if text.trim().is_empty() { " " } else { text.trim() };
    s.execute("variables.define", &json!({"name": name, "type": "custom", "text": text})).map(|_| ()).map_err(|e| e.to_string())
}

/// Updates every profile field the open document uses. Returns how many it updated.
pub fn apply(s: &mut Session, p: &Profile) -> Result<usize, String> {
    if s.doc().is_err() {
        return Ok(0);
    }
    crate::undo::as_one_step(s, "Update Details", |s| apply_steps(s, p))
}

fn apply_steps(s: &mut Session, p: &Profile) -> Result<usize, String> {
    let have = defined(s);
    let mut n = 0;
    for (i, (_, var)) in FIELDS.iter().enumerate() {
        if have.iter().any(|h| h == var) {
            define(s, var, p.get(i))?;
            n += 1;
        }
    }
    Ok(n)
}

/// Inserts detail `i` at the text cursor, as a field.
pub fn insert(s: &mut Session, p: &Profile, i: usize) -> Result<(), String> {
    let has_caret = s.doc().map(|d| d.selection.text.is_some()).unwrap_or(false);
    if !has_caret {
        return Err("Click in a text box where the detail should go, then insert it.".into());
    }
    let var = FIELDS[i].1;
    crate::undo::as_one_step(s, "Insert Detail", |s| {
        define(s, var, p.get(i))?;
        s.execute("variables.insert", &json!({"name": var})).map(|_| ()).map_err(|e| e.to_string())
    })
}

/// The Our details dialog.
#[derive(Default)]
pub struct Dialog {
    pub open: bool,
    pub profile: Profile,
    message: String,
}

/// What the dialog asks of the page.
pub enum Want {
    /// Store the profile and update the document's fields.
    Save(Profile),
    /// Insert detail `i` at the cursor (the profile is saved first).
    Insert(Profile, usize),
}

impl Dialog {
    pub fn set_message(&mut self, m: String) {
        self.message = m;
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<Want> {
        if !self.open {
            return None;
        }
        let mut open = self.open;
        let mut want = None;
        egui::Window::new("Our details").open(&mut open).collapsible(false).resizable(false).default_width(420.0).show(ctx, |ui| {
            ui.label(
                "Type your details once. Insert puts one where the text cursor is, and changing it here later changes it everywhere it was inserted.",
            );
            ui.add_space(4.0);
            egui::Grid::new("profile_fields").num_columns(3).spacing([8.0, 6.0]).show(ui, |ui| {
                for (i, (label, _)) in FIELDS.iter().enumerate() {
                    ui.label(*label);
                    let edit = if i == 1 {
                        egui::TextEdit::multiline(self.profile.get_mut(i)).desired_rows(2).desired_width(260.0)
                    } else {
                        egui::TextEdit::singleline(self.profile.get_mut(i)).desired_width(260.0)
                    };
                    ui.add(edit);
                    if ui.button("Insert").clicked() {
                        want = Some(Want::Insert(self.profile.clone(), i));
                    }
                    ui.end_row();
                }
            });
            ui.label(egui::RichText::new("Kept in this browser only.").small());
            if !self.message.is_empty() {
                ui.label(egui::RichText::new(&self.message).small());
            }
            ui.add_space(6.0);
            if ui.button("Save").clicked() {
                want = Some(Want::Save(self.profile.clone()));
            }
        });
        self.open = open;
        want
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session() -> Session {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let r = s.execute("frame.create", &json!({"rect": [36, 36, 300, 300], "content": "text", "text": "Call "})).unwrap();
        let sid = s.doc().unwrap().doc.item(designcraft_engine::doc::ItemId(r["id"].as_u64().unwrap())).unwrap().text_frame().unwrap().story.0;
        s.execute("text.select", &json!({"story": sid, "anchor": 5})).unwrap();
        s
    }

    fn value(s: &mut Session, name: &str) -> Option<String> {
        s.execute("variables.list", &json!({}))
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == name)
            .map(|v| v["value"].as_str().unwrap_or("").to_string())
    }

    #[test]
    fn an_inserted_detail_updates_when_the_profile_changes() {
        let mut s = session();
        let mut p = Profile { name: "St. Example Parish".into(), phone: "555-0142".into(), ..Default::default() };
        insert(&mut s, &p, 2).unwrap();
        assert_eq!(value(&mut s, "Organization phone").as_deref(), Some("555-0142"));
        p.phone = "555-0199".into();
        assert_eq!(apply(&mut s, &p).unwrap(), 1, "only the field in use");
        assert_eq!(value(&mut s, "Organization phone").as_deref(), Some("555-0199"));
        assert_eq!(value(&mut s, "Organization name"), None, "not inserted, not defined");
    }

    #[test]
    fn inserting_needs_a_text_cursor_and_the_profile_round_trips() {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        assert!(insert(&mut s, &Profile::default(), 0).unwrap_err().contains("Click in a text box"));
        let p = Profile { name: "A".into(), address: "1 Example Road\nSampletown".into(), ..Default::default() };
        assert_eq!(Profile::decode(Some(&serde_json::to_string(&p).unwrap())), p);
        assert_eq!(Profile::decode(Some("nonsense")), Profile::default());
        assert_eq!(Profile::decode(Some(r#"{"name":"Old"}"#)).name, "Old", "fields a stored copy lacks take defaults");
    }
}
