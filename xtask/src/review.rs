//! `cargo xtask review`: one local HTML page for giving the gate's fourth check, the reviewer's
//! "recognizable" verdict (gate/README.md, "Recording a verdict").
//!
//! It reads what `cargo xtask gate` left in `target/gate/<set>/` (results.jsonl and the page renders)
//! for each set named, finds any reference images beside the files (`<label>.ref-<id>.<ext>` in a
//! references folder: Publisher's own PDF or a screenshot of the file in Publisher), and writes
//! `target/gate/review.html`. The page shows each file's renders beside its references, takes a
//! verdict and a note per file, and saves them as a complete `verdicts.json` to put in `gate/`.
//!
//! Nothing leaves the machine: the page loads the renders and references by relative path from
//! `target/gate/`, and keeps verdicts in the browser until they are saved. The files carry real names,
//! so the page is never published.
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::Value;

const TEMPLATE: &str = include_str!("review.html");

#[derive(Serialize)]
struct Entry {
    set: String,
    label: String,
    sha256: String,
    pages: usize,
    /// Page renders, relative to the page.
    renders: Vec<String>,
    /// Reference images and PDFs, relative to the page.
    refs: Vec<String>,
    complete: Value,
    overset: Option<u64>,
    warnings: Vec<String>,
}

/// Builds the page from `gate_dir` (normally target/gate) for `sets`, with references from
/// `ref_dirs`, and returns where it was written and how many files it holds.
pub fn write(gate_dir: &Path, sets: &[String], ref_dirs: &[PathBuf], verdicts: &Value) -> Result<(PathBuf, usize), String> {
    let refs_out = gate_dir.join("refs");
    let mut entries = Vec::new();
    for set in sets {
        let dir = gate_dir.join(set);
        let jsonl = dir.join("results.jsonl");
        let text = std::fs::read_to_string(&jsonl).map_err(|e| format!("{}: {e} (run `cargo xtask gate --set {set} …` first)", jsonl.display()))?;
        for line in text.lines().filter(|l| !l.trim().is_empty()) {
            let r: Value = serde_json::from_str(line).map_err(|e| format!("{}: {e}", jsonl.display()))?;
            // Only a file that exported has pages to look at; the gate shows the rest as "nothing to review".
            if r["exports"]["status"] != "pass" {
                continue;
            }
            let label = r["label"].as_str().unwrap_or_default().to_string();
            let mut renders = Vec::new();
            for n in 1.. {
                let name = format!("page-{n:02}.png");
                if !dir.join(&label).join(&name).is_file() {
                    break;
                }
                renders.push(format!("{set}/{label}/{name}"));
            }
            let refs = copy_refs(&label, ref_dirs, &refs_out)?;
            entries.push(Entry {
                set: set.clone(),
                sha256: r["sha256"].as_str().unwrap_or_default().to_string(),
                pages: r["pages"].as_u64().unwrap_or(0) as usize,
                renders,
                refs,
                complete: r["complete"].clone(),
                overset: r["overset_chars"].as_u64(),
                warnings: r["warning_text"].as_array().map(|a| a.iter().filter_map(|w| w.as_str().map(str::to_string)).collect()).unwrap_or_default(),
                label,
            });
        }
    }
    let data = serde_json::json!({ "files": entries, "verdicts": verdicts });
    // Inside a <script> element, so no "</" may appear in the JSON text.
    let json = serde_json::to_string(&data).map_err(|e| e.to_string())?.replace("</", "<\\/");
    let html = TEMPLATE.replacen("/*DATA*/null", &json, 1);
    let out = gate_dir.join("review.html");
    std::fs::write(&out, html).map_err(|e| format!("{}: {e}", out.display()))?;
    Ok((out, entries.len()))
}

/// Copies the references for `label` (files named `<label>.ref-*`) into `out`, returning their paths
/// relative to the page.
fn copy_refs(label: &str, ref_dirs: &[PathBuf], out: &Path) -> Result<Vec<String>, String> {
    let prefix = format!("{label}.ref-");
    let mut found = Vec::new();
    for dir in ref_dirs {
        let Ok(rd) = std::fs::read_dir(dir) else { continue };
        for e in rd.flatten() {
            let name = e.file_name().to_string_lossy().to_string();
            if name.starts_with(&prefix) {
                found.push((name, e.path()));
            }
        }
    }
    found.sort();
    let mut rel = Vec::new();
    for (name, path) in found {
        std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
        std::fs::copy(&path, out.join(&name)).map_err(|e| format!("{}: {e}", path.display()))?;
        rel.push(format!("refs/{name}"));
    }
    Ok(rel)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_page_lists_exported_files_with_their_renders_references_and_verdicts() {
        let d = std::env::temp_dir().join(format!("foliolum-review-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let gate = d.join("gate");
        std::fs::create_dir_all(gate.join("s/one")).unwrap();
        std::fs::write(gate.join("s/one/page-01.png"), b"png").unwrap();
        std::fs::write(gate.join("s/one/page-02.png"), b"png").unwrap();
        let lines = [
            r#"{"label":"one","sha256":"aa","pages":2,"exports":{"status":"pass"},"complete":{"status":"pass"},"overset_chars":3,"warning_text":["w </script>"]}"#,
            r#"{"label":"two","sha256":"bb","pages":null,"exports":{"status":"unchecked","reason":"did not open"}}"#,
        ];
        std::fs::write(gate.join("s/results.jsonl"), lines.join("\n")).unwrap();
        let refs = d.join("refs");
        std::fs::create_dir_all(&refs).unwrap();
        std::fs::write(refs.join("one.ref-7.pdf"), b"pdf").unwrap();
        std::fs::write(refs.join("onex.ref-8.png"), b"not this file's").unwrap();

        let verdicts = serde_json::json!({ "aa": { "verdict": "recognizable" } });
        let (out, n) = write(&gate, &["s".into()], &[refs], &verdicts).unwrap();
        assert_eq!(n, 1, "a file that did not export has nothing to review");
        let html = std::fs::read_to_string(out).unwrap();
        assert!(html.contains(r#""renders":["s/one/page-01.png","s/one/page-02.png"]"#));
        assert!(html.contains(r#""refs":["refs/one.ref-7.pdf"]"#));
        assert!(html.contains(r#""aa":{"verdict":"recognizable"}"#));
        assert!(!html.contains("/*DATA*/"));
        assert!(!html.contains("w </script>"), "data can't end the script element early");
        assert!(gate.join("refs/one.ref-7.pdf").is_file());
        assert!(!gate.join("refs/onex.ref-8.png").exists());
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn a_set_the_gate_has_not_run_is_an_error_that_says_so() {
        let d = std::env::temp_dir().join(format!("foliolum-review-missing-{}", std::process::id()));
        let e = write(&d, &["nope".into()], &[], &Value::Null).err().unwrap();
        assert!(e.contains("cargo xtask gate --set nope"), "{e}");
    }
}
