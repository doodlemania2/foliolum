//! The gate's second reference: Pubwork's patched libmspub WebAssembly reader (docs/architecture.md,
//! "The Phase 1 gate"). For a file with no Publisher layout export, completeness is judged against
//! what this independent reader finds on each page: page count and sizes, the number of text boxes,
//! pictures and shapes, and the text itself after whitespace is collapsed.
//!
//! The reader runs under Node through `gate/pubwork/oracle.mjs`; its files are fetched at a pin by
//! `gate/fetch-fixtures.sh` into `fixtures/pubwork/` (MPL-2.0, never vendored).
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use designcraft_doc::{Content, Document, Item, SpreadRef, TABLE_ANCHOR};
use serde::{Deserialize, Serialize};

/// Page size tolerance, in points.
const SIZE_TOL: f64 = 0.5;

/// What one reader found on one page.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct Page {
    pub w: f64,
    pub h: f64,
    pub text: usize,
    pub image: usize,
    pub shape: usize,
    #[serde(default)]
    pub table: usize,
    /// The page's text, one entry per story (and per table cell), whitespace collapsed, sorted.
    pub stories: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct Reading {
    pub ok: bool,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub partial: bool,
    #[serde(default)]
    pub pages: Vec<Page>,
}

/// Where the reader's files and Node are, if this machine has them.
pub struct Reader {
    node: PathBuf,
    script: PathBuf,
    dir: PathBuf,
}

impl Reader {
    /// The reader fetched into `fixtures/pubwork/`, run by `node` (or `$FOLIOLUM_NODE`). `Err` says
    /// what is missing, so the gate can show completeness as unchecked rather than guess.
    pub fn find(root: &Path) -> Result<Reader, String> {
        let dir = root.join("fixtures/pubwork");
        if !dir.join("pubreader.wasm").is_file() {
            return Err("Pubwork reader not fetched (gate/fetch-fixtures.sh)".into());
        }
        let node = std::env::var_os("FOLIOLUM_NODE").map(PathBuf::from).unwrap_or_else(|| "node".into());
        match Command::new(&node).arg("--version").output() {
            Ok(o) if o.status.success() => {}
            _ => return Err(format!("{} not found; the Pubwork reader needs Node", node.display())),
        }
        Ok(Reader { node, script: root.join("gate/pubwork/oracle.mjs"), dir })
    }

    pub fn read(&self, file: &Path) -> Result<Reading, String> {
        let o =
            Command::new(&self.node).arg(&self.script).arg(&self.dir).arg(file).output().map_err(|e| format!("running the Pubwork reader: {e}"))?;
        let line = String::from_utf8_lossy(&o.stdout);
        let r: Reading = serde_json::from_str(line.trim()).map_err(|e| format!("Pubwork reader output: {e}"))?;
        if !r.ok {
            return Err(format!("the Pubwork reader failed: {}", r.error.as_deref().unwrap_or("no reason given")));
        }
        Ok(r)
    }
}

/// Whitespace collapsed to single spaces, ends trimmed.
pub fn norm(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn leaves<'a>(it: &'a Item, out: &mut Vec<&'a Item>) {
    match &it.content {
        Content::Group { items } => items.iter().for_each(|c| leaves(c, out)),
        _ => out.push(it),
    }
}

/// The items drawn on one page: its own and those of the parent pages it shows (libmspub draws a
/// master page's items on every page that uses it).
pub fn page_items(d: &Document, abs: usize) -> Vec<&Item> {
    let mut out = Vec::new();
    let Some((si, pi)) = d.page_loc(abs) else { return out };
    let (Some(spread), Some(page)) = (d.spreads.get(si), d.page(abs)) else { return out };
    for it in &spread.items {
        if spread.pages.len() == 1 || spread.page_at_x(it.bounds().center().x) == Some(pi) {
            leaves(it, &mut out);
        }
    }
    if page.show_parent_items {
        for (parent_index, parent_page) in d.shown_parent_pages(SpreadRef::Doc(si), pi) {
            let Some(parent) = d.parents.get(parent_index) else { continue };
            for it in &parent.items {
                if !page.overridden.contains(&it.id) && (parent.pages.len() == 1 || parent.page_at_x(it.bounds().center().x) == Some(parent_page)) {
                    leaves(it, &mut out);
                }
            }
        }
    }
    out
}

