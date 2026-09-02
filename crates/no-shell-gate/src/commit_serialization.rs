//! Make the pre-commit hook's OUTCOME trustworthy when panes commit concurrently.
//!
//! # The defect
//!
//! `omp-orchestrator-nh5`: the hook silently ate commits on this shared checkout when
//! panes committed at the same time. Measured 2026-09-02 against the hook binary's own
//! source — `crates/no-shell-gate/src/bin/pre-commit-gate.rs`, 674 lines:
//!
//! ```text
//! grep -cE 'flock|FileLock|LockFile|lock\(|O_EXCL|create_new'  ->  0
//! strings .git/hooks/pre-commit | grep -cE 'flock|LOCK'        ->  0
//! ```
//!
//! **There was no serialization of any kind.** Two mechanisms follow from that, and
//! they are different failures with different fixes:
//!
//! 1. **INDEX TOCTOU.** The hook reads the staged set with `git diff --cached` and then
//!    gates it. `git commit` writes the tree AFTERWARDS. Between those two moments any
//!    other pane can `git add` or `git reset` on the SAME `.git/index`, so **the gates
//!    certify one set and the commit records another.** The verdict is not wrong, it is
//!    about a different commit — which is exactly why the loss is silent: every gate
//!    honestly passed, on a set that no longer exists.
//! 2. **CONCURRENT HOOK INVOCATIONS SHARE SINGLE-FILE STATE.** `round_trip_check` reads
//!    `.git/MSG_SRC`, one path for the whole checkout. Measured earlier the same day:
//!    `COMMIT-MSG REFUSED: round-trip: MESSAGE MISMATCH — .git/MSG_SRC (368 bytes)
//!    differs from COMMIT_EDITMSG (5647 bytes)` — another pane's message, from the
//!    previous night, still sitting there. That instance refused LOUDLY. **The inverse
//!    is the silent one:** if the sizes and bytes happen to agree, one pane's message
//!    validates another pane's commit and nothing says so.
//!
//! # Why a refusal and not only a lock
//!
//! A lock serialises **our** hooks. It cannot stop a bare `git add` from another pane
//! between our read and git's write, because that path never runs a hook. So the lock
//! is necessary and not sufficient, and the digest recheck below is what closes the
//! TOCTOU. Acceptance 2 allows either; this does both, because each catches a case the
//! other cannot see.
//!
//! # What this does NOT do
//!
//! It does not make the six gates correct — that is their own business. It makes the
//! hook's **outcome** attributable: after this, a passing verdict is about the set that
//! is actually being committed, or the commit is refused with a reason the author can
//! act on. Silence is removed; correctness is not added.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Lock file name, inside the git directory so it is per-index by construction. Two
/// checkouts of the same repo have different git dirs and must not serialise against
/// each other.
pub const LOCK_FILE_NAME: &str = "omp-pre-commit-gate.lock";

/// After this many seconds a lock is treated as abandoned and stolen.
///
/// A holder that crashes mid-gate would otherwise block every writer forever. The
/// window is deliberately longer than a gate run and short enough that a human does not
/// wait on a dead process: the measured full multi-gate run is seconds, and the slowest
/// observed gate build was 86s.
pub const STALE_LOCK_SECS: u64 = 120;

const _: () = assert!(
    STALE_LOCK_SECS >= 90,
    "a stale window shorter than the slowest observed gate run steals a LIVE holder's lock, which re-opens the race it exists to close"
);

/// The outcome of trying to enter the gate section.
#[derive(Debug)]
pub enum Serialization {
    /// We hold the section. Dropping the guard releases it.
    Acquired(GateGuard),
    /// Another invocation holds it. **Typed and retryable** — the author re-runs
    /// `git commit` rather than reaching for `--no-verify`, which disables every gate
    /// to escape one defect.
    Contended { holder_pid: u32, age_secs: u64 },
    /// The lock could not be evaluated. **Fails closed**: an unusable lock is not an
    /// absent one, and treating it as absent is how a guard becomes decoration.
    Unusable { detail: String },
}

