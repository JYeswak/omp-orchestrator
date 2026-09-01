#![forbid(unsafe_code)]
//! THE SINGLE-INSTANCE RUN LOCK (bead cp-qyxk), and the two ways it has failed in production.
//!
//! MEASURED 2026-08-24T00:4xZ: 5 fleet-monitor and 6 controller-tick processes running
//! CONCURRENTLY, oldest fleet-monitor 22m22s, load average 80.65. Cron fires at 3/23/43 while one
//! run takes longer than the 20-minute gap, so every tick stacked another process on the last and
//! nothing reaped them. UNBOUNDED — the count grows until something else fails first.
//!
//! PATTERN = `flock(2)` held for the process's life and released BY THE KERNEL on any exit,
//! including SIGKILL. No pid file to go stale, no staleness dance, no reaper. A dead holder's lock
//! is not held — that is a kernel property, not a check we perform, which is why a killed or
//! crashed run can never wedge the lane. The lock FILE persisting is irrelevant; only the
//! fd-associated flock is the lock.
//!
//! ── THE TWO PRODUCTION FAILURES THIS MODULE IS SHAPED BY ──────────────────────────────────
//!
//! 1. THE INHERITED DESCRIPTOR (fixed in shell by 8d4e054, re-prevented here structurally).
//!    A spawned child inheriting the lock fd keeps the flock alive after the parent exits. Today
//!    that held the controller's lock for 2h26m. In Rust the guard owns a `File` whose descriptor
//!    is created with `O_CLOEXEC` — Rust's `std::fs::File` sets close-on-exec by default on unix,
//!    so **no spawned child can inherit it**, and `Drop` releases the flock at scope exit. Both
//!    halves are structural rather than remembered.
//!
//! 2. THE SKIP ROW THAT COULD NOT NAME ITS BLOCKER (requirement D).
//!    Today's skip rows carried `holder_pid=unknown` because holder detection only pgrep'd for
//!    other fleet-monitor processes and could not see a child that inherited the lock. A lane that
//!    cannot name what is blocking it cannot be diagnosed. So the holder lookup asks THE OS WHO
//!    HOLDS THE DESCRIPTOR (`lsof` on the lock file), and falls back to the process scan only when
//!    lsof is unavailable — never the other way round.
//!
//! REFUSAL EXITS 0, NOT 75. A skipped tick is the NORMAL outcome under load; a nonzero exit would
//! turn it into a cron error and a red Monitor event, training the operator to ignore the signal.
//! The skip is a TYPED LEDGER ROW instead.

use fs2::FileExt;
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;

/// What happened when we tried to take the run lock.
#[derive(Debug)]
pub enum LockOutcome {
    /// Held for this process's life. Dropping the guard releases it.
    Acquired(RunLock),
    /// Another live run holds it. Carries whatever the OS could tell us about the holder.
    Busy { holder_pid: String, holder_elapsed: String },
    /// The lock could not be used at all (unopenable path). FAIL CLOSED: refuse to run
    /// unserialized rather than stack another instance.
    Unusable { reason: String },
}

/// An acquired lock. The flock lives exactly as long as this value.
#[derive(Debug)]
pub struct RunLock {
    file: File,
    path: PathBuf,
}

impl RunLock {
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for RunLock {
    fn drop(&mut self) {
        // Explicit release keeps the intent legible; the kernel would also release on close.
        let _ = self.file.unlock();
    }
}

/// How to ask the OS who holds a lock file. Injectable so the refusal paths can be proven
/// hermetically without racing a real second process for a real descriptor.
pub trait HolderLookup {
    /// PIDs currently holding the file open, most relevant first.
    fn holders(&self, lock_path: &Path) -> Vec<String>;
    /// Elapsed run time of a pid, as `ps -o etime=` reports it.
    fn elapsed(&self, pid: &str) -> Option<String>;
}

/// The production lookup: ask the OS who holds the descriptor.
///
/// ⛔ lsof FIRST, process-scan SECOND. The process scan is what produced `holder_pid=unknown`
/// today: it can only see processes running THIS SCRIPT, so a child that inherited the descriptor
/// is invisible to it. lsof answers the question actually being asked — who holds this file open.
#[derive(Debug, Default)]
pub struct OsHolderLookup;

impl HolderLookup for OsHolderLookup {
    fn holders(&self, lock_path: &Path) -> Vec<String> {
        // lsof returns rc=1 with no matches, so OUTPUT is the predicate, never rc==0.
        if let Ok(out) = Command::new("lsof").arg("-t").arg(lock_path).output() {
            let pids: Vec<String> = String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect();
            if !pids.is_empty() {
                return pids;
            }
        }
        // FALLBACK ONLY: other live processes running this lane. Cannot see an inheriting child,
        // which is precisely why it is the fallback and not the primary.
        let me = std::process::id().to_string();
        if let Ok(out) = Command::new("pgrep").arg("-f").arg("fleet-monitor").output() {
            return String::from_utf8_lossy(&out.stdout)
                .lines()
                .map(str::trim)
                .filter(|s| !s.is_empty() && *s != me)
                .map(str::to_string)
                .collect();
        }
        Vec::new()
    }

