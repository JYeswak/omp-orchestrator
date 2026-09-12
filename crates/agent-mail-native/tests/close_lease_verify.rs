//! Hermetic close-lease legs: every arm that does not need the live daemon.
//!
//! The daemon-reachable arms live in `lease_close_guard_live.rs` (ignored).
//! These run on every remote build: an unreachable endpoint must be a typed
//! `DaemonError`, never a pass, and transport failure must not present as
//! the catalogued ABSENT.

use agent_mail_native::close_lease::{
    enumerate_exclusive_leases, parse_lease_records, verify_close_lease, CloseLeaseVerdict,
};
use agent_mail_native::journey::{AgentName, ProjectKey};
use agent_mail_native::{Endpoint, MailClient, MailError};
use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use std::future::Future;
use std::time::Duration;

fn run<F>(body: F)
where
    F: Future<Output = ()>,
{
    let runtime = RuntimeBuilder::current_thread()
        .build()
        .expect("runtime must build");
    runtime.block_on(async {
        let _cx = Cx::current().expect("runtime installs a Cx");
        body.await;
    });
}

/// A client pointed at a port that refuses immediately: connection-refused,
/// not a timeout, so this stays fast on every worker.
fn dead_client() -> MailClient {
    MailClient::new(Endpoint::new(
        "http://127.0.0.1:9/mcp/".to_owned(),
        Some("dead-endpoint-probe".to_owned()),
    ))
    .with_request_timeout(Duration::from_secs(5))
}

fn record() -> agent_mail_native::close_lease::LeaseRecord {
    parse_lease_records("LEASE bead=b holder=H paths=a.rs").expect("record")[0].clone()
}

#[test]
fn verify_unreachable_daemon_is_daemon_error_never_released() {
    run(async {
        let cx = Cx::current().expect("cx");
        let verdict = verify_close_lease(
            &cx,
            &dead_client(),
            &ProjectKey::new("/repo"),
            &AgentName::new("GateProbe"),
            &record(),
        )
        .await;
        match verdict {
            CloseLeaseVerdict::DaemonError { detail } => {
                assert!(!detail.is_empty(), "an error with no detail is a shrug");
            }
            other => panic!("unreachable daemon must error, got {other:?}"),
        }
    });
}

#[test]
fn enumerate_unreachable_propagates_instead_of_claiming_absent() {
    run(async {
        let cx = Cx::current().expect("cx");
        let error = enumerate_exclusive_leases(&cx, &dead_client())
            .await
            .expect_err("a dead endpoint cannot enumerate");
        assert!(
            !matches!(error, MailError::NoLeaseEnumeration { .. }),
            "transport failure is not a catalogued ABSENT: {error}"
        );
    });
}
