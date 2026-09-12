//! Live close-lease legs: ignored by default, like the rest of
//! `live_journey.rs`. Run explicitly where the daemon lives:
//!
//! ```text
//! cargo test -p agent-mail-native --test lease_close_guard_live -- --ignored --nocapture
//! ```
//!
//! Everything here fails LOUDLY rather than skipping: an unreachable daemon,
//! a missing credential, and a catalogue that gained a listing tool are all
//! assertion failures, never quiet successes. The first catalogue leg is the
//! tripwire for `NoLeaseEnumeration`: the day it reddens, the daemon grew
//! the tool and `enumerate_exclusive_leases` must call it.
use agent_mail_native::close_lease::{
    catalogue_has_lease_listing, enumerate_exclusive_leases, parse_lease_records,
    verify_close_lease, CloseLeaseVerdict,
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

fn live_client() -> MailClient {
    let endpoint = Endpoint::discover();
    assert!(
        endpoint.token().is_some(),
        "no bearer token discoverable; searched {:?}",
        Endpoint::discovery_sources()
    );
    MailClient::new(endpoint).with_request_timeout(Duration::from_secs(20))
}

fn project() -> ProjectKey {
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .and_then(std::path::Path::parent)
        .expect("manifest dir must sit two levels below the repo root");
    ProjectKey::new(root.to_string_lossy().into_owned())
}

#[test]
#[ignore = "requires the live Agent Mail daemon"]
fn live_catalogue_has_no_lease_listing() {
    run(async {
        let cx = Cx::current().expect("cx");
        let tools = live_client()
            .list_tools(&cx)
            .await
            .expect("live catalogue must list");
        assert!(
            !catalogue_has_lease_listing(&tools),
            "daemon gained a lease-listing tool; enumerate_exclusive_leases must call it"
        );
        println!("live catalogue: {} tools, no listing endpoint", tools.len());
    });
}

#[test]
#[ignore = "requires the live Agent Mail daemon"]
fn live_enumerate_is_typed_absent() {
    run(async {
        let cx = Cx::current().expect("cx");
        let error = enumerate_exclusive_leases(&cx, &live_client())
            .await
            .expect_err("no listing tool exists, so enumeration must refuse");
        match error {
            MailError::NoLeaseEnumeration { catalogue_tools } => {
                assert!(catalogue_tools > 0, "a probed catalogue is never empty");
                println!("live ABSENT over {catalogue_tools} tools");
            }
            other => panic!("expected ABSENT, got {other}"),
        }
    });
}

#[test]
#[ignore = "requires the live Agent Mail daemon"]
fn live_verify_unleased_paths_releases() {
    run(async {
        let cx = Cx::current().expect("cx");
        let record = parse_lease_records(
            "LEASE bead=zz-3w9l-probe holder=zz-3w9l-nobody paths=zz-3w9l-no-such-file.rs",
        )
        .expect("record")[0]
            .clone();
        let verdict = verify_close_lease(
            &cx,
            &live_client(),
            &project(),
            &AgentName::new("NobleBasin"),
            &record,
        )
        .await;
        match verdict {
            CloseLeaseVerdict::Released { checked_paths } => {
                assert_eq!(checked_paths, 1);
            }
            other => panic!("nonsense paths must release, got {other:?}"),
        }
    });
}