/// The imported document, summarised the way `oracle.mjs` summarises the reader's: group members
/// rather than groups; a text frame that holds only a table counts as the table; the text is each
/// story's (table anchors left out) and each table cell's.
pub fn ours(d: &Document) -> Vec<Page> {
    let mut pages = Vec::new();
    for abs in 0..d.page_count() {
        let Some(page) = d.page(abs) else { continue };
        let mut s = Page { w: page.width, h: page.height, ..Page::default() };
        let mut seen = BTreeSet::new();
        for it in page_items(d, abs) {
            match &it.content {
                Content::Text(t) => {
                    let Some(story) = d.stories.get(&t.story) else {
                        s.text += 1;
                        continue;
                    };
                    let text = norm(&story.text.replace(TABLE_ANCHOR, " "));
                    if !text.is_empty() || story.tables.is_empty() {
                        s.text += 1;
                    }
                    if !seen.insert(t.story) {
                        continue;
                    }
                    if !text.is_empty() {
                        s.stories.push(text);
                    }
                    for table in story.tables.values() {
                        s.table += 1;
                        s.stories.extend(table.cells.iter().map(|c| norm(&c.text.text)).filter(|t| !t.is_empty()));
                    }
                }
                Content::Graphic(_) => s.image += 1,
                _ => s.shape += 1,
            }
        }
        s.stories.sort();
        pages.push(s);
    }
    pages
}

pub fn is_empty(p: &Page) -> bool {
    p.text + p.image + p.shape + p.table == 0
}

/// Every way the import differs from the reader, and notes that are not differences. No differences
/// means they agree.
pub fn compare(ours: &[Page], theirs: &[Page]) -> (Vec<String>, Vec<String>) {
    let mut diffs = Vec::new();
    let mut notes = Vec::new();
    // libmspub leaves out pages with nothing on them. When that is the whole difference in the page
    // count, compare the pages that have something on them.
    let kept: Vec<Page>;
    let mut ours = ours;
    if ours.len() > theirs.len() {
        kept = ours.iter().filter(|p| !is_empty(p)).cloned().collect();
        if kept.len() == theirs.len() {
            notes.push(format!("{} empty page(s) that Pubwork's reader doesn't list", ours.len() - kept.len()));
            ours = &kept;
        }
    }
    if ours.len() != theirs.len() {
        diffs.push(format!("pages {} vs Pubwork {}", ours.len(), theirs.len()));
    }
    for (i, (a, b)) in ours.iter().zip(theirs).enumerate() {
        let n = i + 1;
        if (a.w - b.w).abs() > SIZE_TOL || (a.h - b.h).abs() > SIZE_TOL {
            diffs.push(format!("page {n} size {:.1}x{:.1} pt vs Pubwork {:.1}x{:.1}", a.w, a.h, b.w, b.h));
        }
        // Fewer than the reader found is a loss. More is noted, not failed: the reader can't see
        // everything (newer WordArt, for one), and the reviewer judges what is drawn. WordArt the
        // reader can't read may still reach it as a bare shape where we have text, so a shortfall in
        // shapes is excused up to the number of extra text boxes, and noted.
        let wordart = b.shape.saturating_sub(a.shape).min(a.text.saturating_sub(b.text));
        if wordart > 0 {
            notes.push(format!("page {n} {wordart} shape(s) the reader found may be WordArt, which we import as text"));
        }
        for (what, x, y) in
            [("text boxes", a.text, b.text), ("pictures", a.image, b.image), ("shapes", a.shape + wordart, b.shape), ("tables", a.table, b.table)]
        {
            if x < y {
                diffs.push(format!("page {n} {what} {x} vs Pubwork {y}"));
            } else if x > y {
                notes.push(format!("page {n} {what} {x} vs Pubwork {y}"));
            }
        }
        let mut missing = minus(&b.stories, &a.stories);
        let extra = minus(&a.stories, &b.stories);
        let split = join_split_cells(&mut missing, &mut extra.clone());
        if split > 0 {
            notes.push(format!("page {n} {split} text(s) the reader lists apart that we hold joined to a neighbour (a cell split in two)"));
        }
        if let Some(m) = missing.first() {
            let ours = extra.first().map_or(String::new(), |e| format!(" (ours has {:?})", clip(e)));
            diffs.push(format!("page {n} text missing or different: {} of the reader's, e.g. {:?}{ours}", missing.len(), clip(m)));
        } else if !extra.is_empty() {
            notes.push(format!("page {n} text only ours: {:?}", clip(extra[0])));
        }
    }
    (diffs, notes)
}

