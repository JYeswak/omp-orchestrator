//! CLI wrapper for the ack-spine step ledger.
//!
//! Exit codes: 0 = ledger consistent, 1 = step count assertion failed,
//! 2 = usage error, 3 = anti-vacuity (empty ledger).

#![forbid(unsafe_code)]

use ack_spine::authorities::{AckAuthority, DeliveryAuthority, ReceiverReceipt, TransportAuthority};
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
        let cx = Cx::current().ok_or_else(|| "ACK_SPINE_ERROR reason=no_runtime_context".to_owned())?;
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
        Some(command) => Err(format!("usage error: unknown command {command}")),
        None => Err("usage: ack-spine --demo | --spine-demo | --selftest".to_owned()),
    }
}

async fn demo(cx: &Cx) -> Result<(), String> {
    let mut ledger = StepLedger::new();
    let bead = "cp-example";
    let pane = "%5";
    let session = "omp-orchestrator";

    step(
        cx,
        &mut ledger,
        StepKind::BeadSelected,
        bead,
        pane,
        session,
        "br ready selected",
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
        "ntm robot-send",
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
    println!("SELFTEST PASS ack-spine (ledger assertions, anti-vacuity, cancel-consistency)");
    Ok(())
}

async fn spine_demo(cx: &Cx) -> Result<(), String> {
    let pending_path = std::env::temp_dir().join(format!("ack-spine-demo-{}", std::process::id()));
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
    let receipt = ReceiverReceipt::idle_to_working(
        "%1409",
        1,
        "demo-before",
        "demo-after",
    )
    .ok_or_else(|| "invalid demo receiver receipt".to_owned())?;
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
