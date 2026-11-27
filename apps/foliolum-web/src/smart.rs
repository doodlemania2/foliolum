//! Smart frames (#52): a text box that fills itself from a small table of data: Mass times,
//! intentions, a schedule. The data comes from a spreadsheet (CSV or Excel) or is typed straight
//! into the dialog's grid, and the box shows it as a table or as one line per row from a pattern
//! such as `{Day}  {Time}  {Intention}`.
//!
//! The binding (layout, pattern and the data itself) travels in the frame's label
//! (`object.label`, DesignCraft's script label), so a saved document reopens with its smart frames
//! still editable. Filling runs through the editor's own commands (`story.setText`,
//! `table.convertFromText`, `object.label`), so it is undone like any other edit.
use designcraft_engine::Session;
use designcraft_engine::doc::ItemId;
use serde::{Deserialize, Serialize};
use serde_json::json;

/// What a smart frame's label starts with.
pub const LABEL_PREFIX: &str = "foliolum-smart:";

/// Largest table a smart frame holds: enough for any bulletin's schedule, small enough for a label.
pub const MAX_ROWS: usize = 500;
pub const MAX_COLS: usize = 20;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layout {
    /// A table with the data's columns.
    #[default]
    Table,
    /// One paragraph per row, from [`Binding::pattern`].
    Lines,
}

/// A smart frame's data and how it shows it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Binding {
    pub layout: Layout,
    /// For [`Layout::Lines`]: `{Name}` is that column (by its header), `{1}` the first column.
    #[serde(default)]
    pub pattern: String,
    /// The first row names the columns. In a table it is the table's first row; with a pattern it
    /// only names the placeholders.
    #[serde(default)]
    pub header: bool,
    /// Where the data last came from (a file name), for the dialog.
    #[serde(default)]
    pub source: String,
    pub rows: Vec<Vec<String>>,
}

impl Binding {
    /// The column names: the header row when there is one, else `1`, `2`, …
    pub fn columns(&self) -> Vec<String> {
        let width = self.rows.iter().map(Vec::len).max().unwrap_or(0);
        (0..width)
            .map(|i| match self.rows.first().filter(|_| self.header).and_then(|r| r.get(i)).map(|s| s.trim()).filter(|s| !s.is_empty()) {
                Some(name) => name.to_string(),
                None => (i + 1).to_string(),
            })
            .collect()
    }

    /// The rows that are data, not the header.
    pub fn data_rows(&self) -> &[Vec<String>] {
        if self.header && !self.rows.is_empty() { &self.rows[1..] } else { &self.rows }
    }

    /// A pattern that shows every column, separated by tabs.
    pub fn default_pattern(&self) -> String {
        self.columns().iter().map(|c| format!("{{{c}}}")).collect::<Vec<_>>().join("\t")
    }

    /// The text the frame holds: for a table, tab-separated rows (the header too, when there is
    /// one); for lines, the pattern filled for each data row. Tabs and line breaks inside a value
    /// become spaces, so they can't break the layout.
    pub fn text(&self) -> String {
        let clean = |s: &str| s.replace(['\t', '\r', '\n'], " ").trim().to_string();
        match self.layout {
            Layout::Table => {
                let width = self.rows.iter().map(Vec::len).max().unwrap_or(0);
                self.rows
                    .iter()
                    .map(|r| (0..width).map(|i| r.get(i).map(|v| clean(v)).unwrap_or_default()).collect::<Vec<_>>().join("\t"))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            Layout::Lines => {
                let pattern = if self.pattern.trim().is_empty() { self.default_pattern() } else { self.pattern.clone() };
                let names = self.columns();
                self.data_rows().iter().map(|r| fill_pattern(&pattern, &names, r, &clean)).collect::<Vec<_>>().join("\n")
            }
        }
    }

    pub fn to_label(&self) -> String {
        format!("{LABEL_PREFIX}{}", serde_json::to_string(self).unwrap_or_default())
    }

    pub fn from_label(label: &str) -> Option<Binding> {
        serde_json::from_str(label.strip_prefix(LABEL_PREFIX)?).ok()
    }
}

/// `pattern` with each `{name}` replaced by that column of `row` (`{1}` is the first column; names
/// match without regard to case). An unknown name is left as written, so a typo shows.
fn fill_pattern(pattern: &str, names: &[String], row: &[String], clean: &dyn Fn(&str) -> String) -> String {
    let mut out = String::new();
    let mut rest = pattern;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let Some(close) = after.find('}') else {
            out.push_str(&rest[open..]);
            return out;
        };
        let key = after[..close].trim();
        let index = names.iter().position(|n| n.eq_ignore_ascii_case(key)).or_else(|| key.parse::<usize>().ok().filter(|n| *n > 0).map(|n| n - 1));
        match index {
            Some(i) if i < names.len() => out.push_str(&row.get(i).map(|v| clean(v)).unwrap_or_default()),
            _ => out.push_str(&rest[open..open + close + 2]),
        }
        rest = &after[close + 1..];
    }
    out.push_str(rest);
    out
}

