//! The Phase 1 gate (docs/architecture.md, "The Phase 1 gate"). Per .pub file:
//!
//! - **opens:** foliolum-pub imports it without error (or panic).
//! - **complete:** pages, page sizes and every element's presence and text match a reference: Publisher's
//!   own layout export (`truth/<name>.json`) when the file has one, otherwise what Pubwork's independent
//!   reader finds on each page (pubwork.rs).
//! - **exports:** DesignCraft writes a PDF whose page count and sizes match the import.
//! - **recognizable:** a person looked at the rendered pages (`gate/verdicts.json`, keyed by SHA-256).
//!
//! A file passes only when all four pass. Anything not yet checkable is "unchecked", never a pass.
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};

use designcraft_engine::{DocState, Session};
use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{pubwork, truth};

/// Longest side of a review render, in pixels.
const REVIEW_PX: u32 = 1400;
/// Page size tolerance, in points.
const SIZE_TOL: f64 = 0.5;

#[derive(Serialize, Clone, Debug, PartialEq)]
#[serde(tag = "status", content = "reason", rename_all = "lowercase")]
pub enum Check {
    Pass,
    Fail(String),
    Unchecked(String),
}

impl Check {
    fn is_pass(&self) -> bool {
        *self == Check::Pass
    }
    pub fn cell(&self) -> String {
        match self {
            Check::Pass => "pass".into(),
            Check::Fail(r) => format!("**fail**: {}", short(r)),
            Check::Unchecked(r) => format!("–: {}", short(r)),
        }
    }
}

fn short(s: &str) -> String {
    let s = s.replace('|', "/").replace('\n', " ");
    if s.chars().count() > 140 { format!("{}…", s.chars().take(140).collect::<String>()) } else { s }
}

#[derive(Serialize, Debug)]
pub struct FileResult {
    pub label: String,
    pub sha256: String,
    pub bytes: usize,
    pub pages: Option<usize>,
    pub warnings: usize,
    /// What the importer said it could not bring over (results.jsonl only, for diagnosis).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warning_text: Vec<String>,
    pub opens: Check,
    pub complete: Check,
    pub exports: Check,
    pub recognizable: Check,
    pub pass: bool,
    pub truth: Option<truth::Score>,
    /// Which reference judged completeness: "layout export" or "Pubwork".
    pub reference: Option<&'static str>,
    /// Every way the import differs from Pubwork's reader, whichever reference judged completeness
    /// (with a layout export these are for information: they show how far the two readers agree).
    pub pubwork_diffs: Vec<String>,
    /// When the readers differ, both page summaries, to see why (results.jsonl only; it never leaves
    /// target/, because the summaries hold the file's text).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pubwork_pages: Option<PagePair>,
    /// Font families the text asks for (characters set in each) and the family that draws them here.
    pub fonts: Vec<FontUse>,
    /// Characters that don't fit their frames as drawn here, and so are missing from the PDF.
    /// Reported, not failed on: a Publisher file can hold overset text itself, but a substitute font
    /// that is wider than the original adds to it.
    pub overset_chars: Option<usize>,
    /// Every difference the reference comparison found, for the per-file JSON (not the table).
    pub notes: Vec<String>,
}

#[derive(Serialize, Debug)]
pub struct PagePair {
    pub ours: Vec<pubwork::Page>,
    pub pubwork: Vec<pubwork::Page>,
}

#[derive(Serialize, Debug, Clone)]
pub struct FontUse {
    pub family: String,
    pub chars: usize,
    pub drawn_with: String,
}

/// The document's font families (characters set in each), each registered with a substitute if
/// needed, and the family that draws it here.
fn font_uses(doc: &designcraft_doc::Document) -> Vec<FontUse> {
    let families = foliolum_fonts::document_families(doc);
    let db = designcraft_fonts::FontDb::global();
    foliolum_fonts::register_families(db, families.keys().map(String::as_str));
    families.into_iter().map(|(family, chars)| FontUse { drawn_with: db.face(&family, "Regular").family.clone(), family, chars }).collect()
}

