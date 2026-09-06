#![forbid(unsafe_code)]

use omp_orchestrator::{census_gates, GateReachability};
use std::path::PathBuf;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("workspace root")
        .to_path_buf()
}

#[test]
fn worker_oracle_has_a_named_reachable_supervisor_trigger() {
    let row = census_gates(&repo_root())
        .rows
        .into_iter()
        .find(|row| row.gate == "worker-oracle-gate")
        .expect("worker oracle must be visible in the supervisor census");
    match row.reachability {
        GateReachability::Reachable { trigger } => {
            assert!(trigger.contains("supervisor:census_gates"), "trigger={trigger}");
            assert!(trigger.contains("worker-oracle-gate::census"), "trigger={trigger}");
            assert!(trigger.contains("ledger_targets="), "trigger={trigger}");
        }
        other => panic!("worker oracle census row is not reachable: {other:?}"),
    }
}

#[test]
fn worker_oracle_admission_reads_the_same_ledger() {
    let report = worker_oracle_gate::admission_check(&repo_root())
        .expect("the current ledger must be readable");
    assert_eq!(report.ledger_host_bound, report.marker_matches);
    assert_eq!(report.ledger_host_bound, report.source_host_bound);
    assert!(report.ledger_host_bound > 0, "host-bound ledger must not be vacuous");
}