/// Releases the section on drop, including on a panic unwinding out of a gate.
#[derive(Debug)]
pub struct GateGuard {
    path: PathBuf,
}

impl Drop for GateGuard {
    fn drop(&mut self) {
        // A failed release is not worth aborting a commit that already passed: the
        // stale window reclaims it. Deliberately silent, and the ONLY silence in here.
        let _ = fs::remove_file(&self.path);
    }
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Parse `<pid> <unix_secs>` from a lock file body.
///
/// A body we cannot parse is `None`, and the caller must NOT treat that as "no
/// holder" — see [`enter_gate_section`], where it becomes a steal only once the file
/// is also older than the stale window by mtime.
pub fn parse_holder(body: &str) -> Option<(u32, u64)> {
    let mut parts = body.split_whitespace();
    let pid = parts.next()?.parse::<u32>().ok()?;
    let at = parts.next()?.parse::<u64>().ok()?;
    Some((pid, at))
}

/// Try to enter the gate section for `git_dir`.
pub fn enter_gate_section(git_dir: &Path, pid: u32) -> Serialization {
    let path = git_dir.join(LOCK_FILE_NAME);
    match try_create(&path, pid) {
        Ok(guard) => return Serialization::Acquired(guard),
        Err(error) if error.kind() != std::io::ErrorKind::AlreadyExists => {
            return Serialization::Unusable {
                detail: format!("cannot create {}: {error}", path.display()),
            };
        }
        Err(_) => {}
    }
    // The file exists. Decide holder-alive vs abandoned from its CONTENTS, falling
    // back to mtime when the body is unreadable or malformed — a torn write by a
    // holder that died mid-`write_all` must not pin the section forever.
    let body = fs::read_to_string(&path).unwrap_or_default();
    let (holder_pid, age_secs) = match parse_holder(&body) {
        Some((holder_pid, at)) => (holder_pid, now_unix().saturating_sub(at)),
        None => {
            let age = fs::metadata(&path)
                .and_then(|m| m.modified())
                .ok()
                .and_then(|t| t.elapsed().ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            (0, age)
        }
    };
    if age_secs <= STALE_LOCK_SECS {
        return Serialization::Contended {
            holder_pid,
            age_secs,
        };
    }
    // Abandoned: steal it. If the remove races another stealer, the create below
    // fails and we report contention rather than double-entering.
    if let Err(error) = fs::remove_file(&path) {
        return Serialization::Unusable {
            detail: format!("stale lock {} unremovable: {error}", path.display()),
        };
    }
    match try_create(&path, pid) {
        Ok(guard) => Serialization::Acquired(guard),
        Err(_) => Serialization::Contended {
            holder_pid,
            age_secs,
        },
    }
}

fn try_create(path: &Path, pid: u32) -> std::io::Result<GateGuard> {
    // `create_new` is O_EXCL: the check and the create are one syscall, so two
    // processes cannot both believe they created it. A `path.exists()` test followed
    // by a create would be the same TOCTOU this module exists to close.
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    write!(file, "{pid} {}", now_unix())?;
    file.sync_data()?;
    Ok(GateGuard {
        path: path.to_path_buf(),
    })
}

/// An order-independent digest of the staged set, used to detect index drift across
/// the gate section.
///
/// Order-independent on purpose: `git diff --cached` output order is not a contract,
/// and a digest that changed when only the order changed would refuse honest commits —
/// the over-strictness that gets a commit-path gate routed around.
pub fn staged_digest(paths: &[String]) -> u64 {
    let mut acc: u64 = 0xcbf2_9ce4_8422_2325;
    for path in paths {
        // Per-path FNV, then a commutative fold, so the total does not depend on order.
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in path.as_bytes() {
            h ^= u64::from(*byte);
            h = h.wrapping_mul(0x100_0000_01b3);
        }
        acc = acc.wrapping_add(h ^ (h >> 29));
    }
    acc
}

/// Whether the staged set moved while the gates ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDrift {
    Unchanged,
    /// Another pane staged or unstaged something mid-flight. **The gates' verdict is
    /// about a set that no longer exists**, which is the silent loss itself.
    Drifted { before: u64, after: u64 },
}

pub fn classify_drift(before: u64, after: u64) -> IndexDrift {
    if before == after {
        IndexDrift::Unchanged
    } else {
        IndexDrift::Drifted { before, after }
    }
}

/// The refusal text. One function so the wording cannot drift between the two call
/// sites, and so a test can assert on the emitted TEXT per gate rule 7.
pub fn retry_refusal(reason: &str, detail: &str) -> String {
    format!(
        "RETRY_CONCURRENT_COMMIT reason={reason} {detail} next_action=re-run-git-commit -- \
         another pane held the gate section or moved the index mid-flight; the gates' \
         verdict would have described a different staged set. Re-run; do NOT reach for \
         --no-verify, which disables every gate to escape one race."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    fn fixture_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "nh5-{tag}-{}-{}",
            std::process::id(),
            now_unix()
        ));
        fs::create_dir_all(&dir).expect("fixture dir");
        dir
    }

    /// THE FAILING-FIRST OBSERVATION, acceptance 1, as a deterministic assertion:
    /// two invocations against ONE git dir, and exactly one may enter.
    ///
    /// Before this module the answer was TWO — the hook had no lock at all, so both
    /// panes gated the shared index concurrently and one verdict described a staged set
    /// the other had already changed.
    #[test]
    fn only_one_invocation_can_enter_the_gate_section() {
        let dir = fixture_dir("single");
        let first = enter_gate_section(&dir, 1001);
        assert!(matches!(first, Serialization::Acquired(_)), "{first:?}");
        let second = enter_gate_section(&dir, 1002);
        match second {
            Serialization::Contended {
                holder_pid,
                age_secs,
            } => {
                assert_eq!(holder_pid, 1001, "the refusal must NAME the holder");
                assert!(age_secs <= STALE_LOCK_SECS);
            }
            other => panic!("the second entry must be refused, got {other:?}"),
        }
        drop(first);
        // Released: a retry now succeeds, which is what makes the refusal actionable
        // rather than terminal.
        assert!(matches!(
            enter_gate_section(&dir, 1003),
            Serialization::Acquired(_)
        ));
        fs::remove_dir_all(&dir).ok();
    }

    /// REAL CONCURRENCY, not a simulation: eight threads race for one section and
    /// exactly one wins. A lock that passes the sequential test above and fails here
    /// is the whole defect class.
    #[test]
    fn eight_racing_threads_yield_exactly_one_holder() {
        let dir = Arc::new(fixture_dir("race"));
        let acquired = Arc::new(AtomicUsize::new(0));
        let refused = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();
        for pid in 0..8u32 {
            let dir = Arc::clone(&dir);
            let acquired = Arc::clone(&acquired);
            let refused = Arc::clone(&refused);
            handles.push(std::thread::spawn(move || {
                match enter_gate_section(&dir, 9000 + pid) {
                    Serialization::Acquired(guard) => {
                        acquired.fetch_add(1, Ordering::SeqCst);
                        // Hold it long enough that every sibling must observe it.
                        std::thread::sleep(std::time::Duration::from_millis(120));
                        drop(guard);
                    }
                    Serialization::Contended { .. } => {
                        refused.fetch_add(1, Ordering::SeqCst);
                    }
                    Serialization::Unusable { detail } => panic!("unusable: {detail}"),
                }
            }));
        }
        for handle in handles {
            handle.join().expect("thread");
        }
        assert_eq!(
            acquired.load(Ordering::SeqCst),
            1,
            "exactly one thread may hold the section"
        );
        assert_eq!(refused.load(Ordering::SeqCst), 7);
        fs::remove_dir_all(dir.as_path()).ok();
    }

    /// A holder that died mid-gate must not pin the section forever, and a lock whose
    /// body was torn by that death must not either.
    #[test]
    fn an_abandoned_lock_is_stolen_and_a_torn_body_does_not_pin_the_section() {
        let dir = fixture_dir("stale");
        let path = dir.join(LOCK_FILE_NAME);
        // Stamped well past the stale window.
        fs::write(&path, format!("4242 {}", now_unix() - STALE_LOCK_SECS - 10)).expect("write");
        assert!(matches!(
            enter_gate_section(&dir, 7),
            Serialization::Acquired(_)
        ));
        fs::remove_file(&path).ok();

        // A body that parses to nothing: the age must come from mtime, and a FRESH
        // torn file must still be treated as a live holder.
        fs::write(&path, "").expect("write empty");
        assert!(
            matches!(enter_gate_section(&dir, 8), Serialization::Contended { .. }),
            "a freshly torn lock is a live holder, not an absent one"
        );
        assert_eq!(parse_holder(""), None);
        assert_eq!(parse_holder("not-a-pid 123"), None);
        assert_eq!(parse_holder("77 1767331200"), Some((77, 1_767_331_200)));
        fs::remove_dir_all(&dir).ok();
    }

    /// FAIL CLOSED: a lock path that cannot be created is `Unusable`, never `Acquired`.
    /// An unusable guard read as an absent one is a guard that is decoration.
    #[test]
    fn an_uncreatable_lock_path_fails_closed() {
        let missing = Path::new("/nonexistent-nh5-dir-that-cannot-be-created");
        match enter_gate_section(missing, 1) {
            Serialization::Unusable { detail } => {
                assert!(detail.contains(LOCK_FILE_NAME), "{detail}");
            }
            other => panic!("must fail closed, got {other:?}"),
        }
    }

    /// THE TOCTOU LEG. The lock serialises our hooks; it cannot stop a bare `git add`
    /// from another pane, because that path runs no hook. The digest recheck is what
    /// catches it.
    #[test]
    fn a_staged_set_that_moves_mid_gate_is_detected() {
        let before = staged_digest(&["crates/a/src/lib.rs".to_owned()]);
        let after = staged_digest(&[
            "crates/a/src/lib.rs".to_owned(),
            "crates/b/src/lib.rs".to_owned(),
        ]);
        assert_eq!(classify_drift(before, before), IndexDrift::Unchanged);
        assert_eq!(
            classify_drift(before, after),
            IndexDrift::Drifted { before, after }
        );
        // An UNSTAGE is drift too: the gates certified a file the commit will not carry.
        let unstaged = staged_digest(&[]);
        assert_ne!(before, unstaged);
    }

    /// The digest must be order-INDEPENDENT: `git diff --cached` output order is not a
    /// contract, and refusing on a reorder is the over-strictness that gets a
    /// commit-path gate routed around.
    #[test]
    fn the_digest_ignores_order_but_not_content() {
        let a = staged_digest(&["x".to_owned(), "y".to_owned(), "z".to_owned()]);
        let b = staged_digest(&["z".to_owned(), "x".to_owned(), "y".to_owned()]);
        assert_eq!(a, b, "a reorder is not drift");
        assert_ne!(a, staged_digest(&["x".to_owned(), "y".to_owned()]));
        assert_ne!(a, staged_digest(&["x".to_owned(), "y".to_owned(), "Z".to_owned()]));
        // ANTI-VACUITY: a digest that returned a constant would pass every equality
        // above. Two different sets must differ.
        assert_ne!(staged_digest(&[]), a);
    }

    /// Gate rule 7: assert on the emitted TEXT. The refusal must name the race AND the
    /// retry, because a refusal an author cannot act on is what put `--no-verify` into
    /// circulation in the first place.
    #[test]
    fn the_refusal_names_the_race_and_the_retry() {
        let text = retry_refusal("GATE_SECTION_HELD", "holder_pid=4242 age_secs=3");
        assert!(text.starts_with("RETRY_CONCURRENT_COMMIT"), "{text}");
        assert!(text.contains("reason=GATE_SECTION_HELD"), "{text}");
        assert!(text.contains("holder_pid=4242"), "{text}");
        assert!(text.contains("next_action=re-run-git-commit"), "{text}");
        assert!(text.contains("--no-verify"), "{text}");
        // One line: the hook's output is read in a terminal, and a multi-line refusal
        // scrolls its own reason away.
        assert_eq!(text.lines().count(), 1, "{text}");
    }
}
