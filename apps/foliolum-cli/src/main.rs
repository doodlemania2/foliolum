//! Foliolum headless tools.
//!
//! foliolum-cli convert IN.pub OUT.pdf    import a Publisher file and export a print PDF
//! foliolum-cli inspect IN.pub            print one JSON line: pages, objects, warnings
//!
//! Exit codes: 0 ok, 1 usage, 2 import failed, 3 export failed.
use designcraft_engine::{DocState, Session};
use serde_json::json;
use std::process::ExitCode;

const USAGE: &str = "usage: foliolum-cli convert IN.pub OUT.pdf | inspect IN.pub";

fn main() -> ExitCode {
    foliolum_fonts::install(designcraft_fonts::FontDb::global());
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["convert", input, output] => convert(input, Some(output)),
        ["inspect", input] => convert(input, None),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(1)
        }
    }
}

fn convert(input: &str, output: Option<&str>) -> ExitCode {
    let bytes = match std::fs::read(input) {
        Ok(b) => b,
        Err(e) => return report(input, "read-failed", json!({ "error": e.to_string() }), 2),
    };
    let imported = match foliolum_pub::import_pub(&bytes) {
        Ok(i) => i,
        Err(e) => return report(input, "import-failed", json!({ "error": e.to_string() }), 2),
    };
    let families = foliolum_fonts::document_families(&imported.document);
    foliolum_fonts::register_families(designcraft_fonts::FontDb::global(), families.keys().map(String::as_str));
    let mut objects = 0usize;
    for item in imported.document.spreads.iter().flat_map(|s| &s.items) {
        item.walk(&mut |_| objects += 1);
    }
    let summary = json!({
        "pages": imported.document.page_count(),
        "objects": objects,
        "warnings": imported.warnings,
    });
    let Some(output) = output else { return report(input, "ok", summary, 0) };
    let mut session = Session::new();
    session.add_document(DocState::new(imported.document, None));
    match session.execute("file.exportPdf", &json!({ "path": output })) {
        Ok(_) => report(input, "ok", summary, 0),
        Err(e) => report(input, "export-failed", json!({ "error": e.to_string(), "import": summary }), 3),
    }
}

fn report(file: &str, status: &str, detail: serde_json::Value, code: u8) -> ExitCode {
    println!("{}", json!({ "file": file, "status": status, "detail": detail }));
    ExitCode::from(code)
}