pub struct Options {
    /// Where PDFs, review PNGs and results.json go.
    pub out: PathBuf,
    /// Label files by name (public sets) instead of by hash (contributed files).
    pub names: bool,
    /// Reviewer verdicts by SHA-256.
    pub verdicts: serde_json::Value,
    /// Pubwork's reader, or why it can't run here.
    pub pubwork: Result<pubwork::Reader, String>,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn panic_text(p: Box<dyn std::any::Any + Send>) -> String {
    p.downcast_ref::<&str>().map(|s| s.to_string()).or_else(|| p.downcast_ref::<String>().cloned()).unwrap_or_else(|| "panic".into())
}

impl FileResult {
    /// Opens, complete and exports all pass: everything but the reviewer's verdict. This is what CI
    /// holds every public dev file to, since a person, not a machine, says whether a page is
    /// recognizable. An unchecked completeness (no reference) is not a pass here either.
    pub fn machine_checks_pass(&self) -> bool {
        self.opens.is_pass() && self.complete.is_pass() && self.exports.is_pass()
    }
}

pub fn run_file(path: &Path, opts: &Options) -> FileResult {
    let bytes = std::fs::read(path).unwrap_or_default();
    let sha256 = sha256_hex(&bytes);
    let label = if opts.names {
        path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_else(|| sha256[..12].to_string())
    } else {
        sha256[..12].to_string()
    };
    let mut r = FileResult {
        label: label.clone(),
        sha256: sha256.clone(),
        bytes: bytes.len(),
        pages: None,
        warnings: 0,
        warning_text: Vec::new(),
        opens: Check::Unchecked("not run".into()),
        complete: Check::Unchecked("did not open".into()),
        exports: Check::Unchecked("did not open".into()),
        recognizable: Check::Unchecked("did not open".into()),
        pass: false,
        truth: None,
        reference: None,
        pubwork_diffs: Vec::new(),
        pubwork_pages: None,
        fonts: Vec::new(),
        overset_chars: None,
        notes: Vec::new(),
    };
    if bytes.is_empty() {
        r.opens = Check::Fail("empty or unreadable file".into());
        return r;
    }

    // 1. Opens.
    let imported = match catch_unwind(AssertUnwindSafe(|| foliolum_pub::import_pub(&bytes))) {
        Ok(Ok(i)) => i,
        Ok(Err(e)) => {
            r.opens = Check::Fail(e.to_string());
            return r;
        }
        Err(p) => {
            r.opens = Check::Fail(format!("importer panicked: {}", panic_text(p)));
            return r;
        }
    };
    r.opens = Check::Pass;
    let doc = imported.document;
    r.pages = Some(doc.page_count());
    r.warnings = imported.warnings.len();
    r.warning_text = imported.warnings.clone();
    r.fonts = font_uses(&doc);
    let sizes: Vec<(f64, f64)> = (0..doc.page_count()).filter_map(|i| doc.page(i).map(|p| (p.width, p.height))).collect();

    // 2. Complete, against Publisher's layout export when there is one, otherwise Pubwork's reader.
    let pubwork = opts.pubwork.as_ref().map_err(String::clone).and_then(|reader| reader.read(path)).and_then(|reading| {
        // A reference that sees nothing can't vouch for anything (libmspub reads some files as empty).
        if reading.pages.is_empty() && pubwork::ours(&doc).iter().any(|p| !pubwork::is_empty(p)) {
            return Err("Pubwork's reader found no pages in this file".into());
        }
        Ok(reading)
    });
    let pubwork = pubwork.map(|reading| {
        let ours = pubwork::ours(&doc);
        let (diffs, notes) = pubwork::compare(&ours, &reading.pages);
        r.notes.extend(notes.into_iter().map(|n| format!("Pubwork: {n}")));
        if !diffs.is_empty() {
            r.pubwork_pages = Some(PagePair { ours, pubwork: reading.pages });
        }
        (diffs, reading.partial)
    });
    if let Ok((diffs, _)) = &pubwork {
        r.pubwork_diffs = diffs.clone();
    }
    let truth_path =
        path.parent().unwrap_or(Path::new(".")).join("truth").join(format!("{}.json", path.file_stem().unwrap_or_default().to_string_lossy()));
    r.complete = match std::fs::read_to_string(&truth_path).ok().and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok()) {
        None => match pubwork {
            Err(why) => Check::Unchecked(format!("no layout export; {why}")),
            Ok((diffs, partial)) => {
                r.reference = Some("Pubwork");
                if partial {
                    r.notes.push("Pubwork read this file only in part".into());
                }
                if diffs.is_empty() { Check::Pass } else { Check::Fail(format!("vs Pubwork: {}", diffs.join("; "))) }
            }
        },
        Some(t) => {
            r.reference = Some("layout export");
            let mut s = truth::Score::default();
            let mut notes = Vec::new();
            truth::score(&doc, &t, &mut s, &mut notes);
            let mut fails = Vec::new();
            fails.extend(notes.iter().filter(|n| n.starts_with("pages ") || n.starts_with("page ")).cloned());
            if s.found < s.elements {
                fails.push(format!("{} of {} elements missing", s.elements - s.found, s.elements));
            }
            if s.text_ok < s.text_total {
                fails.push(format!("{} of {} text frames differ", s.text_total - s.text_ok, s.text_total));
            }
            r.truth = Some(s);
            r.notes.extend(notes);
            if fails.is_empty() { Check::Pass } else { Check::Fail(fails.join("; ")) }
        }
    };

