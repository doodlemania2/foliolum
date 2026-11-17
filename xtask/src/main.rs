//! Foliolum's developer tasks. Run through the alias in `.cargo/config.toml`:
//!
//!   cargo xtask gate [--set NAME] [--names] [--record] [--out DIR] [--verdicts FILE] [--system-fonts] PATH...
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
//!
//! Exit codes: 0 every file passes, 1 usage or I/O error, 2 at least one file does not pass.
mod gate;
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
        _ => {
            eprintln!("usage: cargo xtask gate [--set NAME] [--names] [--record] [--out DIR] [--verdicts FILE] [--system-fonts] PATH...");
            ExitCode::from(1)
        }
    }
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
    let opts = gate::Options { out: out.clone(), names, verdicts };

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
    Ok(if passed == results.len() { ExitCode::SUCCESS } else { ExitCode::from(2) })
}

fn pin(root: &Path) -> String {
    std::fs::read_to_string(root.join("fork/pins.env"))
        .ok()
        .and_then(|t| t.lines().find_map(|l| l.strip_prefix("UPSTREAM_COMMIT=").map(|s| s[..12.min(s.len())].to_string())))
        .unwrap_or_else(|| "unknown".into())
}
