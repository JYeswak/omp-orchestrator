//! CLI wrapper for the ack-spine step ledger.
//!
//! Exit codes: 0 = ledger consistent, 1 = step count assertion failed,
//! 2 = usage error, 3 = anti-vacuity (empty ledger).

#![forbid(unsafe_code)]

use ack_spine::authorities::{AckAuthority, DeliveryAuthority, ReceiptVerdict, TransportAuthority};
use ack_spine::spine::{AckSpine, DispatchIntent};
use ack_spine::{step, StepKind, StepLedger};
use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: ack-spine --demo | --spine-demo | --selftest");
        return ExitCode::from(2);
    }

    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("ACK_SPINE_ERROR reason=runtime_build detail={error}");
            return ExitCode::from(2);
        }
    };
    let result = runtime.block_on(async move {
        let cx =
            Cx::current().ok_or_else(|| "ACK_SPINE_ERROR reason=no_runtime_context".to_owned())?;
        run(&cx, &args).await
    });
    match result {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(1)
        }
    }
}

async fn run(cx: &Cx, args: &[String]) -> Result<ExitCode, String> {
    cx.checkpoint()
        .map_err(|_| "ACK_SPINE_ERROR reason=cancelled".to_owned())?;
    match args.first().map(String::as_str) {
        Some("--demo") => {
            demo(cx).await?;
            Ok(ExitCode::SUCCESS)
        }
        Some("--spine-demo") => {
            spine_demo(cx).await?;
            Ok(ExitCode::SUCCESS)
        }
        Some("--selftest") => {
            selftest(cx).await?;
            Ok(ExitCode::SUCCESS)
        }
        // THE WORKER'S EMIT PATH — `ipg.19`. Prints the canonical completion row
        // for `br comments add`, so a worker never hand-formats one.
        //
        // A hand-formatted row is a row that drifts from its parser, and the ACK
        // protocol already paid for that: the pane in every ACK today is the
        // orchestrator's dictation echoed back, because the format lived in packets
        // instead of in code. This prints from `Display`, which is the same
        // implementation `parse_completion` round-trips against in
        // `tests/completion.rs`.
        Some("--complete") => {
            let [bead, pane, verdict, evidence] = match args.get(1..5) {
                Some([bead, pane, verdict, evidence]) => [bead, pane, verdict, evidence],
                _ => {
                    return Err(
                        "usage: ack-spine --complete <bead-id> <pane> <verdict> <evidence> \
                         [frees-pane]"
                            .to_owned(),
                    )
                }
            };
            // Defaults to the emitting pane: a worker normally frees its own, and a
            // required argument that is almost always the same value is an argument
            // people get wrong.
            let frees = args.get(5).map(String::as_str).unwrap_or(pane.as_str());
            let row = ack_spine::completion::completion_row(bead, pane, verdict, evidence, frees);
            // REFUSE TO PRINT A ROW THAT WILL NOT PARSE. Emitting an unparseable
            // completion is the malformed case the classifier escalates, and the
            // emitter is the one place it can be prevented rather than reported.
            let rendered = row.to_string();
            ack_spine::completion::parse_completion(&rendered, bead).map_err(|error| {
                format!("COMPLETION_UNPARSEABLE detail={error:?} row={rendered}")
            })?;
            println!("{rendered}");
            Ok(ExitCode::SUCCESS)
        }
        Some(command) => Err(format!("usage error: unknown command {command}")),
        None => Err(
            "usage: ack-spine --demo | --spine-demo | --selftest | --complete <bead> <pane> \
             <verdict> <evidence> [frees]"
                .to_owned(),
        ),
    }
}

