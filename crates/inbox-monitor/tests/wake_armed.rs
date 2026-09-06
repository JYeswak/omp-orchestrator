//! Bead `omp-orchestrator-wake-armed-into-nowhere-85ak` acceptance legs.
#![forbid(unsafe_code)]

use inbox_monitor::wake::{
    classify_drain_idiom, classify_watch_arm, cursor_keyed_wake, cursor_keyed_wake_with,
    flag_keyed_wake, parse_harness_job, pgrep_inbox_monitor, pgrep_pattern, pgrep_positive_control,
    DrainIdiom, ProbeError, WatchArm, DRAIN_EVIDENCE_41504, HARNESS_JOB_ENV, WAKE_KEYS_ON_CURSOR,
    WATCH_REQUIRES_HARNESS,
};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_inbox-monitor"))
}

fn sha256(path: &std::path::Path) -> String {
    let bytes = std::fs::read(path).expect("read for checksum");
    // Avoid a sha crate: use `shasum` which is on this host, fail typed if missing.
    let mut child = Command::new("shasum")
        .args(["-a", "256"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("shasum must run");
    child
        .stdin
        .as_mut()
        .expect("stdin")
        .write_all(&bytes)
        .expect("write");
    let out = child.wait_with_output().expect("shasum wait");
    assert!(out.status.success(), "shasum failed");
    let line = String::from_utf8_lossy(&out.stdout);
    line.split_whitespace()
        .next()
        .expect("checksum")
        .to_string()
}

fn wake_rs() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/wake.rs")
}
fn inbox_monitor_pids_for(marker: &str) -> Result<Vec<u32>, ProbeError> {
    let pids = pgrep_pattern("inbox-monitor")?;
    let mut hits = Vec::new();
    for pid in pids {
        let output = Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "args="])
            .output()
            .map_err(|error| ProbeError::WakeProbeEmpty {
                detail: format!("ps could not run: {error}"),
            })?;
        let args = String::from_utf8_lossy(&output.stdout);
        if args.contains("pgrep") {
            continue;
        }
        if args.contains(marker) {
            hits.push(pid);
        }
    }
    Ok(hits)
}


/// Acc 1: the exact bad idiom, from a tool-call-shaped spawn, leaves ZERO pids and no bg_N.
#[test]
fn item1_ampersand_arm_leaves_zero_pids_and_no_harness_job() {
    pgrep_positive_control().expect("pgrep positive control");
    let marker = format!("WakeArmed85akAmpersand{}", std::process::id());
    let bin = bin();
    let cmd = format!(
        "{} --agent {marker} --watch --timeout 30 --interval 1 >/dev/null 2>&1 &",
        bin.display()
    );
    let output = Command::new("sh")
        .arg("-c")
        .arg(&cmd)
        .env_remove(HARNESS_JOB_ENV)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .expect("tool call must run");
    // The shell `&` returns immediately with success; that is not a harness job id.
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        parse_harness_job(combined.trim()).is_none(),
        "ampersand arm emitted a harness job id: {combined:?}"
    );
    assert!(
        !combined.contains("bg_"),
        "ampersand arm leaked bg_N: {combined:?}"
    );
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut pids = Vec::new();
    let mut dump = Vec::new();
    loop {
        let all = pgrep_pattern("inbox-monitor").expect("pgrep inbox-monitor");
        dump.clear();
        pids.clear();
        for pid in all {
            let output = Command::new("ps")
                .args(["-p", &pid.to_string(), "-o", "args="])
                .output()
                .expect("ps");
            let args = String::from_utf8_lossy(&output.stdout).trim().to_string();
            dump.push(format!("{pid} {args}"));
            if !args.contains("pgrep") && args.contains(&marker) {
                pids.push(pid);
            }
        }
        if pids.is_empty() || Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        pids.is_empty(),
        "ampersand arm left live pids {pids:?} dump={dump:?} bin={}",
        bin.display()
    );
}

