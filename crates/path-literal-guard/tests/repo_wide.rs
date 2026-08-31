//! The repo-wide home-path-literal gate (bead omp-orchestrator-npq, acceptance #2).
//!
//! Asserts the count over `<repo>/crates/*/src` is zero, prints the scan set with the
//! verdict so a reader can see exactly what was covered, and treats an EMPTY scan set
//! as an ERROR — never a pass. A reintroduced literal turns this test RED and the
//! failure message names the file and line (acceptance #3).

use path_literal_guard::{repo_root, scan};

#[test]
fn zero_home_path_literals_across_crates_src() {
    let report = scan(&repo_root());

    // Print the scan set: a verdict without its coverage is unauditable.
    println!("PATH-LITERAL-GATE scan set ({} .rs files under crates/*/src):", report.scanned.len());
    for file in &report.scanned {
        println!("  {}", file.display());
    }

    // Anti-vacuity: an empty scan set is an ERROR, never a pass. A repo whose crates
    // tree vanished (or a gate pointed at the wrong root) must fail loudly here.
    assert!(
        !report.scanned.is_empty(),
        "PATH-LITERAL-GATE RED: empty scan set — no crates/*/src found under {}",
        repo_root().display()
    );

    assert!(
        report.hits.is_empty(),
        "PATH-LITERAL-GATE RED: hardcoded home-path literal(s) in crates/*/src \
         (omp-orchestrator-npq: a hardcoded root compiles after a move and then \
         silently reads the wrong repo): {:?}",
        report
            .hits
            .iter()
            .map(std::string::ToString::to_string)
            .collect::<Vec<String>>()
    );

    println!("PATH-LITERAL-GATE PASS: {} files scanned, zero home-path literals", report.scanned.len());
}