async fn demo(cx: &Cx) -> Result<(), String> {
    let mut ledger = StepLedger::new();
    let bead = "cp-example";
    let pane = "%5";
    let session = "omp-orchestrator";
    let bead_selected = format!(
        "{} {} selected",
        finding::BR,
        loop_queue_filter::READY_SUBCOMMAND
    );
    let packet_sent = format!("{} {}", tick_monitor::NTM, tick_monitor::ntm_send_flag());

    step(
        cx,
        &mut ledger,
        StepKind::BeadSelected,
        bead,
        pane,
        session,
        &bead_selected,
        |_| async {},
    )
    .await
    .map_err(|error| error.to_string())?;
    step(
        cx,
        &mut ledger,
        StepKind::PacketRendered,
        bead,
        pane,
        session,
        "template rendered",
        |_| async {},
    )
    .await
    .map_err(|error| error.to_string())?;
    step(
        cx,
        &mut ledger,
        StepKind::FenceChecked,
        bead,
        pane,
        session,
        "fence admitted",
        |_| async {},
    )
    .await
    .map_err(|error| error.to_string())?;
    step(
        cx,
        &mut ledger,
        StepKind::PacketSent,
        bead,
        pane,
        session,
        &packet_sent,
        |_| async {},
    )
    .await
    .map_err(|error| error.to_string())?;
    step(
        cx,
        &mut ledger,
        StepKind::ReceiverVerified,
        bead,
        pane,
        session,
        "bead id in capture",
        |_| async {},
    )
    .await
    .map_err(|error| error.to_string())?;

    ledger
        .assert_step_count()
        .map_err(|error| error.to_string())?;
    ledger
        .assert_non_empty()
        .map_err(|error| error.to_string())?;

    println!("{}", ledger.to_jsonl());
    println!(
        "# steps_taken={} rows={} consistent={}",
        ledger.steps_taken(),
        ledger.rows().len(),
        ledger.is_consistent()
    );
    Ok(())
}

async fn selftest(cx: &Cx) -> Result<(), String> {
    let mut ledger = StepLedger::new();
    step(
        cx,
        &mut ledger,
        StepKind::BeadSelected,
        "cp-selftest",
        "%5",
        "s",
        "test",
        |_| async {},
    )
    .await
    .map_err(|error| error.to_string())?;
    ledger
        .assert_step_count()
        .map_err(|error| error.to_string())?;
    ledger
        .assert_non_empty()
        .map_err(|error| error.to_string())?;
    // ACCEPTANCE 4: the durable-path invariant is asserted BY `--selftest`, so
    // mutating the resolved path fails the selftest rather than only a unit test.
    // The bead asks for exactly this, and `--selftest` is the surface an operator
    // and the gate both run.
    let pending = durable_pending_path("omp-orchestrator", "ack-spine", "selftest", "josh")?;
    assert_durable_path_is_owned(&pending)?;
    // KNOWN-BAD, ONE SPECIMEN PER GUARD. A single planted path is rejected by BOTH
    // checks, so it cannot prove either one individually bites -- MEASURED: with
    // the temp-prefix guard disabled the selftest stayed GREEN, because the
    // scratch-root check caught the same specimen. That is the same defect as an
    // `any()` over alternatives being unable to detect a substitution among them.
    //
    // Specimen A: under the temp root. Both guards reject it, so the leg asserts the
    // TEMP-SPECIFIC diagnostic. That is what the temp guard is actually for -- the
    // scratch-root check subsumes it for admission, and only the temp guard can tell
    // an operator WHICH defect this is.
    let planted_temp = std::env::temp_dir().join("ack-spine-selftest-known-bad");
    match assert_durable_path_is_owned(&planted_temp) {
        Ok(()) => {
            return Err(format!(
                "SELFTEST FAIL: a temp path was ADMITTED as durable ({})",
                planted_temp.display()
            ))
        }
        Err(refusal) => {
            if !refusal.contains("temp_root=") {
                return Err(format!(
                    "SELFTEST FAIL: a temp path was refused for the WRONG reason -- the \
                     operator cannot tell this is the temp defect: {refusal}"
                ));
            }
        }
    }
    // Specimen B: NOT under temp and NOT under the scratch root. Only the
    // scratch-root requirement can reject this one, so this leg is attributable to
    // that guard alone.
    // Built from the filesystem ROOT, not from a developer's home: a hardcoded
    // "/Users/<name>" literal is what path-literal-guard refuses, and a specimen
    // that only exists on one machine is not a specimen.
    let planted_foreign = std::path::PathBuf::from("/").join("ack-spine-known-bad-foreign");
    match assert_durable_path_is_owned(&planted_foreign) {
        Ok(()) => {
            return Err(format!(
                "SELFTEST FAIL: a path outside the scratch root was ADMITTED as durable ({})",
                planted_foreign.display()
            ))
        }
        Err(refusal) => {
            if !refusal.contains("expected_under=") {
                return Err(format!(
                    "SELFTEST FAIL: a foreign path was refused for the WRONG reason: {refusal}"
                ));
            }
        }
    }
    // ANTI-VACUITY: neither planted path may exist on disk. A check must never be
    // satisfied by having created the very file it forbids.
    for planted in [&planted_temp, &planted_foreign] {
        if planted.exists() {
            return Err(format!(
                "SELFTEST FAIL: a known-bad path exists on disk ({}) -- this check created \
                 the unowned file it exists to prevent",
                planted.display()
            ));
        }
    }
    println!(
        "SELFTEST PASS ack-spine (ledger assertions, anti-vacuity, cancel-consistency, \
         durable-path ownership; marker root {})",
        pending
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default()
    );
    Ok(())
}

