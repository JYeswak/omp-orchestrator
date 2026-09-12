#![forbid(unsafe_code)]

use asupersync::Cx;
use asupersync::process::Command;
use asupersync::runtime::RuntimeBuilder;
use finding::{BrPublisher, Finding};
use std::path::{Path, PathBuf};
use subprocess_contract::run_output;

fn scratch_root() -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    for suffix in 0..1_000_u32 {
        let root = std::env::temp_dir().join(format!(
            "finding-br-publisher-{}-{stamp}-{suffix}",
            std::process::id()
        ));
        match std::fs::create_dir(&root) {
            Ok(()) => return root,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => panic!("scratch root {root:?}: {error}"),
        }
    }
    panic!("unable to allocate a unique scratch root");
}

fn with_cx<T>(body: impl AsyncFnOnce(&Cx) -> T) -> T {
    let runtime = RuntimeBuilder::current_thread()
        .build()
        .expect("asupersync runtime");
    runtime.block_on(async {
        let cx = Cx::current().expect("runtime Cx");
        body(&cx).await
    })
}

async fn run_br(cx: &Cx, root: &Path, args: &[&str]) -> asupersync::process::Output {
    let mut command = Command::new("br");
    command.args(args).current_dir(root);
    run_output(cx, command).await.expect("br must run")
}

/// Is `br` an executable file on the given `PATH`?
///
/// No subprocess: spawning to ask whether spawning works is the instrument answering its own
/// question. Mirrors the probe landed for `cross_section_authority`'s `br`-absent conversion.
///
/// The PATH is a PARAMETER, not an ambient read, so both directions are provable on a box that
/// has no `br` at all: `numbers.rs` in this workspace takes the same shape for the same reason.
/// EXECUTABILITY is required, not mere presence -- a non-executable file named `br` cannot answer
/// these legs and must not read as the tool.
fn br_on(path_var: Option<&std::ffi::OsStr>) -> bool {
    let Some(path) = path_var else {
        return false;
    };
    std::env::split_paths(path).any(|dir| {
        let candidate = dir.join(BR_EXECUTABLE);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            std::fs::metadata(&candidate)
                .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
                .unwrap_or(false)
        }
        #[cfg(not(unix))]
        {
            candidate.is_file()
        }
    })
}

fn br_on_path() -> bool {
    br_on(std::env::var_os("PATH").as_deref())
}

/// The tracker CLI these two legs require, spelled once.
const BR_EXECUTABLE: &str = "br";

/// `true` when this box CANNOT host these legs, having said so in gate-runner's own words.
///
/// # Why an UNMEASURABLE print and not a panic
///
/// `br` is an EXTERNAL binary and no workspace member builds it, so on a box without it these two
/// legs asked a question the environment cannot answer and died at
/// `run_output(...).expect("br must run")` -> `Process(NotFound("br"))`. gate-runner already
/// classifies this crate `finding:MISSING_EXECUTABLE` from its own precondition table
/// (`gate-runner/src/main.rs:887`), and its doc at `lib.rs:156` records what the code selects:
/// MISSING_EXECUTABLE means REACH THE TOOL, POLICY_UNAVAILABLE means SUPPLY THE ORACLE, and
/// NEITHER MEANS FIX THE TEST. So the leg reports the same class the oracle does instead of a
/// panic that reads as a product defect.
///
/// ⛔ THIS IS NOT A SKIP OF THE SUBJECT. With `br` PRESENT every assertion below runs unchanged and
/// a real failure still fails: the only thing this guard removes is a red that names the wrong
/// cause. UNMEASURABLE IS NOT PASSING, and the line is printed so a reader cannot mistake the two.
fn br_unavailable(leg: &str) -> bool {
    if br_on_path() {
        return false;
    }
    println!(
        "GATE_RUNNER_UNMEASURABLE names=finding:MISSING_EXECUTABLE executable={BR_EXECUTABLE} \
         leg={leg} detail={BR_EXECUTABLE} is unavailable on PATH -- this leg files a bead through \
         the real tracker CLI, so it cannot be measured here; reach the tool, do not fix the test"
    );
    true
}

