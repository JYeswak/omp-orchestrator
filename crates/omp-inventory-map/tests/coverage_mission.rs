use omp_inventory_map::{CoverageTableError, parse_omp_coverage_table};
use std::collections::BTreeSet;

const RUNTIME: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/plan/ipg11-coverage.json"
));
const RUNTIME_CONTRACT_COLUMNS: &[&str] = &[
    "contract_1_asupersync_native",
    "contract_2_unsafe_code_forbid",
    "contract_3_cancel_correct",
    "contract_4_typed_exhaustive",
    "contract_5_logged_typed_rows",
    "contract_6_observable_own_predicates",
    "contract_7_robot_reachable",
    "contract_8_wired_with_empty_unwired_allowance",
];
const TABLE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../docs/plan/OMP-COVERAGE-TABLE.jsonl"
));
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
        BTreeSet::from([
            "omp:async",
            "omp:auto-thinking",
            "omp:cleanse",
            "omp:commit",
            "omp:compress",
            "omp:edit",
            "omp:lib",
            "omp:lsp",
            "omp:markit",
            "omp:tiny",
            "omp:utils",
            "omp:vibe",
        ])
    );
    assert_eq!(
        rows.iter().filter(|row| row.classification == "b").count(),
        1
    );
    assert!(rows.iter().any(|row| row.coverage == "FULLY COVERED"));
    let commit = rows
        .iter()
        .find(|row| row.surface == "omp:commit")
        .expect("commit row");
    assert!(
        commit
            .omp_alternative
            .as_deref()
            .is_some_and(|alternative| alternative.contains("OMP"))
    );
}

#[test]
fn runtime_wave_has_six_nonvacuous_rows_and_eight_contract_columns() {
    let document: serde_json::Value =
        serde_json::from_str(RUNTIME).expect("runtime coverage artifact parses");
    let rows = document["rows"]
        .as_array()
        .expect("runtime coverage rows are an array");
    assert_eq!(rows.len(), 6, "all six runtime surfaces must be enumerated");
    let surfaces: BTreeSet<_> = rows
        .iter()
        .map(|row| row["surface"].as_str().unwrap_or(""))
        .collect();
    assert_eq!(
        surfaces,
        BTreeSet::from(["async", "auto-thinking", "lib", "tiny", "utils", "vibe"])
    );
    for row in rows {
        for column in RUNTIME_CONTRACT_COLUMNS {
            assert!(
                row.get(*column)
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|value| !value.trim().is_empty()),
                "runtime row {} is missing contract column {}",
                row["surface"],
                column
            );
        }
        assert!(matches!(
            row["classification"].as_str(),
            Some("a" | "b" | "c")
        ));
        if row["classification"] == "b" {
            assert!(
                row["omp_alternative"]
                    .as_str()
                    .is_some_and(|value| !value.trim().is_empty()),
                "category-b row {} must name its OMP alternative",
                row["surface"]
            );
        }
        assert!(row["omp_files"].as_u64().is_some_and(|count| count > 0));
    }
    assert!(
        rows.iter()
            .any(|row| row["surface"] == "async" && row["coverage"] == "FULLY COVERED"),
        "async must provide the positive control"
    );
    assert_eq!(document["anti_vacuity"]["surfaces_enumerated"], 6);
    assert_eq!(document["anti_vacuity"]["files_enumerated"], 60);
    assert_eq!(document["anti_vacuity"]["zero_surfaces"], "ERROR");
}