    // 3. Exports, then 4. renders for review.
    let pdf_path = opts.out.join(format!("{label}.pdf"));
    let exported = catch_unwind(AssertUnwindSafe(|| {
        let mut session = Session::new();
        session.add_document(DocState::new(doc, None));
        let overset: usize = designcraft_engine::cmd::preflight::overset_warnings(&session, None).iter().filter_map(|w| overset_count(w)).sum();
        session.execute("file.exportPdf", &json!({ "path": pdf_path.to_string_lossy() })).map(|_| overset).map_err(|e| e.to_string())
    }));
    r.exports = match exported {
        Err(p) => Check::Fail(format!("export panicked: {}", panic_text(p))),
        Ok(Err(e)) => Check::Fail(e),
        Ok(Ok(overset)) => {
            r.overset_chars = Some(overset);
            check_pdf(&pdf_path, &sizes, &opts.out.join(&label))
        }
    };
    r.recognizable = if !r.exports.is_pass() {
        Check::Unchecked("nothing to review".into())
    } else {
        match &opts.verdicts[&sha256] {
            v if v["verdict"] == "recognizable" => Check::Pass,
            v if v["verdict"].is_string() => Check::Fail(v["note"].as_str().unwrap_or("reviewer said no").to_string()),
            _ => Check::Unchecked("awaiting review".into()),
        }
    };
    r.pass = [&r.opens, &r.complete, &r.exports, &r.recognizable].iter().all(|c| c.is_pass());
    r
}

/// The character count in one of DesignCraft's overset warnings: the number before "character(s)".
fn overset_count(warning: &str) -> Option<usize> {
    let words: Vec<&str> = warning.split_whitespace().collect();
    let at = words.iter().position(|w| w.starts_with("character(s)"))?;
    words.get(at.checked_sub(1)?)?.parse().ok()
}

/// Reads the exported PDF back, compares its pages with the document, and writes one PNG per page.
/// A page render on white paper. The renderer leaves the paper transparent, and some viewers show
/// that as black, which would put a reviewer off a page that prints on white.
fn on_paper(png: &[u8]) -> Option<Vec<u8>> {
    let page = tiny_skia::Pixmap::decode_png(png).ok()?;
    let mut out = tiny_skia::Pixmap::new(page.width(), page.height())?;
    out.fill(tiny_skia::Color::WHITE);
    out.draw_pixmap(0, 0, page.as_ref(), &tiny_skia::PixmapPaint::default(), tiny_skia::Transform::identity(), None);
    out.encode_png().ok()
}