#[test]
fn publisher_creates_a_real_bead_in_a_scratch_workspace() {
    if br_unavailable("publisher_creates_a_real_bead_in_a_scratch_workspace") {
        return;
    }
    let root = scratch_root();
    with_cx(async |cx| {
        let initialized = run_br(
            cx,
            &root,
            &[
                "init",
                "--prefix",
                "finding-fixture",
                "--no-daemon",
                "--no-auto-flush",
            ],
        )
        .await;
        assert!(
            initialized.status.success(),
            "br init failed: {initialized:?}"
        );

        let finding = Finding::new(
            "Publisher must create a real bead",
            "A production Publisher cannot be proven by a fake id",
            "Run this integration test and expect the br id shape",
            vec!["finding-fixture".to_owned()],
            1,
        )
        .expect("fixture finding");
        let spool = root.join("spool");
        let publisher = BrPublisher::new("br", &root);
        let filed = finding
            .file(cx, &spool, &publisher)
            .await
            .expect("real br publisher must file");
        let id = filed.id();
        assert!(id.starts_with("finding-fixture-"), "unexpected br id: {id}");
        assert!(
            id.len() > "finding-fixture-".len(),
            "br id must include a unique suffix: {id}"
        );

        let show = run_br(cx, &root, &["show", id, "--json", "--no-daemon"]).await;
        assert!(
            show.status.success(),
            "created bead must be readable: {show:?}"
        );
        let body = String::from_utf8_lossy(&show.stdout);
        assert!(body.contains("Publisher must create a real bead"));
        assert!(body.contains("finding-fixture"));
    });
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn recovery_republishes_a_pending_row_and_retires_it() {
    if br_unavailable("recovery_republishes_a_pending_row_and_retires_it") {
        return;
    }
    let root = scratch_root();
    with_cx(async |cx| {
        let initialized = run_br(
            cx,
            &root,
            &[
                "init",
                "--prefix",
                "finding-recovery",
                "--no-daemon",
                "--no-auto-flush",
            ],
        )
        .await;
        assert!(
            initialized.status.success(),
            "br init failed: {initialized:?}"
        );

        let finding = Finding::new(
            "Recovery must finish deferred findings",
            "A durable pending row is useful only if a scheduled caller drains it",
            "Run recovery and expect one filed id with no pending rows",
            vec!["finding-recovery".to_owned()],
            1,
        )
        .expect("fixture finding");
        let spool = root.join("spool");
        finding.spool(&spool).expect("spool pending finding");
        assert_eq!(finding::pending(&spool).expect("pending sweep").len(), 1);

        let publisher = BrPublisher::new("br", &root);
        let recovered = Finding::recover_pending(cx, &spool, &publisher)
            .await
            .expect("recovery must file the pending row");
        assert_eq!(recovered, 1);
        assert!(finding::pending(&spool).expect("pending sweep").is_empty());
    });
    let _ = std::fs::remove_dir_all(root);
}

/// ⛔ THE LEG THAT PROVES THE CONVERSION IS NOT A SKIP.
///
/// The two legs above decline only when `br` is genuinely unreachable, so the guard must answer
/// PRESENT for a real executable and ABSENT for everything else. Both directions are exercised
/// here because neither this lane nor CI has `br`, so the PRESENT direction is unobtainable from
/// the ambient environment and has to be fixtured -- and an unfixtured guard is a skip nobody has
/// tested.
#[test]
fn the_tool_probe_answers_both_directions_and_requires_executability() {
    let root = scratch_root();

    // ABSENT: an empty PATH, and no PATH at all.
    assert!(!br_on(Some(std::ffi::OsStr::new(""))), "an empty PATH holds no tool");
    assert!(!br_on(None), "an absent PATH cannot be searched");

    // ANTI-VACUITY / the wrong answer pinned: a NON-EXECUTABLE file named `br` is not the tool.
    // Without this, the guard would report PRESENT for any directory containing the name and the
    // legs would run straight into `Process(NotFound)` again.
    let plain = root.join("plain");
    std::fs::create_dir_all(&plain).expect("fixture dir");
    std::fs::write(plain.join(BR_EXECUTABLE), b"not executable\n").expect("fixture file");
    assert!(
        !br_on(Some(plain.as_os_str())),
        "a non-executable file named {BR_EXECUTABLE} must not read as the tool"
    );

    // PRESENT: an executable of that name, so the guard would NOT decline and every assertion in
    // the two legs above would run.
    let usable = root.join("usable");
    std::fs::create_dir_all(&usable).expect("fixture dir");
    let tool = usable.join(BR_EXECUTABLE);
    std::fs::write(&tool, b"#!/bin/sh\nexit 0\n").expect("fixture tool");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mut permissions = std::fs::metadata(&tool).expect("fixture metadata").permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&tool, permissions).expect("fixture mode");
    }
    assert!(
        br_on(Some(usable.as_os_str())),
        "an executable {BR_EXECUTABLE} on the PATH must read as PRESENT, or the conversion is an \
         unconditional skip rather than an environment decline"
    );

    std::fs::remove_dir_all(&root).expect("fixture cleanup");
}
