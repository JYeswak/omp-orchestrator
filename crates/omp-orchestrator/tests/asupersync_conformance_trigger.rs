//! Source-level proof for the resident supervisor's asupersync conformance trigger.
//!
//! NO-CLAIM: this proves command spelling and reachability only. It does not prove that the
//! conformance scanner or its generated table is correct.

const SUPERVISOR_SOURCE: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"));

#[test]
fn run_cycle_owns_reachable_asupersync_conformance_check() {
    let cycle_start = SUPERVISOR_SOURCE
        .find("async fn run_cycle")
        .expect("run_cycle remains the resident cycle entrypoint");
    let supervisor_start = SUPERVISOR_SOURCE[cycle_start..]
        .find("async fn run_supervisor")
        .map(|offset| cycle_start + offset)
        .expect("run_supervisor remains the cycle caller");
    let cycle = &SUPERVISOR_SOURCE[cycle_start..supervisor_start];
    let supervisor = &SUPERVISOR_SOURCE[supervisor_start..];

    assert!(
        cycle.contains("invoke(cx, config, \"asupersync-conformance\", &conformance_args).await"),
        "run_cycle must invoke the existing bounded conformance binary"
    );
    assert!(
        cycle.contains("\"--repo\".to_owned()")
            && cycle.contains("config.repo.display().to_string()"),
        "the conformance command must receive the active repository"
    );
    assert!(
        cycle.contains("\"--check\".to_owned()")
            && cycle.contains("\"ASUPERSYNC-CONFORMANCE.md\".to_owned()"),
        "the check must use the canonical conformance document"
    );
    assert!(
        cycle.contains("no_claim=table_compliance")
            && cycle.contains("AsupersyncConformanceEvidence::Missing")
            && cycle.contains("AsupersyncConformanceEvidence::Stale")
            && cycle.contains("AsupersyncConformanceEvidence::Unavailable"),
        "non-current evidence must remain typed and explicitly no-claim"
    );
    assert!(
        supervisor.contains("run_cycle(cx, &config, tick).await"),
        "run_supervisor must retain the reachable run_cycle trigger"
    );
}
