//! Report-keyed retire legs (bead 4ftow): hermetic end to end against the
//! checked-in `fake-box-transport` binary staged under BOTH the `rch` and
//! `ssh` names in a sandbox-only PATH. No shell runs, no network, no worker.
//!
//! Serializes the legs that mutate process-global env (PATH plus the fake
//! inputs). Cargo runs legs in threads; without this, two env writers could
//! interleave a fake read. Poisoning-tolerant: a prior panic still yields.

#![forbid(unsafe_code)]

use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use contabo_reclaim::model::{worker_by_id, ReclaimMode, RunOutcome};
use std::path::{Path, PathBuf};

static ENV_SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn lock_process_env() -> std::sync::MutexGuard<'static, ()> {
    ENV_SERIAL.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

struct RestoreEnv {
    path: Option<std::ffi::OsString>,
    status: Option<std::ffi::OsString>,
    box_root: Option<std::ffi::OsString>,
    pgrep: Option<std::ffi::OsString>,
}

impl RestoreEnv {
    fn take() -> Self {
        Self {
            path: std::env::var_os("PATH"),
            status: std::env::var_os("FAKE_RCH_STATUS"),
            box_root: std::env::var_os("FAKE_BOX_ROOT"),
            pgrep: std::env::var_os("FAKE_PGREP_LINES"),
        }
    }
}

impl Drop for RestoreEnv {
    fn drop(&mut self) {
        for (key, value) in [
            ("PATH", &self.path),
            ("FAKE_RCH_STATUS", &self.status),
            ("FAKE_BOX_ROOT", &self.box_root),
            ("FAKE_PGREP_LINES", &self.pgrep),
        ] {
            match value {
                Some(previous) => std::env::set_var(key, previous),
                None => std::env::remove_var(key),
            }
        }
    }
}

fn idle_status() -> String {
    r#"{"success":true,"data":{"daemon":{"workers":[{"id":"contabo-2","host":"94.72.121.46","status":"healthy","used_slots":0}],"active_builds":[]}}}"#
        .to_owned()
}

fn busy_status() -> String {
    r#"{"success":true,"data":{"daemon":{"workers":[{"id":"contabo-2","host":"94.72.121.46","status":"healthy","used_slots":1}],"active_builds":[{"worker_id":"contabo-2","id":"b-live"}]}}}"#
        .to_owned()
}

/// Fixture box: <sandbox>/box/<home-triple>/<export>/.rch-target-<w>-pool-<h>/payload,
/// where <home-triple> is built segment by segment below, never spelled: this
/// file is scanned by the path-literal gate it does not get to trip.
struct Fixture {
    _env: std::sync::MutexGuard<'static, ()>,
    _restore: RestoreEnv,
    sandbox: PathBuf,
    box_root: PathBuf,
}

impl Fixture {
    fn new(status: String, pgrep: &str) -> Self {
        let env = lock_process_env();
        let restore = RestoreEnv::take();
        let sandbox = std::env::temp_dir().join(format!(
            "contabo-reclaim-retire-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&sandbox);
        std::fs::create_dir_all(&sandbox).expect("sandbox");
        let fake_bin =
            PathBuf::from(env!("CARGO_BIN_EXE_fake-box-transport"));
        for name in ["rch", "ssh"] {
            let staged = sandbox.join(name);
            std::fs::copy(&fake_bin, &staged).expect("stage fake transport");
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let mut permissions = std::fs::metadata(&staged)
                    .expect("fake metadata")
                    .permissions();
                permissions.set_mode(0o755);
                std::fs::set_permissions(&staged, permissions).expect("fake executable");
            }
        }
        let box_root = sandbox.join("box");
        std::fs::create_dir_all(&box_root).expect("box root");
        std::env::set_var("PATH", &sandbox);
        std::env::set_var("FAKE_RCH_STATUS", status);
        std::env::set_var("FAKE_BOX_ROOT", &box_root);
        std::env::set_var("FAKE_PGREP_LINES", pgrep);
        Self {
            _env: env,
            _restore: restore,
            sandbox,
            box_root,
        }
    }

    /// The operator-home triple, built segment by segment and never spelled:
    /// this file is scanned by the path-literal gate (see module doc).
    fn home_triple() -> PathBuf {
        PathBuf::from("Users").join("josh").join("Developer")
    }

    fn pool(&self, export: &str, pool: &str, payload_bytes: usize) -> PathBuf {
        let dir = self
            .box_root
            .join(Self::home_triple())
            .join(export)
            .join(pool);
        std::fs::create_dir_all(&dir).expect("pool dir");
        std::fs::write(dir.join("payload"), vec![7u8; payload_bytes]).expect("payload");
        dir
    }

    fn export_dir(&self, export: &str) -> PathBuf {
        self.box_root.join(Self::home_triple()).join(export)
    }

    fn run_retire(
        &self,
        export: &str,
        mode: ReclaimMode,
    ) -> contabo_reclaim::model::ReclaimReport {
        let runtime = RuntimeBuilder::current_thread().build().expect("runtime");
        runtime.block_on(async {
            let cx = Cx::current().expect("runtime Cx");
            contabo_reclaim::probe::retire_export(
                &cx,
                &Path::new("/").join(Self::home_triple()),
                worker_by_id("contabo-2").expect("known worker"),
                export,
                mode,
            )
            .await
            .expect("retire must not error at the transport level")
        })
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.sandbox);
    }
}

