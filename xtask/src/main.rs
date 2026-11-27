//! Foliolum's developer tasks. Run through the alias in `.cargo/config.toml`:
//!
//!   cargo xtask gate [--set NAME] [--names] [--record] [--out DIR] [--verdicts FILE] [--system-fonts] [--no-review] PATH...
//!
//! `gate` runs the Phase 1 gate (see gate.rs) over every .pub file in each PATH (a file or a folder).
//!   --set NAME      name of this file set; results go to target/gate/NAME/ (default "local")
//!   --names         label files by file name. Only for public sets: contributed files are labelled by
//!                   hash, because their names can say whose they are
//!   --record        also write gate/results/NAME.md, the table that is committed
//!   --out DIR       where PDFs, review PNGs and results.json go (default target/gate/NAME)
//!   --verdicts F    reviewer verdicts by SHA-256 (default gate/verdicts.json)
//!   --system-fonts  also use the fonts installed on this machine. By default the gate sees only
//!                   DesignCraft's and Foliolum's bundled fonts, as the browser does, so its results
//!                   don't depend on the machine that runs it
//!   --no-review     exit by the machine checks alone (opens, complete, exports), not the reviewer's
//!                   verdict. For CI on the public dev sets; the table still shows every check
//!
//! Completeness is judged against Publisher's layout export when the file has one, otherwise against
//! Pubwork's reader (pubwork.rs), which needs `gate/fetch-fixtures.sh` and Node (`node` on the path, or
//! `$FOLIOLUM_NODE`).
//!
//!   cargo xtask synth [--count N] [--seed S] OUT
//!   cargo xtask demo OUT
//!
//! `synth` writes the stress set (synth.rs): N synthetic publications (default 50, seed 1) made by
//! our own writer, with a reference beside each in the layout-export shape, for `gate` to check.
//! They test robustness, not fidelity to Publisher, and don't count toward the Phase 1 gate.
//!
//!   cargo xtask review [--out DIR] [--verdicts FILE] [--refs DIR]... SET...
//!
//! `review` builds target/gate/review.html (review.rs) from what `gate` left for each SET: every file
//! that exported, its page renders beside any Publisher reference (`<label>.ref-*` in each --refs DIR,
//! default fixtures/lobz-refs), with buttons for the reviewer's verdict. It saves a complete
//! verdicts.json to put in gate/. The page only works on this machine and is never published.
//!   --out DIR       the gate's output folder (default target/gate)
//!
//!   cargo xtask favicon
//!
//! `favicon` renders apps/foliolum-web/favicon.svg into favicon.ico beside it (favicon.rs). Run it
//! after changing the SVG; a test fails until you do.
//!
//! Exit codes: 0 every file passes, 1 usage or I/O error, 2 at least one file does not pass.
mod favicon;
mod gate;
mod pubwork;
mod review;
mod synth;
mod truth;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = pico_args::Arguments::from_env();
    match args.subcommand().ok().flatten().as_deref() {
        Some("gate") => match run_gate(args) {
            Ok(code) => code,
            Err(e) => {
                eprintln!("xtask gate: {e}");
                ExitCode::from(1)
            }
        },
        Some("demo") => match args.free_from_str::<std::path::PathBuf>() {
            Ok(out) => match std::fs::write(&out, synth::demo_bytes()) {
                Ok(()) => {
                    println!("demo bulletin in {}", out.display());
                    std::process::ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("xtask demo: {}: {e}", out.display());
                    std::process::ExitCode::FAILURE
                }
            },
            Err(e) => {
                eprintln!("usage: cargo xtask demo OUT ({e})");
                std::process::ExitCode::FAILURE
            }
        },
        Some("synth") => match run_synth(args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("xtask synth: {e}");
                ExitCode::from(1)
            }
        },
        Some("review") => match run_review(args) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("xtask review: {e}");
                ExitCode::from(1)
            }
        },
        Some("favicon") => {
            let web = Path::new(env!("CARGO_MANIFEST_DIR")).join("../apps/foliolum-web");
            match favicon::write(&web.join("favicon.svg"), &web.join("favicon.ico")) {
                Ok(()) => {
                    println!("wrote {}", web.join("favicon.ico").display());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("xtask favicon: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        _ => {
            eprintln!(
                "usage: cargo xtask gate [--set NAME] [--names] [--record] [--out DIR] [--verdicts FILE] [--system-fonts] [--no-review] PATH...\n       cargo xtask synth [--count N] [--seed S] OUT\n       cargo xtask demo OUT\n       cargo xtask review [--out DIR] [--verdicts FILE] [--refs DIR]... SET...\n       cargo xtask favicon"
            );
            ExitCode::from(1)
        }
    }
}

fn run_synth(mut args: pico_args::Arguments) -> Result<(), String> {
    let count: usize = args.opt_value_from_str("--count").map_err(|e| e.to_string())?.unwrap_or(50);
    let seed: u64 = args.opt_value_from_str("--seed").map_err(|e| e.to_string())?.unwrap_or(1);
    let out: PathBuf = args.free_from_str().map_err(|_| "give the folder to write the set to".to_string())?;
    synth::write_set(&out, count, seed)?;
    println!("{count} synthetic publications (seed {seed}) and their references in {}", out.display());
    Ok(())
}

fn run_review(mut args: pico_args::Arguments) -> Result<(), String> {
    let root = root();
    let out: PathBuf = args.opt_value_from_str("--out").map_err(|e| e.to_string())?.unwrap_or_else(|| root.join("target/gate"));
    let verdicts_path: PathBuf = args.opt_value_from_str("--verdicts").map_err(|e| e.to_string())?.unwrap_or_else(|| root.join("gate/verdicts.json"));
    let mut refs: Vec<PathBuf> = args.values_from_str("--refs").map_err(|e| e.to_string())?;
    if refs.is_empty() {
        refs.push(root.join("fixtures/lobz-refs"));
    }
    let sets: Vec<String> = args.finish().into_iter().map(|s| s.to_string_lossy().into_owned()).collect();
    if sets.is_empty() {
        return Err("name at least one set the gate has run, e.g. `cargo xtask review dev22 lobz`".into());
    }
    let verdicts = match std::fs::read_to_string(&verdicts_path) {
        Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{}: {e}", verdicts_path.display()))?,
        Err(_) => serde_json::json!({}),
    };
    let (page, n) = review::write(&out, &sets, &refs, &verdicts)?;
    println!("{n} files to review: open {}", page.display());
    println!("then save verdicts.json from the page over {} and run the gate again", verdicts_path.display());
    Ok(())
}

/// DesignCraft's switch for a font database without the machine's installed fonts.
const NO_SYSTEM_FONTS: (&str, &str) = ("DESIGNCRAFT_SYSTEM_FONTS", "0");

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask sits in the repo root").to_path_buf()
}