/// Reads a spreadsheet: `.xlsx` (its first sheet) or delimited text (comma, semicolon or tab,
/// whichever the first line uses most). Trailing empty rows and columns are dropped, and the size
/// is limited to [`MAX_ROWS`] × [`MAX_COLS`].
pub fn read_table(name: &str, bytes: &[u8]) -> Result<Vec<Vec<String>>, String> {
    let lower = name.to_ascii_lowercase();
    let mut rows = if lower.ends_with(".xlsx") {
        designcraft_textimport::xlsx_records(bytes, None).map_err(|e| format!("Couldn't read {name}: {e}"))?
    } else if lower.ends_with(".xls") {
        return Err("Old Excel files (.xls) can't be read. Save it as .xlsx or CSV and try again.".into());
    } else {
        let text = String::from_utf8_lossy(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes)).into_owned();
        parse_delimited(&text)
    };
    while rows.last().is_some_and(|r| r.iter().all(|c| c.trim().is_empty())) {
        rows.pop();
    }
    let width = rows.iter().map(|r| r.iter().rposition(|c| !c.trim().is_empty()).map_or(0, |i| i + 1)).max().unwrap_or(0);
    if rows.is_empty() || width == 0 {
        return Err(format!("{name} has no data."));
    }
    if rows.len() > MAX_ROWS || width > MAX_COLS {
        return Err(format!("{name} is too big for a smart frame: at most {MAX_ROWS} rows and {MAX_COLS} columns."));
    }
    for r in &mut rows {
        r.resize(width, String::new());
    }
    Ok(rows)
}

/// CSV as spreadsheets write it: quoted fields may hold the separator, doubled quotes and line
/// breaks.
pub fn parse_delimited(text: &str) -> Vec<Vec<String>> {
    let first = text.lines().next().unwrap_or("");
    let sep = [',', ';', '\t'].into_iter().max_by_key(|c| first.matches(*c).count()).filter(|c| first.contains(*c)).unwrap_or(',');
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            match c {
                '"' if chars.peek() == Some(&'"') => {
                    field.push('"');
                    chars.next();
                }
                '"' => quoted = false,
                _ => field.push(c),
            }
        } else if c == '"' && field.is_empty() {
            quoted = true;
        } else if c == sep {
            row.push(std::mem::take(&mut field));
        } else if c == '\n' || c == '\r' {
            if c == '\r' && chars.peek() == Some(&'\n') {
                chars.next();
            }
            row.push(std::mem::take(&mut field));
            rows.push(std::mem::take(&mut row));
        } else {
            field.push(c);
        }
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    rows
}

/// The text frame `id` and its story, if `id` is one.
fn frame_story(s: &Session, id: ItemId) -> Option<u64> {
    let d = s.doc().ok()?;
    d.doc.item(id)?.text_frame().map(|tf| tf.story.0)
}

