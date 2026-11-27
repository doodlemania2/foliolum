//! Opening a Publisher file: the one thing Foliolum's shell does before DesignCraft sees a file.
//!
//! DesignCraft's open routing doesn't know .pub, so the shell takes .pub files out of the inbox (picked
//! with File ▸ Open, or dropped), imports them with foliolum-pub, registers substitutes for the fonts
//! they name, and adds the document to the session. Everything else goes on to DesignCraft unchanged.
use designcraft_engine::{DocState, Session};
use designcraft_ui_egui::ImportedFile;

/// Is `name` a Publisher file, by its extension?
pub fn is_pub(name: &str) -> bool {
    std::path::Path::new(name).extension().is_some_and(|e| e.eq_ignore_ascii_case("pub"))
}

/// Imports `bytes` as the Publisher file `name` and opens it in `session`. Returns the status line:
/// how many pages came over, and how many things the importer couldn't bring across.
pub fn open_pub(session: &mut Session, name: &str, bytes: &[u8]) -> Result<String, String> {
    let imported = foliolum_pub::import_pub(bytes).map_err(|e| format!("Couldn't open {name}: {e}"))?;
    let mut doc = imported.document;
    doc.title = std::path::Path::new(name).file_stem().map_or_else(|| name.to_string(), |s| s.to_string_lossy().into_owned());
    let families = foliolum_fonts::document_families(&doc);
    foliolum_fonts::register_families(designcraft_fonts::FontDb::global(), families.keys().map(String::as_str));
    let pages = doc.page_count();
    session.add_document(DocState::new(doc, None));
    let mut status = format!("Opened {name}: {pages} page{}", if pages == 1 { "" } else { "s" });
    if !imported.warnings.is_empty() {
        let n = imported.warnings.len();
        status += &format!(", {n} thing{} not brought over exactly", if n == 1 { "" } else { "s" });
    }
    Ok(status)
}

/// Opens every Publisher file in `files` and returns the rest, in order, for DesignCraft. Each file's
/// status (or error) goes to `report`.
pub fn take_pubs(session: &mut Session, files: Vec<ImportedFile>, mut report: impl FnMut(String)) -> Vec<ImportedFile> {
    let mut rest = Vec::with_capacity(files.len());
    for f in files {
        if is_pub(&f.name) {
            report(open_pub(session, &f.name, &f.bytes).unwrap_or_else(|e| e));
        } else {
            rest.push(f);
        }
    }
    rest
}

#[cfg(test)]
mod tests {
    use super::*;

    const HELLO: &[u8] = include_bytes!("../../../crates/foliolum-pub/tests/data/hello.pub");

    #[test]
    fn pub_files_are_recognised_by_extension() {
        assert!(is_pub("Bulletin.pub") && is_pub("OLD.PUB"));
        assert!(!is_pub("Bulletin.pub.pdf") && !is_pub("pub") && !is_pub("Bulletin.designcraft"));
    }

    #[test]
    fn a_pub_opens_as_a_titled_document() {
        let mut session = Session::new();
        let status = open_pub(&mut session, "hello.pub", HELLO).unwrap();
        assert!(status.starts_with("Opened hello.pub: 1 page"), "{status}");
        assert_eq!(session.documents().len(), 1);
        assert_eq!(session.doc().unwrap().doc.title, "hello");
    }

    #[test]
    fn only_publisher_files_are_taken_from_the_inbox() {
        use designcraft_ui_egui::ImportRequest;
        let file = |name: &str, bytes: &[u8]| ImportedFile {
            request: ImportRequest { purpose: "drop".into(), target: None, wanted: Vec::new() },
            name: name.into(),
            bytes: bytes.to_vec(),
        };
        let mut session = Session::new();
        let mut reports = Vec::new();
        let rest = take_pubs(&mut session, vec![file("a.png", b"png"), file("hello.pub", HELLO), file("b.designcraft", b"dc")], |s| reports.push(s));
        assert_eq!(rest.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["a.png", "b.designcraft"]);
        assert_eq!(session.documents().len(), 1);
        assert_eq!(reports.len(), 1);
    }

    #[test]
    fn a_broken_pub_reports_an_error_and_opens_nothing() {
        let mut session = Session::new();
        let err = open_pub(&mut session, "broken.pub", b"not a publisher file").unwrap_err();
        assert!(err.starts_with("Couldn't open broken.pub"), "{err}");
        assert!(session.documents().is_empty());
    }
}