/// A table cell split in two keeps both halves' text, one after the other, in one cell of ours
/// (gate/README.md, "Cells split in two"), where the reader lists each half, or only some of them.
/// A reader text that is a word-bounded leading or trailing part of one of our unmatched texts is
/// held there, not lost: remove it, and use up that part of ours so it can't count twice.
/// Returns how many reader texts were found that way.
fn join_split_cells(missing: &mut Vec<&String>, extra: &mut [&String]) -> usize {
    let mut rest: Vec<String> = extra.iter().map(|s| s.to_string()).collect();
    let mut found = 0;
    missing.retain(|m| {
        for r in rest.iter_mut() {
            if r.is_empty() {
                continue;
            } else if r == *m {
                r.clear();
            } else if let Some(tail) = r.strip_prefix(m.as_str()).and_then(|t| t.strip_prefix(' ')) {
                *r = tail.to_string();
            } else if let Some(head) = r.strip_suffix(m.as_str()).and_then(|t| t.strip_suffix(' ')) {
                *r = head.to_string();
            } else {
                continue;
            }
            found += 1;
            return false;
        }
        true
    });
    found
}

/// Entries of `a` that `b` doesn't have, counting repeats.
fn minus<'a>(a: &'a [String], b: &[String]) -> Vec<&'a String> {
    let mut left: Vec<&String> = b.iter().collect();
    a.iter()
        .filter(|s| match left.iter().position(|t| t == s) {
            Some(i) => {
                left.swap_remove(i);
                false
            }
            None => true,
        })
        .collect()
}