/// The smart-frame binding of frame `id`, if it has one.
pub fn binding_of(s: &Session, id: ItemId) -> Option<Binding> {
    Binding::from_label(&s.doc().ok()?.doc.item(id)?.label)
}

/// The one text frame selected, if exactly one is.
pub fn selected_frame(s: &Session) -> Option<ItemId> {
    let d = s.doc().ok()?;
    match d.selection.items.as_slice() {
        [id] => d.doc.item(*id)?.text_frame().map(|_| *id),
        _ => None,
    }
}

/// Fills frame `id` from `b` and keeps `b` in its label.
pub fn fill(s: &mut Session, id: ItemId, b: &Binding) -> Result<(), String> {
    crate::undo::as_one_step(s, "Smart Frame", |s| fill_steps(s, id, b))
}

fn fill_steps(s: &mut Session, id: ItemId, b: &Binding) -> Result<(), String> {
    let story = frame_story(s, id).ok_or("Select a text box to make it a smart frame.")?;
    let text = b.text();
    let run = |s: &mut Session, cmd: &str, p: serde_json::Value| s.execute(cmd, &p).map(|_| ()).map_err(|e| format!("{cmd}: {e}"));
    run(s, "story.setText", json!({"story": story, "text": text}))?;
    if b.layout == Layout::Table && !text.is_empty() {
        run(s, "text.select", json!({"story": story, "anchor": 0, "focus": text.len()}))?;
        run(s, "table.convertFromText", json!({"columnSeparator": "tab", "rowSeparator": "paragraph"}))?;
    }
    run(s, "selection.set", json!({"ids": [id.0]}))?;
    run(s, "object.label", json!({"label": b.to_label(), "ids": [id.0]}))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn masses() -> Binding {
        Binding {
            layout: Layout::Table,
            pattern: String::new(),
            header: true,
            source: "masses.csv".into(),
            rows: parse_delimited("Day,Time,Intention\nSaturday,5:00 pm,\"For Ann Example, RIP\"\nSunday,10:30 am,People of the parish\n"),
        }
    }

    #[test]
    fn csv_reads_as_spreadsheets_write_it() {
        let rows = parse_delimited("a;b;c\r\n\"x;y\";\"say \"\"hi\"\"\";\"two\nlines\"\r\n");
        assert_eq!(rows, vec![vec!["a", "b", "c"], vec!["x;y", "say \"hi\"", "two\nlines"]]);
        assert_eq!(parse_delimited("one\ttwo\n1\t2"), vec![vec!["one", "two"], vec!["1", "2"]]);
        let t = read_table("m.csv", b"\xEF\xBB\xBFA,B,,\n1,2,,\n,,,\n\n").unwrap();
        assert_eq!(t, vec![vec!["A", "B"], vec!["1", "2"]], "BOM, trailing empty rows and columns dropped");
        assert!(read_table("m.xls", b"").unwrap_err().contains(".xlsx"));
        assert!(read_table("m.csv", b"\n\n").unwrap_err().contains("no data"));
    }

    #[test]
    fn a_pattern_fills_one_line_per_row() {
        let mut b = masses();
        b.layout = Layout::Lines;
        b.pattern = "{day} {Time}: {3} {Nope}".into();
        assert_eq!(b.text(), "Saturday 5:00 pm: For Ann Example, RIP {Nope}\nSunday 10:30 am: People of the parish {Nope}");
        b.pattern.clear();
        assert_eq!(b.text().lines().next(), Some("Saturday\t5:00 pm\tFor Ann Example, RIP"));
        assert_eq!(b.columns(), vec!["Day", "Time", "Intention"]);
    }

    #[test]
    fn a_table_keeps_its_header_and_cleans_values() {
        let mut b = masses();
        b.rows[1][2] = "two\tparts\nhere".into();
        assert_eq!(b.text(), "Day\tTime\tIntention\nSaturday\t5:00 pm\ttwo parts here\nSunday\t10:30 am\tPeople of the parish");
    }

    #[test]
    fn the_binding_round_trips_through_the_label() {
        let b = masses();
        let label = b.to_label();
        assert!(label.starts_with(LABEL_PREFIX));
        assert_eq!(Binding::from_label(&label), Some(b));
        assert_eq!(Binding::from_label("price"), None);
    }

    fn session_with_frame() -> (Session, ItemId) {
        let mut s = Session::new();
        s.execute("file.new", &json!({})).unwrap();
        let r = s.execute("frame.create", &json!({"rect": [72, 72, 372, 272], "content": "text", "text": "old"})).unwrap();
        let id = ItemId(r["id"].as_u64().expect("frame id"));
        (s, id)
    }

    #[test]
    fn filling_makes_a_table_and_keeps_the_binding_and_undo_works() {
        let (mut s, id) = session_with_frame();
        let b = masses();
        fill(&mut s, id, &b).unwrap();
        let d = s.doc().unwrap();
        let sid = d.doc.item(id).unwrap().text_frame().unwrap().story;
        let story = &d.doc.stories[&sid];
        let table = story.tables.values().next().expect("a table");
        assert_eq!(table.cells.len(), 9, "3 × 3 cells");
        assert!(table.cells.iter().any(|c| c.text.text == "For Ann Example, RIP"));
        assert_eq!(binding_of(&s, id), Some(b));
        s.execute("edit.undo", &json!({})).unwrap();
        let d = s.doc().unwrap();
        assert_eq!(d.doc.stories[&sid].text, "old", "one undo takes the whole fill back");
        assert!(d.doc.item(id).unwrap().label.is_empty());
    }

    #[test]
    fn filling_with_a_pattern_writes_lines() {
        let (mut s, id) = session_with_frame();
        let mut b = masses();
        b.layout = Layout::Lines;
        b.pattern = "{Day}, {Time}".into();
        fill(&mut s, id, &b).unwrap();
        let d = s.doc().unwrap();
        let sid = d.doc.item(id).unwrap().text_frame().unwrap().story;
        assert_eq!(d.doc.stories[&sid].text, "Saturday, 5:00 pm\nSunday, 10:30 am");
        assert_eq!(selected_frame(&s), Some(id));
    }
}

