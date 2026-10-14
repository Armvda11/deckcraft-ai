use deckcraft_engine::Session;
use serde_json::{Value, json};

fn inspect(s: &mut Session) -> Value {
    s.execute("document.inspect", &json!({})).unwrap()
}
fn setup() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({})).unwrap();
    s
}

#[test]
fn content_revision_ignores_selection_but_tracks_edit_undo_and_redo() {
    let mut s = setup();
    let before = inspect(&mut s)["contentRevision"].as_u64().unwrap();
    s.execute("edit.selectAll", &json!({})).unwrap();
    assert_eq!(inspect(&mut s)["contentRevision"], before);
    s.execute("shape.insert", &json!({"preset":"rect", "rect":[20,20,100,80]})).unwrap();
    let edited = inspect(&mut s)["contentRevision"].as_u64().unwrap();
    assert!(edited > before);
    s.execute("edit.undo", &json!({})).unwrap();
    let undone = inspect(&mut s)["contentRevision"].as_u64().unwrap();
    assert!(undone > edited);
    s.execute("edit.redo", &json!({})).unwrap();
    assert!(inspect(&mut s)["contentRevision"].as_u64().unwrap() > undone);
}

#[test]
fn candidate_is_atomic_undoable_and_rejects_stale_content() {
    let mut original = setup();
    let info = inspect(&mut original);
    let before = original.doc().unwrap().doc.clone();
    let mut candidate = setup();
    candidate.execute("slide.new", &json!({"title":"AI draft"})).unwrap();
    let bytes = candidate.execute("file.saveBytes", &json!({})).unwrap()["data"].clone();
    let request = json!({"documentUid":info["documentUid"], "contentRevision":info["contentRevision"], "data":bytes});
    original.execute("edit.selectAll", &json!({})).unwrap();
    original.execute("document.applyCandidate", &request).unwrap();
    assert_eq!(original.doc().unwrap().doc.slides.len(), 2);
    assert_eq!(original.doc().unwrap().history.undo.len(), 1);
    assert!(original.execute("document.applyCandidate", &request).is_err());
    original.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(*original.doc().unwrap().doc, *before);
    original.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(original.doc().unwrap().doc.slides.len(), 2);
}

#[test]
fn invalid_candidate_preserves_document_and_history() {
    let mut s = setup();
    let info = inspect(&mut s);
    let before = s.doc().unwrap().doc.clone();
    assert!(
        s.execute(
            "document.applyCandidate",
            &json!({"documentUid":info["documentUid"], "contentRevision":info["contentRevision"], "data":"invalid"})
        )
        .is_err()
    );
    assert_eq!(*s.doc().unwrap().doc, *before);
    assert!(s.doc().unwrap().history.undo.is_empty());
}
