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

#[test]
fn publisher_creates_a_real_bead_in_a_scratch_workspace() {
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