/// Where a spreadsheet the page read waits for the dialog: (file name, bytes).
pub type Loaded = std::sync::Arc<std::sync::Mutex<Option<(String, Vec<u8>)>>>;

/// The Smart frame dialog: the data as an editable grid (typed, or loaded from a spreadsheet),
/// how the frame shows it, and Fill.
#[derive(Default)]
pub struct Dialog {
    pub open: bool,
    frame: Option<ItemId>,
    binding: Binding,
    message: String,
    /// A spreadsheet the page read for the dialog: (file name, bytes).
    loaded: Loaded,
}

/// What the dialog asks of the page this frame.
pub enum Want {
    /// Fill the frame from this binding.
    Fill(ItemId, Binding),
    /// Ask the person for a spreadsheet and put it in [`Dialog::loaded_slot`].
    PickFile,
}

impl Dialog {
    /// Opens on the selected text box, with its data if it is already a smart frame.
    pub fn open_for(&mut self, s: &Session) {
        self.open = true;
        self.message.clear();
        self.frame = selected_frame(s);
        self.binding = match self.frame.and_then(|id| binding_of(s, id)) {
            Some(b) => b,
            None => Binding {
                header: true,
                rows: vec![vec!["Day".into(), "Time".into(), "Intention".into()], vec![String::new(); 3]],
                ..Default::default()
            },
        };
    }

    pub fn loaded_slot(&self) -> Loaded {
        self.loaded.clone()
    }

    pub fn show(&mut self, ctx: &egui::Context) -> Option<Want> {
        if !self.open {
            return None;
        }
        if let Some((name, bytes)) = self.loaded.lock().unwrap_or_else(|e| e.into_inner()).take() {
            match read_table(&name, &bytes) {
                Ok(rows) => {
                    self.binding.rows = rows;
                    self.binding.source = name.clone();
                    self.message = format!("Loaded {name}. Check the first row and fill.");
                }
                Err(e) => self.message = e,
            }
        }
        let mut open = self.open;
        let mut want = None;
        egui::Window::new("Smart frame").open(&mut open).collapsible(false).default_width(560.0).show(ctx, |ui| want = self.body(ui));
        self.open = open && !matches!(want, Some(Want::Fill(..)));
        want
    }

