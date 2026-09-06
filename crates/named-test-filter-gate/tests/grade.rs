//! Vacuous cargo filters must refuse. Real passing names must still admit.

use input_manifest::{CargoInputBound, InputManifest};
use named_test_filter_gate::{
    census_beads, grade, grade_with_bound, implemented_test_fns, named_tests_in, parse_tally,
    Grade, GradeError,
};

const VACUOUS: &str =
    "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out\n";

const REAL_PASS: &str =
    "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n";

#[test]
fn known_bad_missing_name_zero_passed_refuses() {
    match grade(VACUOUS, 0) {
        Grade::Refuse {
            error:
                GradeError::Vacuous {
                    passed: 0,
                    filtered: 2,
                },
            manifest: InputManifest::Full,
        } => {
            println!("refused vacuous 0 passed / 2 filtered, exit 0");
        }
        other => panic!("missing name must refuse, got {other:?}"),
    }
}

#[test]
fn known_good_existing_name_admits() {
    match grade(REAL_PASS, 0) {
        Grade::Admit {
            passed: 1,
            manifest: InputManifest::Full,
        } => println!("admitted 1 passed"),
        other => panic!("real pass must admit, got {other:?}"),
    }
}

#[test]
fn unparseable_is_named_error_never_pass() {
    match grade("error: could not compile `ompo-start`", 101) {
        Grade::Refuse {
            error: GradeError::Unparseable,
            manifest: InputManifest::Refused { .. },
        } => {
            println!("unparseable compile log is named error");
        }
        other => panic!("unparseable must not pass, got {other:?}"),
    }
    assert!(parse_tally("").is_err());
}

#[test]
fn partial_grade_cannot_close_a_bead() {
    let output = concat!(
        "test result: ok. 26 passed; 0 failed; 0 ignored; 0 measured; 10 filtered out\n",
        "test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 2 filtered out\n",
    );
    let grade = grade_with_bound(output, 0, CargoInputBound::Head { lines: 1 });
    match &grade {
        Grade::Admit {
            passed: 26,
            manifest:
                InputManifest::Partial {
                    bound_kind,
                    bound_value: 2,
                    ..
                },
        } => assert_eq!(bound_kind, "head -1 of N target lines"),
        other => panic!("bounded grade must be partial, got {other:?}"),
    }
    assert!(
        grade.acceptance_evidence().is_err(),
        "a partial grade must not close a bead"
    );
}

#[test]
fn named_tests_in_extracts_filter_fn() {
    let text =
        "cargo test -p ompo-start --test l3_step_parity both_renderers_borrow_the_same_array";
    let names = named_tests_in(text);
    assert!(
        names
            .iter()
            .any(|n| n == "both_renderers_borrow_the_same_array"),
        "{names:?}"
    );
}

#[test]
fn census_reports_unresolved_without_inventing_tests() {
    let jsonl = concat!(
        r#"{"id":"omp-orchestrator-s1-l3-array-zr1g","acceptance_criteria":"cargo test -p ompo-start --test l3_step_parity both_renderers_borrow_the_same_array","description":"","title":"x"}"#,
        "\n",
        r#"{"id":"omp-orchestrator-s1-l3-skipped-t4k9","acceptance_criteria":"cargo test -p ompo-start --test l3_step_parity skipped_steps_remain_in_both_renders","description":"","title":"x"}"#,
        "\n"
    );
    let implemented = vec!["skipped_steps_remain_in_both_renders".to_owned()];
    let rows = census_beads(jsonl, &implemented).expect("census");
    let zr1g = rows
        .iter()
        .find(|row| row.test_fn == "both_renderers_borrow_the_same_array")
        .expect("zr1g name");
    assert!(!zr1g.resolved, "do not invent the missing test");
    let t4k9 = rows
        .iter()
        .find(|row| row.test_fn == "skipped_steps_remain_in_both_renders")
        .expect("t4k9 name");
    assert!(t4k9.resolved);
}

#[test]
fn live_tree_census_does_not_unpark() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let jsonl = std::fs::read_to_string(root.join(".beads/issues.jsonl")).expect("issues.jsonl");
    let implemented = implemented_test_fns(&root.join("crates")).expect("tests/");
    let rows = census_beads(&jsonl, &implemented).expect("repo census");
    let unresolved: Vec<_> = rows.iter().filter(|row| !row.resolved).collect();
    println!(
        "named_test_refs={} unresolved={} implemented_fns={}",
        rows.len(),
        unresolved.len(),
        implemented.len()
    );
    for row in unresolved.iter().take(40) {
        println!("UNRESOLVED {} {}", row.bead_id, row.test_fn);
    }
    assert!(
        implemented
            .iter()
            .any(|n| n == "skipped_steps_remain_in_both_renders"),
        "positive control: skipped_steps exists"
    );
    assert!(
        !implemented
            .iter()
            .any(|n| n == "both_renderers_borrow_the_same_array"),
        "do not invent both_renderers_borrow_the_same_array"
    );
}