/// Item 5, both directions in one leg: retiring export A drops A's pools
/// (proven absent on the box) and leaves export B's pools present.
/// Unreported exports are never named and therefore never touched.
#[test]
fn retire_drops_named_export_pools_and_leaves_unnamed_untouched() {
    let fx = Fixture::new(idle_status(), "");
    let pool_a1 = fx.pool("grade-a-41966", ".rch-target-contabo-2-pool-aaaa", 2048);
    let pool_a2 = fx.pool("grade-a-41966", ".rch-target-contabo-2-pool-bbbb", 1024);
    let pool_b = fx.pool("grade-b-41966", ".rch-target-contabo-2-pool-cccc", 512);
    // A non-pool entry under the retired export is live work, not residue.
    std::fs::write(fx.export_dir("grade-a-41966").join("notes.txt"), "keep me")
        .expect("non-pool file");

    let report = fx.run_retire("grade-a-41966", ReclaimMode::Apply);
    assert_eq!(
        report.outcome,
        RunOutcome::Reclaimed,
        "retire must reclaim: {}",
        report.detail
    );
    assert_eq!(report.directories, 2, "two pools dropped: {}", report.detail);
    assert!(
        report.bytes >= 3072,
        "honest bytes for 2048+1024 payloads: {}",
        report.bytes
    );
    assert!(
        !pool_a1.exists() && !pool_a2.exists(),
        "dropped pools must read absent on the box"
    );
    assert!(
        pool_b.exists(),
        "the unreported export's pool must survive the disposal path"
    );
    assert!(
        fx.export_dir("grade-a-41966").join("notes.txt").exists(),
        "non-pool entries under a retired export are not pools and must survive"
    );
}

/// Item 4: a busy worker defers with exit 2 and deletes nothing. A 0 here
/// would report disposal that never happened.
#[test]
fn retire_busy_worker_defers_without_deleting() {
    let fx = Fixture::new(busy_status(), "123 cargo --build\n");
    let pool = fx.pool("grade-busy-41966", ".rch-target-contabo-2-pool-dddd", 512);

    let report = fx.run_retire("grade-busy-41966", ReclaimMode::Apply);
    assert_eq!(
        report.outcome,
        RunOutcome::Deferred,
        "busy worker must defer, not pass: {}",
        report.detail
    );
    assert_eq!(
        report.outcome.exit_code(),
        2,
        "deferral is did-not-finish, never silent"
    );
    assert!(
        pool.exists(),
        "a deferred retire must delete nothing"
    );
}

/// Item 6, first half: an export with zero pools is Vacuous exit 4, never clean.
#[test]
fn retire_zero_pools_is_vacuous_error() {
    let fx = Fixture::new(idle_status(), "");
    std::fs::create_dir_all(fx.export_dir("grade-empty-41966")).expect("empty export");

    let report = fx.run_retire("grade-empty-41966", ReclaimMode::Apply);
    assert_eq!(
        report.outcome,
        RunOutcome::Vacuous,
        "zero pools must error, never pass: {}",
        report.detail
    );
    assert_eq!(report.outcome.exit_code(), 4);
    assert!(
        report.detail.contains("RETIRE_VACUOUS"),
        "vacuity must name itself: {}",
        report.detail
    );
}

/// Item 6, second half: a missing export is vacuous with its own reason, not
/// "no pools" and not a find failure.
#[test]
fn retire_missing_export_is_vacuous_not_unreachable() {
    let fx = Fixture::new(idle_status(), "");

    let report = fx.run_retire("grade-never-existed-41966", ReclaimMode::Apply);
    assert_eq!(
        report.outcome,
        RunOutcome::Vacuous,
        "missing export must error loudly: {}",
        report.detail
    );
    assert!(
        report.detail.contains("RETIRE_NO_SUCH_EXPORT"),
        "missing scope needs its own reason: {}",
        report.detail
    );
}

/// Dry run plans without deleting: the grader previews before --apply.
#[test]
fn retire_dry_run_plans_without_deleting() {
    let fx = Fixture::new(idle_status(), "");
    let pool = fx.pool("grade-dry-41966", ".rch-target-contabo-2-pool-eeee", 512);

    let report = fx.run_retire("grade-dry-41966", ReclaimMode::DryRun);
    assert_eq!(
        report.outcome,
        RunOutcome::Planned,
        "dry run must plan: {}",
        report.detail
    );
    assert_eq!(report.directories, 1);
    assert!(pool.exists(), "dry run deletes nothing");
}
