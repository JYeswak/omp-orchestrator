#![forbid(unsafe_code)]

//! The LIBRARY-level actor guard on `BrPublisher::publish` (ca9q).
//!
//! WHY THIS FILE EXISTS: `operator_surface.rs` already asserts that `finding
//! file` refuses an omitted `--actor`, but that refusal is the CLI's own
//! field check in `main.rs` — it never reaches `BrPublisher::publish`. Measured
//! by mutation: replacing `self.actor.as_ref().ok_or(ActorUnset)?` with a
//! fabricated `"josh"` default left ALL EIGHT operator_surface legs green. So
//! the library guard — the backstop for every caller that is not the CLI, and
//! the one the supervisor route at `resident.rs` depends on — had ZERO
//! coverage. A test name asserting an actor refusal existed; nothing bit.
//!
//! A separate file because `tests/br_publisher.rs` carries another agent's
//! uncommitted work, and because those legs require a real `br` on PATH (they
//! fail `Process(NotFound("br"))` on the remote lane). These legs need no `br`
//! at all: the refusal must happen BEFORE any process is spawned.

use asupersync::Cx;
use asupersync::runtime::RuntimeBuilder;
use finding::{BrPublisher, Finding, FindingError, Publisher};
use std::path::PathBuf;

fn with_cx<T>(body: impl AsyncFnOnce(&Cx) -> T) -> T {
    let runtime = RuntimeBuilder::current_thread()
        .build()
        .expect("asupersync runtime");
    runtime.block_on(async {
        let cx = Cx::current().expect("runtime Cx");
        body(&cx).await
    })
}

fn fixture_finding() -> Finding {
    Finding::new(
        "The library actor guard must refuse",
        "br falls back to an ambient identity, so an unset actor is fabricated provenance",
        "Publish without an actor and expect ActorUnset before any spawn",
        vec!["finding-actor-fixture".to_owned()],
        1,
    )
    .expect("fixture finding")
}

/// KNOWN-BAD FOR THE GUARD ITSELF: no actor set. EXPECT `ActorUnset`.
///
/// The program is a path that CANNOT EXIST, which is what makes this leg
/// load-bearing rather than decorative: if the guard were removed, the call
/// would reach the spawn and fail with `PublishFailed` instead. The two errors
/// are distinguishable, so the leg pins the REFUSAL and its ORDER — before any
/// argv is built and before any process runs.
#[test]
fn publishing_without_an_actor_is_refused_before_any_spawn() {
    with_cx(async |cx| {
        let publisher = BrPublisher::new(
            PathBuf::from("/nonexistent/br-must-never-be-spawned"),
            PathBuf::from("/nonexistent/repo"),
        );
        let error = publisher
            .publish(cx, &fixture_finding())
            .await
            .expect_err("an unattributable finding must not be published");
        assert!(
            matches!(error, FindingError::ActorUnset),
            "expected ActorUnset before the spawn, got: {error}"
        );
        let text = error.to_string();
        assert!(
            text.contains("FINDING_ACTOR_UNSET"),
            "the refusal must name its reason code, got: {text}"
        );
    });
}

/// OVER-STRICTNESS CONTROL: with an actor set the guard lets the call through.
/// Without this leg, a guard that refused unconditionally would also pass the
/// known-bad above. `/bin/echo` returns its own argv as the "bead id", so the
/// actor is observable end to end with no `br` on PATH.
#[test]
fn publishing_with_an_actor_passes_the_guard_and_the_actor_reaches_the_argv() {
    with_cx(async |cx| {
        let publisher = BrPublisher::new(PathBuf::from("/bin/echo"), std::env::temp_dir())
            .with_actor("ca9q-fixture-actor");
        let id = publisher
            .publish(cx, &fixture_finding())
            .await
            .expect("an attributed finding must publish");
        assert!(
            id.contains("--actor") && id.contains("ca9q-fixture-actor"),
            "the actor must reach br's argv, got: {id}"
        );
    });
}