    fn elapsed(&self, pid: &str) -> Option<String> {
        let out = Command::new("ps").arg("-p").arg(pid).arg("-o").arg("etime=").output().ok()?;
        let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if s.is_empty() {
            None
        } else {
            Some(s)
        }
    }
}

/// Try to take the run lock.
///
/// The returned guard must be held for the life of the run: dropping it releases the flock.
pub fn acquire<L: HolderLookup>(lock_path: &Path, lookup: &L) -> LockOutcome {
    if let Some(parent) = lock_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    // std::fs::File is O_CLOEXEC on unix, so no spawned child inherits this descriptor.
    let file = match OpenOptions::new().create(true).append(true).open(lock_path) {
        Ok(f) => f,
        Err(e) => {
            return LockOutcome::Unusable { reason: format!("open_failed: {e}") };
        }
    };
    match file.try_lock_exclusive() {
        Ok(()) => LockOutcome::Acquired(RunLock { file, path: lock_path.to_path_buf() }),
        Err(_) => {
            // BUSY. Name the blocker — a lane that cannot say what is blocking it cannot be
            // diagnosed, which is exactly today's `holder_pid=unknown` defect.
            let holders = lookup.holders(lock_path);
            let holder_pid = holders.first().cloned().unwrap_or_else(|| "unknown".to_string());
            let holder_elapsed = holders
                .first()
                .and_then(|p| lookup.elapsed(p))
                .unwrap_or_else(|| "unknown".to_string());
            LockOutcome::Busy { holder_pid, holder_elapsed }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("fm-lock-{}-{}", std::process::id(), name));
        std::fs::create_dir_all(&d).unwrap();
        d.join("fleet-monitor.run.lock")
    }

    /// A lookup that knows an answer, so refusal paths prove without racing a real process.
    struct FakeLookup {
        pids: Vec<String>,
        elapsed: Option<String>,
        calls: RefCell<usize>,
    }
    impl HolderLookup for FakeLookup {
        fn holders(&self, _p: &Path) -> Vec<String> {
            *self.calls.borrow_mut() += 1;
            self.pids.clone()
        }
        fn elapsed(&self, _pid: &str) -> Option<String> {
            self.elapsed.clone()
        }
    }

    #[test]
    fn rule_second_instance_is_refused_while_one_is_live() {
        let p = tmp("second");
        let look = FakeLookup { pids: vec!["4242".into()], elapsed: Some("22:22".into()), calls: RefCell::new(0) };
        let first = acquire(&p, &look);
        assert!(
            matches!(first, LockOutcome::Acquired(_)),
            "RULE lock_first_acquires: the first run must take the lock"
        );
        let second = acquire(&p, &look);
        assert!(
            matches!(second, LockOutcome::Busy { .. }),
            "RULE lock_single_instance: a SECOND instance while one is live must be REFUSED — \
             stacking is what put 5 concurrent runs on the box at load 80"
        );
        drop(first);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn rule_skip_row_names_its_holder() {
        let p = tmp("names");
        let look = FakeLookup { pids: vec!["4242".into()], elapsed: Some("22:22".into()), calls: RefCell::new(0) };
        let first = acquire(&p, &look);
        assert!(matches!(first, LockOutcome::Acquired(_)));
        match acquire(&p, &look) {
            LockOutcome::Busy { holder_pid, holder_elapsed } => {
                assert_eq!(
                    holder_pid, "4242",
                    "RULE skip_row_names_holder: a skip row must NAME its blocker — \
                     holder_pid=unknown is the defect that made today's wedge undiagnosable"
                );
                assert_eq!(
                    holder_elapsed, "22:22",
                    "RULE skip_row_elapsed: the holder's elapsed time must be recorded"
                );
            }
            other => panic!("expected Busy, got {other:?}"),
        }
        assert!(
            *look.calls.borrow() >= 1,
            "RULE skip_row_asks_os: the holder lookup must actually be consulted"
        );
        drop(first);
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn rule_lock_released_on_drop_is_reacquirable() {
        // A dead/finished holder's lock is NOT held. This is the kernel property that means a
        // crashed run can never wedge the lane — and the recovery path that must not be deleted.
        let p = tmp("stale");
        let look = FakeLookup { pids: vec![], elapsed: None, calls: RefCell::new(0) };
        {
            let first = acquire(&p, &look);
            assert!(matches!(first, LockOutcome::Acquired(_)));
        } // dropped here — kernel releases
        let again = acquire(&p, &look);
        assert!(
            matches!(again, LockOutcome::Acquired(_)),
            "RULE lock_stale_recovery: a lock whose holder is gone MUST be acquirable — \
             a stale lock file must never wedge the lane"
        );
        let _ = std::fs::remove_file(&p);
    }

    #[test]
    fn rule_unusable_lock_fails_closed() {
        // A path that cannot be opened must refuse, never run unserialized.
        let p = Path::new("/nonexistent-root-xyz/deeper/fleet-monitor.run.lock");
        let look = FakeLookup { pids: vec![], elapsed: None, calls: RefCell::new(0) };
        match acquire(p, &look) {
            LockOutcome::Unusable { .. } => {}
            other => panic!(
                "RULE lock_fail_closed: an unusable lock must refuse to run unserialized, got {other:?}"
            ),
        }
    }

    #[test]
    fn rule_holder_unknown_when_os_cannot_say() {
        let p = tmp("unknown");
        let look = FakeLookup { pids: vec![], elapsed: None, calls: RefCell::new(0) };
        let first = acquire(&p, &look);
        assert!(matches!(first, LockOutcome::Acquired(_)));
        match acquire(&p, &look) {
            LockOutcome::Busy { holder_pid, .. } => assert_eq!(
                holder_pid, "unknown",
                "RULE skip_row_honest: when the OS cannot name the holder the row says so \
                 rather than inventing a pid"
            ),
            other => panic!("expected Busy, got {other:?}"),
        }
        drop(first);
        let _ = std::fs::remove_file(&p);
    }
}