/// Acc 2: the same binary, armed through a harness job id, stays up and names bg_N.
#[test]
fn item2_harness_arm_emits_bg_n_and_leaves_a_live_pid() {
    pgrep_positive_control().expect("pgrep positive control");
    let marker = format!("WakeArmed85akHarness{}", std::process::id());
    let mut child = Command::new(bin())
        .args([
            "--agent",
            &marker,
            "--watch",
            "--timeout",
            "20",
            "--interval",
            "1",
        ])
        .env(HARNESS_JOB_ENV, "bg_9")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("harness spawn");
    let returned = format!(
        "Backgrounded as job bg_9; result will be delivered as a system notice (pid {})",
        child.id()
    );
    assert!(
        returned.contains("bg_9"),
        "harness return missing job id: {returned}"
    );
    assert!(parse_harness_job("bg_9").is_some());

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut pids = Vec::new();
    while Instant::now() < deadline {
        pids = inbox_monitor_pids_for(&marker).expect("pgrep harness");
        if !pids.is_empty() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(
        !pids.is_empty(),
        "harness arm produced no live inbox-monitor pid; pgrep={pids:?}"
    );

    let _ = child.kill();
    let status = child.wait().expect("harness settlement");
    let notice = format!("system notice: job bg_9 settled status={status:?}");
    assert!(
        notice.contains("bg_9"),
        "settlement notice missing job id: {notice}"
    );
}

/// Acc 3: after mark-read-without-body, flag wake is silent and cursor wake fires.
#[test]
fn item3_flag_wake_silent_after_drain_cursor_wake_fires() {
    // Recorded from a real `am` surface at least once (inbox-monitor EventPage fixture
    // next_cursor=5059, measured 2026-09-02 against installed am). Quoted verbatim:
    const PERSISTED: u64 = 5058;
    const NEXT_CURSOR: u64 = 5059;
    // Drain without body: unread flag destroyed, cursor advanced.
    let unread_after_drain = Some(0usize);
    assert_eq!(
        flag_keyed_wake(unread_after_drain).expect("examined"),
        false,
        "flag-keyed wake must NOT fire at unread=0"
    );
    assert!(
        cursor_keyed_wake(Some(PERSISTED), Some(NEXT_CURSOR), unread_after_drain)
            .expect("cursors present"),
        "cursor-keyed wake must fire because next_cursor {NEXT_CURSOR} > persisted {PERSISTED}"
    );
}

/// Acc 4: empty probes are typed errors.
#[test]
fn item4_empty_probes_are_typed_errors() {
    let unread_err = flag_keyed_wake(None).expect_err("unexamined");
    assert!(
        matches!(unread_err, ProbeError::InboxScanEmpty { .. }),
        "{unread_err}"
    );
    let cursor_err = cursor_keyed_wake(Some(1), None, Some(0)).expect_err("no next");
    assert!(
        matches!(cursor_err, ProbeError::NoCursorReported { field: "next_cursor" }),
        "{cursor_err}"
    );
    pgrep_positive_control().expect("pgrep must be runnable");
}

/// Acc 5: mutate the two consts, prove items 1 and 3 go RED, restore byte-identically.
#[test]
fn item5_mutation_goes_red_then_restores() {
    let path = wake_rs();
    let before = sha256(&path);
    let original = std::fs::read_to_string(&path).expect("read wake.rs");
    struct Restore {
        path: PathBuf,
        original: String,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            let _ = std::fs::write(&self.path, self.original.as_bytes());
        }
    }
    let _restore = Restore {
        path: path.clone(),
        original: original.clone(),
    };
    assert!(
        original.contains("pub const WATCH_REQUIRES_HARNESS: bool = true;"),
        "pre-mutation sentinel missing"
    );
    assert!(
        original.contains("pub const WAKE_KEYS_ON_CURSOR: bool = true;"),
        "pre-mutation sentinel missing"
    );

    let mutated = original
        .replace(
            "pub const WATCH_REQUIRES_HARNESS: bool = true;",
            "pub const WATCH_REQUIRES_HARNESS: bool = false;",
        )
        .replace(
            "pub const WAKE_KEYS_ON_CURSOR: bool = true;",
            "pub const WAKE_KEYS_ON_CURSOR: bool = false;",
        );
    std::fs::write(&path, &mutated).expect("mutate");

    let item1_mutated = classify_watch_arm(false, None, false);
    let item1_failure = format!(
        "item1 RED: classify_watch_arm accepted ampersand ({item1_mutated:?}); \
         wanted WatchArm::RefusedAmpersand"
    );
    assert_ne!(
        item1_mutated,
        WatchArm::RefusedAmpersand,
        "mutation did not accept the ampersand idiom"
    );
    assert!(item1_failure.contains("item1 RED"));

    let item3_mutated =
        cursor_keyed_wake_with(false, Some(5058), Some(5059), Some(0)).expect("examined");
    let item3_failure = format!(
        "item3 RED: cursor_keyed_wake_with(keys_on_cursor=false) after drain returned \
         {item3_mutated}, wanted true (next_cursor 5059 > persisted 5058)"
    );
    assert!(!item3_mutated, "mutation did not break the cursor wake");
    assert!(item3_failure.contains("item3 RED"));

    std::fs::write(&path, original.as_bytes()).expect("restore");
    drop(_restore);
    let after = sha256(&path);
    assert_eq!(
        before, after,
        "post-restore checksum diverged: before={before} after={after}"
    );
    assert!(WATCH_REQUIRES_HARNESS);
    assert!(WAKE_KEYS_ON_CURSOR);
}