fn run_gate(mut args: pico_args::Arguments) -> Result<ExitCode, String> {
    // The font database reads the environment once, when first used; setting it in-process would
    // need unsafe code, so run again as a child with it set.
    if !args.contains("--system-fonts") && std::env::var_os(NO_SYSTEM_FONTS.0).is_none() {
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let status = std::process::Command::new(exe)
            .arg("gate")
            .args(args.finish())
            .env(NO_SYSTEM_FONTS.0, NO_SYSTEM_FONTS.1)
            .status()
            .map_err(|e| e.to_string())?;
        return Ok(ExitCode::from(status.code().unwrap_or(1).clamp(0, 255) as u8));
    }
    foliolum_fonts::install(designcraft_fonts::FontDb::global());
    let root = root();
    let set: String = args.opt_value_from_str("--set").map_err(|e| e.to_string())?.unwrap_or_else(|| "local".into());
    if !set.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(format!("--set {set:?}: use letters, digits, - and _"));
    }
    let names = args.contains("--names");
    let record = args.contains("--record");
    let no_review = args.contains("--no-review");
    let out: PathBuf = args.opt_value_from_str("--out").map_err(|e| e.to_string())?.unwrap_or_else(|| root.join("target/gate").join(&set));
    let verdicts_path: PathBuf = args.opt_value_from_str("--verdicts").map_err(|e| e.to_string())?.unwrap_or_else(|| root.join("gate/verdicts.json"));
    let paths: Vec<PathBuf> = args.finish().into_iter().map(PathBuf::from).collect();
    if paths.is_empty() {
        return Err("give at least one file or folder of .pub files".into());
    }

    let mut files = Vec::new();
    for p in &paths {
        if p.is_dir() {
            for e in std::fs::read_dir(p).map_err(|e| format!("{}: {e}", p.display()))?.flatten() {
                files.push(e.path());
            }
        } else {
            files.push(p.clone());
        }
    }
    files.retain(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("pub")));
    files.sort();
    if files.is_empty() {
        return Err("no .pub files found".into());
    }

    let verdicts = match std::fs::read_to_string(&verdicts_path) {
        Ok(t) => serde_json::from_str(&t).map_err(|e| format!("{}: {e}", verdicts_path.display()))?,
        Err(_) => serde_json::json!({}),
    };
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let reader = pubwork::Reader::find(&root);
    if let Err(why) = &reader {
        eprintln!("note: {why}; files without a layout export can't be checked for completeness");
    }
    let opts = gate::Options { out: out.clone(), names, verdicts, pubwork: reader };

    // The importer and renderer are caught per file; keep their panic messages out of the report stream.
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|_| {}));
    let mut results = Vec::new();
    for f in &files {
        let r = gate::run_file(f, &opts);
        eprintln!("{} {}", if r.pass { "PASS" } else { "fail" }, r.label);
        results.push(r);
    }
    std::panic::set_hook(default_hook);

    let mut jsonl = String::new();
    for r in &results {
        jsonl += &serde_json::to_string(r).map_err(|e| e.to_string())?;
        jsonl.push('\n');
    }
    std::fs::write(out.join("results.jsonl"), jsonl).map_err(|e| e.to_string())?;
    let table = gate::markdown(&set, &pin(&root), &results);
    std::fs::write(out.join("results.md"), &table).map_err(|e| e.to_string())?;
    if record {
        let dir = root.join("gate/results");
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        std::fs::write(dir.join(format!("{set}.md")), &table).map_err(|e| e.to_string())?;
    }
    let passed = results.iter().filter(|r| r.pass).count();
    println!("{passed} of {} pass; table: {}", results.len(), out.join("results.md").display());
    if no_review {
        let machine = results.iter().filter(|r| r.machine_checks_pass()).count();
        println!("{machine} of {} pass the machine checks (opens, complete, exports)", results.len());
        for r in results.iter().filter(|r| !r.machine_checks_pass()) {
            eprintln!("machine checks fail: {}", r.label);
        }
        return Ok(if machine == results.len() { ExitCode::SUCCESS } else { ExitCode::from(2) });
    }
    Ok(if passed == results.len() { ExitCode::SUCCESS } else { ExitCode::from(2) })
}

fn pin(root: &Path) -> String {
    std::fs::read_to_string(root.join("fork/pins.env"))
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("UPSTREAM_COMMIT=").map(|s| s[..12.min(s.len())].to_string())))
        .unwrap_or_else(|| "unknown".into())
}