    fn body(&mut self, ui: &mut egui::Ui) -> Option<Want> {
        let mut want = None;
        let Some(frame) = self.frame else {
            ui.label("Select one text box first, then choose Smart frame again. Its text will be replaced by the data.");
            return None;
        };
        ui.label("The box fills itself from this data. Type it here, or load a spreadsheet; open this again any time to change it.");
        ui.horizontal(|ui| {
            if ui.button("Load a spreadsheet…").on_hover_text("CSV or Excel (.xlsx); the first sheet is read").clicked() {
                want = Some(Want::PickFile);
            }
            if !self.binding.source.is_empty() {
                ui.label(egui::RichText::new(format!("From {}", self.binding.source)).small());
            }
        });
        ui.checkbox(&mut self.binding.header, "The first row names the columns");
        ui.separator();
        let b = &mut self.binding;
        let width = b.rows.iter().map(Vec::len).max().unwrap_or(0).max(1);
        let mut remove_row = None;
        egui::ScrollArea::both().max_height(260.0).id_salt("smart_grid").show(ui, |ui| {
            egui::Grid::new("smart_cells").striped(true).spacing([4.0, 2.0]).show(ui, |ui| {
                for (r, row) in b.rows.iter_mut().enumerate() {
                    row.resize(width, String::new());
                    for cell in row.iter_mut() {
                        let edit = egui::TextEdit::singleline(cell).desired_width(120.0);
                        let edit = if r == 0 && b.header { edit.font(egui::TextStyle::Button) } else { edit };
                        ui.add(edit);
                    }
                    if ui.small_button("✕").on_hover_text("Remove this row").clicked() {
                        remove_row = Some(r);
                    }
                    ui.end_row();
                }
            });
        });
        if let Some(r) = remove_row {
            b.rows.remove(r);
        }
        ui.horizontal(|ui| {
            if ui.button("Add a row").clicked() && b.rows.len() < MAX_ROWS {
                b.rows.push(vec![String::new(); width]);
            }
            if ui.button("Add a column").clicked() && width < MAX_COLS {
                b.rows.iter_mut().for_each(|r| r.push(String::new()));
            }
            if width > 1 && ui.button("Remove the last column").clicked() {
                b.rows.iter_mut().for_each(|r| {
                    r.pop();
                });
            }
        });
        ui.separator();
        ui.horizontal(|ui| {
            ui.label("Show it as");
            ui.radio_value(&mut b.layout, Layout::Table, "a table");
            ui.radio_value(&mut b.layout, Layout::Lines, "one line per row");
        });
        if b.layout == Layout::Lines {
            ui.horizontal(|ui| {
                ui.label("Each line");
                let hint = b.default_pattern();
                ui.add(egui::TextEdit::singleline(&mut b.pattern).hint_text(hint).desired_width(360.0));
            });
            let names: Vec<String> = b.columns().iter().map(|c| format!("{{{c}}}")).collect();
            ui.label(egui::RichText::new(format!("Use {} for the columns.", names.join(" "))).small());
            if let Some(first) = b.text().lines().next() {
                ui.label(egui::RichText::new(format!("First line: {}", first.replace('\t', "  "))).small().italics());
            }
        }
        if !self.message.is_empty() {
            ui.label(egui::RichText::new(&self.message).small());
        }
        ui.add_space(6.0);
        let empty = b.data_rows().iter().all(|r| r.iter().all(|c| c.trim().is_empty()));
        if ui.add_enabled(!empty, egui::Button::new("Fill the box")).clicked() {
            want = Some(Want::Fill(frame, b.clone()));
        }
        want
    }
}