fn check_pdf(pdf_path: &Path, sizes: &[(f64, f64)], png_dir: &Path) -> Check {
    let Ok(pdf) = std::fs::read(pdf_path) else { return Check::Fail("no PDF written".into()) };
    let Some(pdf_sizes) = designcraft_render::pdf_page_sizes(&pdf) else { return Check::Fail("PDF does not parse".into()) };
    if pdf_sizes.len() != sizes.len() {
        return Check::Fail(format!("PDF has {} pages, document {}", pdf_sizes.len(), sizes.len()));
    }
    for (i, (a, b)) in pdf_sizes.iter().zip(sizes).enumerate() {
        if (a.0 - b.0).abs() > SIZE_TOL || (a.1 - b.1).abs() > SIZE_TOL {
            return Check::Fail(format!("page {} is {:.1}x{:.1} pt in the PDF, {:.1}x{:.1} in the document", i + 1, a.0, a.1, b.0, b.1));
        }
    }
    let _ = std::fs::create_dir_all(png_dir);
    for i in 0..pdf_sizes.len() {
        let png = catch_unwind(AssertUnwindSafe(|| designcraft_render::pdf_page_png(&pdf, i, REVIEW_PX, &[], None)));
        match png {
            Ok(Some(png)) => {
                if let Err(e) = std::fs::write(png_dir.join(format!("page-{:02}.png", i + 1)), on_paper(&png).unwrap_or(png)) {
                    return Check::Fail(format!("writing page {} render: {e}", i + 1));
                }
            }
            Ok(None) => return Check::Fail(format!("page {} does not render", i + 1)),
            Err(p) => return Check::Fail(format!("rendering page {} panicked: {}", i + 1, panic_text(p))),
        }
    }
    Check::Pass
}