fn clip(s: &str) -> String {
    if s.chars().count() > 60 { format!("{}…", s.chars().take(60).collect::<String>()) } else { s.to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(text: usize, image: usize, shape: usize, stories: &[&str]) -> Page {
        Page { w: 612.0, h: 792.0, text, image, shape, table: 0, stories: stories.iter().map(|s| s.to_string()).collect() }
    }

    #[test]
    fn the_same_pages_agree() {
        let p = vec![page(2, 1, 3, &["A headline", "Some body text"])];
        assert_eq!(compare(&p, &p), (vec![], vec![]));
    }

    #[test]
    fn a_split_cell_held_as_one_text_is_noted_not_failed() {
        let ours = vec![page(1, 0, 0, &["Mon", "Daily Challenge Free time"])];
        let theirs = vec![page(1, 0, 0, &["Daily Challenge", "Free time", "Mon"])];
        let (diffs, notes) = compare(&ours, &theirs);
        assert!(diffs.is_empty(), "{diffs:?}");
        assert!(notes.iter().any(|n| n.contains("2 text(s) the reader lists apart")), "{notes:?}");
        // The reader may list only one half; it is still held, and only once.
        let theirs = vec![page(1, 0, 0, &["Free time", "Free time", "Mon"])];
        let (diffs, _) = compare(&ours, &theirs);
        assert!(diffs.iter().any(|d| d.contains("1 of the reader's")), "{diffs:?}");
        // Only a word-bounded part counts.
        let ours = vec![page(1, 0, 0, &["Mon", "Daily Challenge Free timer"])];
        let theirs = vec![page(1, 0, 0, &["Free time", "Mon"])];
        assert!(!compare(&ours, &theirs).0.is_empty());
    }

    #[test]
    fn missing_text_is_a_loss() {
        let ours = vec![page(1, 0, 0, &["A headline"])];
        let theirs = vec![page(2, 0, 0, &["A headline", "Some body text"])];
        let (diffs, _) = compare(&ours, &theirs);
        assert_eq!(diffs.len(), 2, "{diffs:?}");
        assert!(diffs[0].contains("text boxes 1 vs Pubwork 2"));
        assert!(diffs[1].contains("\"Some body text\""));
    }

    #[test]
    fn changed_text_is_a_loss_even_with_the_same_counts() {
        let (diffs, _) = compare(&[page(1, 0, 0, &["Mass at 9"])], &[page(1, 0, 0, &["Mass at 9:30"])]);
        assert_eq!(diffs, vec!["page 1 text missing or different: 1 of the reader's, e.g. \"Mass at 9:30\" (ours has \"Mass at 9\")"]);
    }

    #[test]
    fn fewer_pictures_or_shapes_or_another_page_size_is_a_loss() {
        let mut theirs = page(0, 2, 4, &[]);
        theirs.w = 595.3;
        let (diffs, _) = compare(&[page(0, 1, 3, &[])], &[theirs]);
        assert_eq!(diffs.len(), 3, "{diffs:?}");
    }

    #[test]
    fn more_than_the_reader_found_is_noted_not_failed() {
        let (diffs, notes) = compare(&[page(2, 1, 1, &["WordArt words", "Body"])], &[page(1, 1, 1, &["Body"])]);
        assert!(diffs.is_empty(), "{diffs:?}");
        assert_eq!(notes.len(), 2, "{notes:?}");
    }

    #[test]
    fn a_shape_where_we_have_wordart_text_is_excused_once() {
        // The reader found WordArt only as a shape; we have it as text.
        let (diffs, notes) = compare(&[page(1, 0, 0, &["Sample Text!"])], &[page(0, 0, 1, &[])]);
        assert!(diffs.is_empty(), "{diffs:?}");
        assert!(notes.iter().any(|n| n.contains("may be WordArt")), "{notes:?}");
        // Two missing shapes and one extra text box: one shape is still missing.
        let (diffs, _) = compare(&[page(1, 0, 0, &["Sample Text!"])], &[page(0, 0, 2, &[])]);
        assert_eq!(diffs, vec!["page 1 shapes 1 vs Pubwork 2"]);
    }

    #[test]
    fn empty_pages_the_reader_leaves_out_are_noted() {
        let ours = vec![page(1, 0, 0, &["Front"]), page(0, 0, 0, &[]), page(1, 0, 0, &["Back"])];
        let theirs = vec![page(1, 0, 0, &["Front"]), page(1, 0, 0, &["Back"])];
        let (diffs, notes) = compare(&ours, &theirs);
        assert!(diffs.is_empty(), "{diffs:?}");
        assert_eq!(notes, vec!["1 empty page(s) that Pubwork's reader doesn't list"]);
        // A page with something on it is never left out.
        let (diffs, _) = compare(&[page(1, 0, 0, &["Front"]), page(1, 0, 0, &["Back"])], &[page(1, 0, 0, &["Front"])]);
        assert_eq!(diffs, vec!["pages 2 vs Pubwork 1"]);
    }

    #[test]
    fn repeated_stories_are_counted() {
        let (diffs, _) = compare(&[page(1, 0, 0, &["Caption"])], &[page(2, 0, 0, &["Caption", "Caption"])]);
        assert!(diffs.iter().any(|d| d.contains("text missing")), "{diffs:?}");
    }

    #[test]
    fn whitespace_is_collapsed() {
        assert_eq!(norm("  Mass\n\tat   9:30 \u{2028}"), "Mass at 9:30");
    }
}
