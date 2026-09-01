use omp_inventory_map::{parse_omp_coverage_table, CoverageTableError};
use std::collections::BTreeSet;

const TABLE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/plan/OMP-COVERAGE-TABLE.jsonl"));
const ROW: &str = r#"{"surface":"surface:type_root:edit","tool":"omp","kind":"type_root","batch":9,"maps_to_crate":null,"disposition":"RETIRE","validated_by":"probe","graded_by":"ipg.8","classification":"a","classification_note":"a — not ours","omp_alternative":null,"coverage":"FULLY COVERED"}"#;

#[test]
fn positive_control_is_fully_covered() {
    let rows = parse_omp_coverage_table(ROW).expect("one complete row parses");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].coverage, "FULLY COVERED");
}

#[test]
fn empty_surface_table_is_a_typed_error() {
    assert_eq!(
        parse_omp_coverage_table("\n"),
        Err(CoverageTableError::Empty)
    );
}

#[test]
fn scraping_classification_requires_named_omp_alternative() {
    let row = ROW.replace("\"classification\":\"a\"", "\"classification\":\"b\"");
    assert!(matches!(
        parse_omp_coverage_table(&row),
        Err(CoverageTableError::MissingOmpAlternative { .. })
    ));
}

#[test]
fn actual_table_covers_exact_named_surfaces() {
    let rows = parse_omp_coverage_table(TABLE).expect("coverage table parses");
    let names: BTreeSet<_> = rows.iter().map(|row| row.surface.as_str()).collect();
    assert_eq!(
        names,
        BTreeSet::from(["omp:cleanse", "omp:commit", "omp:compress", "omp:edit", "omp:lsp", "omp:markit"])
    );
    assert_eq!(rows.iter().filter(|row| row.classification == "b").count(), 1);
    assert!(rows.iter().any(|row| row.coverage == "FULLY COVERED"));
    let commit = rows.iter().find(|row| row.surface == "omp:commit").expect("commit row");
    assert!(commit.omp_alternative.as_deref().is_some_and(|alternative| alternative.contains("OMP")));
}