/// The table that `gate/results/<set>.md` records: hashes, labels and results, never file contents.
pub fn markdown(set: &str, pin: &str, results: &[FileResult]) -> String {
    let count = |f: &dyn Fn(&FileResult) -> bool| results.iter().filter(|r| f(r)).count();
    let mut s = format!("# Gate results: {set}\n\n");
    s += &format!("DesignCraft pin `{pin}`. Generated by `cargo xtask gate`; do not edit by hand.\n\n");
    s += &format!(
        "**{} of {} pass.** Opens {}, complete {}, exports {}, recognizable {}.\n\n",
        count(&|r| r.pass),
        results.len(),
        count(&|r| r.opens.is_pass()),
        count(&|r| r.complete.is_pass()),
        count(&|r| r.exports.is_pass()),
        count(&|r| r.recognizable.is_pass()),
    );
    s += &format!(
        "Completeness is judged against a layout export beside the file (`truth/<name>.json`) for {} files and against Pubwork's reader for {}.\n\n",
        count(&|r| r.reference == Some("layout export")),
        count(&|r| r.reference == Some("Pubwork")),
    );
    s += &format!(
        "Overset (text that doesn't fit its frame, so isn't in the PDF; reported, not failed on): {} characters in {} files.\n\n",
        results.iter().filter_map(|r| r.overset_chars).sum::<usize>(),
        count(&|r| r.overset_chars.is_some_and(|n| n > 0)),
    );
    s += "| File | SHA-256 | Pages | Overset | Opens | Complete | Exports | Recognizable | Result |\n|---|---|---|---|---|---|---|---|---|\n";
    for r in results {
        s += &format!(
            "| {} | `{}` | {} | {} | {} | {} | {} | {} | {} |\n",
            r.label,
            &r.sha256[..12],
            r.pages.map_or("–".into(), |p| p.to_string()),
            r.overset_chars.map_or("–".into(), |n| n.to_string()),
            r.opens.cell(),
            r.complete.cell(),
            r.exports.cell(),
            r.recognizable.cell(),
            if r.pass { "**PASS**" } else { "fail" }
        );
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(dir: &Path, verdicts: serde_json::Value) -> Options {
        Options { out: dir.to_path_buf(), names: true, verdicts, pubwork: Err("not in unit tests".into()) }
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("foliolum-gate-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    const HELLO: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../crates/foliolum-pub/tests/data/hello.pub");

    #[test]
    fn an_unreadable_file_fails_to_open_and_nothing_else_runs() {
        let d = scratch("empty");
        let f = d.join("empty.pub");
        std::fs::write(&f, b"").unwrap();
        let r = run_file(&f, &opts(&d, json!({})));
        assert!(matches!(r.opens, Check::Fail(_)));
        assert!(!r.pass);
    }

    #[test]
    fn a_good_file_without_reference_or_review_is_not_a_pass() {
        let d = scratch("hello");
        let r = run_file(Path::new(HELLO), &opts(&d, json!({})));
        assert_eq!(r.opens, Check::Pass);
        assert_eq!(r.exports, Check::Pass);
        assert!(matches!(r.complete, Check::Unchecked(_)));
        assert!(matches!(r.recognizable, Check::Unchecked(_)));
        assert!(!r.pass);
        assert!(!r.machine_checks_pass(), "no reference: completeness is unchecked, so not a pass");
        assert!(d.join("hello/page-01.png").exists());
        let render = tiny_skia::Pixmap::decode_png(&std::fs::read(d.join("hello/page-01.png")).unwrap()).unwrap();
        assert_eq!(render.pixel(0, 0).unwrap().demultiply(), tiny_skia::ColorU8::from_rgba(255, 255, 255, 255), "paper is white");
        // A reviewer's yes alone does not make it pass while completeness is unchecked.
        let r = run_file(Path::new(HELLO), &opts(&d, json!({ r.sha256.clone(): { "verdict": "recognizable" } })));
        assert_eq!(r.recognizable, Check::Pass);
        assert!(!r.pass);
    }

    #[test]
    fn a_reviewer_no_is_a_fail_with_the_note() {
        let d = scratch("no");
        let sha = sha256_hex(&std::fs::read(HELLO).unwrap());
        let r = run_file(Path::new(HELLO), &opts(&d, json!({ sha: { "verdict": "not-recognizable", "note": "headline cut off" } })));
        assert_eq!(r.recognizable, Check::Fail("headline cut off".into()));
    }

    #[test]
    fn overset_warnings_are_counted() {
        assert_eq!(overset_count("page 2: 37 character(s) of the story in frame 9 do not fit it and are not in the export"), Some(37));
        assert_eq!(overset_count("the story in frame 4: 12 character(s) do not fit and are not in the export"), Some(12));
        assert_eq!(overset_count("5 character(s) of a story do not fit their frame and are not in the export"), Some(5));
        assert_eq!(overset_count("something else"), None);
    }

    /// The reader, end to end (CI's dev22 step checks real files against it): with no layout export,
    /// Pubwork judges completeness.
    /// Needs the reader fetched (gate/fetch-fixtures.sh) and Node; skipped without them, except in CI.
    #[test]
    fn a_reader_that_finds_nothing_leaves_completeness_unchecked() {
        let reader = match pubwork::Reader::find(&crate::root()) {
            Ok(r) => r,
            Err(why) if std::env::var_os("CI").is_none() => {
                eprintln!("skipped: {why}");
                return;
            }
            Err(why) => panic!("{why}"),
        };
        let d = scratch("pubwork");
        let mut o = opts(&d, json!({}));
        o.pubwork = Ok(reader);
        // hello.pub is written from scratch by foliolum-pub's tests, and libmspub finds no pages in
        // it: a reference that sees nothing leaves completeness unchecked, never passed.
        let r = run_file(Path::new(HELLO), &o);
        assert_eq!(r.complete, Check::Unchecked("no layout export; Pubwork's reader found no pages in this file".into()));
        assert!(!r.machine_checks_pass());
    }

    #[test]
    fn labels_are_hashes_unless_names_are_asked_for() {
        let d = scratch("label");
        let mut o = opts(&d, json!({}));
        o.names = false;
        let r = run_file(Path::new(HELLO), &o);
        assert_eq!(r.label, r.sha256[..12]);
        assert!(!markdown("t", "pin", &[r]).contains("hello"));
    }
}