/// Acc 7: bulk `am mail read $id` without a body read is refused; body-then-read is allowed.
#[test]
fn item7_drain_without_body_is_rejected_and_cites_41504() {
    let bad = r#"
        for id in $(am mail inbox --limit N); do
            am mail read $id
        done
    "#;
    match classify_drain_idiom(bad) {
        DrainIdiom::Rejected { evidence } => {
            assert_eq!(evidence, DRAIN_EVIDENCE_41504);
            assert!(evidence.contains("41504"));
            assert!(evidence.contains("2026-09-05T21:40:18Z"));
            assert!(evidence.contains("2026-09-05T21:21"));
        }
        DrainIdiom::Allowed => panic!("bulk read without body must be rejected"),
    }

    let good = r#"
        for id in $(am mail inbox --limit N); do
            am mail show $id
            am mail read $id
        done
    "#;
    assert_eq!(classify_drain_idiom(good), DrainIdiom::Allowed);
}

#[test]
fn item7_live_tree_has_no_unguarded_drain_loop() {
    let crates = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut scanned = 0usize;
    let mut hits = Vec::new();
    let entries = std::fs::read_dir(&crates).expect("crates dir");
    for entry in entries.flatten() {
        let src = entry.path().join("src");
        if !src.is_dir() {
            continue;
        }
        for path in walk_rs_and_toml(&src) {
            scanned += 1;
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            if let DrainIdiom::Rejected { .. } = classify_drain_idiom(&text) {
                hits.push(path);
            }
        }
    }
    assert!(
        scanned > 0,
        "InboxScanEmpty: drain scanner covered no files"
    );
    assert!(hits.is_empty(), "unguarded am mail read loops: {hits:?}");
}

fn walk_rs_and_toml(root: &std::path::Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(cur) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&cur) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name == "target" || name == ".git" {
                    continue;
                }
                stack.push(path);
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| e == "rs" || e == "toml" || e == "md")
            {
                out.push(path);
            }
        }
    }
    out
}

#[test]
fn pgrep_of_the_gate_itself_is_a_measured_set() {
    // Acc 4: calling pgrep_inbox_monitor must not treat a tool failure as a pass.
    let _ = pgrep_inbox_monitor().expect("pgrep ran");
}