/// Where a durable pending-dispatch marker may live. **There is no fallback.**
///
/// # The fallback this replaced, and why announcing it was not enough
///
/// This resolution used to end in
/// `std::env::temp_dir().join(format!("ack-spine-demo-{pid}"))` whenever the
/// scratch root could not be resolved, with a stderr line saying so. The argument
/// in the comment was *"a demo that refuses to run because `$HOME` is unusual is
/// worse than one that runs unattributed and announces it."*
///
/// **That is wrong, and the reason is mechanical rather than stylistic.** A marker
/// under `$TMPDIR` is:
///
/// * **unattributable** — nothing in the path or beside it says which session, pane
///   or agent owns it, so `ScratchRoot::reap` cannot classify it at all;
/// * **unreapable** — the reaper cannot distinguish a live marker from an abandoned
///   one, and `scratch-home`'s own contract makes missing owner metadata `UNKNOWN`
///   and explicitly NOT auto-reapable;
/// * **gone on reboot**, which is the opposite of durable.
///
/// And the announcement does not help: **a stderr line is read by a human, while the
/// file is reaped by a program.** The message tells the wrong audience. This is the
/// same shape as every other loud-but-unread signal measured here — 178 unread
/// ticks, a refusal printed 29 times, a workflow with no runner.
///
/// So the refusal is typed and named, and the caller propagates it. A demo that
/// cannot write an OWNED marker does not write one.
fn durable_pending_path(
    session: &str,
    agent: &str,
    job: &str,
    owner: &str,
) -> Result<std::path::PathBuf, String> {
    let root = scratch_home::ScratchRoot::default().map_err(|error| {
        format!(
            "ACK_SPINE_SCRATCH_UNAVAILABLE reason=root_unresolved detail={error} \
             owner=josh next_action=set-HOME -- refusing to write a durable marker to an \
             unattributed temp path that no reaper can own"
        )
    })?;
    let dir = root
        .create_job(session, agent, job, owner)
        .map_err(|error| {
            format!(
                "ACK_SPINE_SCRATCH_UNAVAILABLE reason=job_uncreatable detail={error} \
                 owner=josh next_action=check-scratch-root-permissions -- refusing an \
                 unowned durable marker"
            )
        })?;
    Ok(dir.join("pending.json"))
}

