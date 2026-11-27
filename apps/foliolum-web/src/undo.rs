//! One undo step for an action made of several edits (Smart frame, Our details, Make it fit), so a
//! single Undo takes back the whole action, as a person expects.
use std::sync::Arc;

use designcraft_engine::{HistoryEntry, Session};

/// Runs `f`, then replaces whatever undo steps it recorded (or none, for direct edits) with one
/// step named `label` that returns to the document as it was before. Nothing is recorded when the
/// document didn't change.
pub fn as_one_step<T>(s: &mut Session, label: &str, f: impl FnOnce(&mut Session) -> Result<T, String>) -> Result<T, String> {
    let before = s.doc().ok().map(|d| (d.doc.clone(), d.selection.clone(), d.history.undo.len()));
    let result = f(s);
    if let (Some((doc, selection, depth)), Ok(st)) = (before, s.doc_mut())
        && !Arc::ptr_eq(&doc, &st.doc)
        && *doc != *st.doc
    {
        st.history.undo.truncate(depth);
        st.history.undo.push(HistoryEntry { label: label.to_string(), doc, selection });
        st.history.redo.clear();
        let limit = st.history.limit.max(1);
        if st.history.undo.len() > limit {
            let extra = st.history.undo.len() - limit;
            st.history.undo.drain(..extra);
        }
    }
    result
}
