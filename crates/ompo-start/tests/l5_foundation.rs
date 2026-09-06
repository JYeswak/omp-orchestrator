//! L5 FOUNDATION.jsonl append: stage=S1 row cites inception.json.

use ompo_start::{append_s1_foundation, s1_rows_citing_inception, INCEPTION_REF};

#[test]
fn writer_appends_s1_row_citing_inception() {
    let dir = tempfile::tempdir().expect("scratch");
    let path = dir.path().join("FOUNDATION.jsonl");
    assert!(append_s1_foundation(&path).expect("first append"));
    assert!(!append_s1_foundation(&path).expect("idempotent"));
    let jsonl = std::fs::read_to_string(&path).expect("read");
    let rows = s1_rows_citing_inception(&jsonl);
    assert_eq!(rows.len(), 1);
    let refs = rows[0]
        .get("output_refs")
        .and_then(|v| v.as_array())
        .expect("output_refs");
    assert!(
        refs.iter()
            .any(|r| r.as_str() == Some(INCEPTION_REF)),
        "{refs:?}"
    );
    assert_eq!(rows[0].get("stage").and_then(|v| v.as_str()), Some("S1"));
    println!("inception_ref {INCEPTION_REF}");
}