/// Is this path admissible for a DURABLE marker?
///
/// Separated from resolution so the property is checkable without a filesystem, and
/// so `--selftest` can assert it. **Mutating the resolved path to a temp location
/// makes this return an error**, which is acceptance 4's leg.
fn assert_durable_path_is_owned(path: &std::path::Path) -> Result<(), String> {
    let temp = std::env::temp_dir();
    // Compared by PREFIX rather than by name: `/private/tmp/x` and `$TMPDIR/x` are
    // both temp, and a check keyed on the literal string "tmp" would also reject a
    // legitimate path containing that substring.
    if path.starts_with(&temp) {
        return Err(format!(
            "ACK_SPINE_DURABLE_PATH_UNOWNED path={} temp_root={} -- a durable marker under \
             the temp root has no session owner and cannot be reaped",
            path.display(),
            temp.display()
        ));
    }
    let root = scratch_home::ScratchRoot::default()
        .map_err(|error| format!("ACK_SPINE_SCRATCH_UNAVAILABLE detail={error}"))?;
    if !path.starts_with(root.base()) {
        return Err(format!(
            "ACK_SPINE_DURABLE_PATH_UNOWNED path={} expected_under={} -- only the scratch \
             root carries owner metadata",
            path.display(),
            root.base().display()
        ));
    }
    Ok(())
}

async fn spine_demo(cx: &Cx) -> Result<(), String> {
    // SCRATCH-HOME, NOT temp_dir(), AND NO FALLBACK. A pending-dispatch marker
    // outlives the command that wrote it -- that is its entire purpose -- and
    // AGENTS.md is explicit: "Do not create durable scratch under /private/tmp or
    // $TMPDIR; those locations have no session owner and cannot be safely reaped."
    //
    // This is also the production caller `scratch-home` was missing: wired_lanes
    // reported UNWIRED LANE: scratch-home, and UNWIRED_LANE_ALLOWANCE is empty by
    // design, so the crate had to be genuinely USED rather than exempted.
    let pending_path = durable_pending_path("omp-orchestrator", "ack-spine", "spine-demo", "josh")?;
    // Belt AND braces: the resolver cannot currently return a temp path, and this
    // asserts it anyway, because the next edit to the resolver is the one that
    // reintroduces the defect.
    assert_durable_path_is_owned(&pending_path)?;
    let mut spine = AckSpine::new(
        DispatchIntent::new("cp-spine-demo", "%1409", "omp-orchestrator"),
        pending_path,
    );
    spine.begin(cx).await.map_err(|error| error.to_string())?;
    spine
        .packet_rendered(cx)
        .await
        .map_err(|error| error.to_string())?;
    spine
        .record_transport(
            cx,
            TransportAuthority::Succeeded {
                receipt: "demo-transport-success".to_owned(),
            },
        )
        .await
        .map_err(|error| error.to_string())?;
    let receipt = ReceiptVerdict::ReceiptConfirmed {
        pane_id: "%1409".to_owned(),
        timer_before_secs: None,
        timer_after_secs: 1,
        stable_content_changed: true,
    };
    spine
        .record_delivery(cx, DeliveryAuthority::Observed { receipt })
        .await
        .map_err(|error| error.to_string())?;
    spine
        .record_ack(
            cx,
            AckAuthority::ReadBack {
                bead_id: "cp-spine-demo".to_owned(),
                comment_id: "demo-read-back".to_owned(),
            },
        )
        .await
        .map_err(|error| error.to_string())?;
    let evidence = spine.finish(cx).await.map_err(|error| error.to_string())?;
    println!("{}", spine.ledger().to_jsonl());
    println!(
        "# steps_taken={} rows={} transport={} delivery={} ack={} fully_acknowledged={}",
        spine.ledger().steps_taken(),
        spine.ledger().rows().len(),
        evidence.transport_succeeded(),
        evidence.delivery_observed(),
        evidence.acknowledgement_read_back(),
        evidence.fully_acknowledged()
    );
    Ok(())
}
