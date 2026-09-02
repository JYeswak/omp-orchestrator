//! GROUP-KILL GATE — a deadline must reach grandchildren, not just the direct child.
//!
//! # The measured trap
//!
//! AGENTS.md, verbatim: *"Kill the process GROUP, never the pid. Measured: orphaned
//! grandchildren (`ppid=1`, 0.0% CPU) held the admission lock, so every timeout guaranteed
//! the next attempt also failed — the failure created the condition for its own repetition."*
//!
//! A bare `child.kill()` sends SIGKILL to one pid. Anything that child spawned is reparented
//! to init and keeps running, holding whatever the parent held. The repeat-failure property
//! is what makes it expensive: the orphan outlives the timeout that created it, so the next
//! attempt inherits a poisoned lock and times out too.
//!
//! # Measured 2026-09-01
//!
//! Roughly 30 pid-only kill sites across 15 crates — and several crates signal a GROUP in one
//! function while pid-killing in another, so the inconsistency is intra-crate, not just
//! across the workspace. `loop-tick` was fixed in this same session: its 11 existing tests
//! passed both before AND after the fix, because none of them spawned a grandchild.
//!
//! # RATCHET, not a wall
//!
//! ~30 sites cannot be converted in one pass, and a gate that stays red gets routed around —
//! the measured death of `state-wildcard-lint` at 89% false positives. So the count is a
//! CEILING that may only fall. A NEW pid-only kill fails immediately.
//!
//! # NO-CLAIM
//!
//! This counts a TEXTUAL pattern, not a proof of cancel-correctness. A site that signals the
//! group can still be wrong (no deadline, undrained pipes, a child that was never a group
//! leader), and a `child.kill()` used purely as a belt-and-braces fallback AFTER a group
//! signal is legitimate — this gate cannot tell those apart, which is why it ratchets a count
//! rather than asserting a property.

use std::path::PathBuf;

/// Measured 2026-09-01 BY THIS GATE'S OWN SCAN. Lower it as sites convert; never raise it.
/// Seeded from this scan, not a neighbouring one — the build-identity ratchet was first set
/// from a different measurement and had a slot of slack, so its mutation probe passed.
const PID_KILL_CEILING: usize = 31; // 32 -> 31: loop-tick now routes through the kernel

fn repo_root() -> Option<PathBuf> {
    let mut cur = std::env::current_dir().ok()?;
    loop {
        if cur.join("crates").is_dir() && cur.join("docs/plan").is_dir() {
            return Some(cur);
        }
        if !cur.pop() {
            return None;
        }
    }
}

/// Strip comment lines before matching. The build-identity gate matched its subject's own doc
/// comment ABOUT the pattern it was hunting — a checker whose input contains prose about the
/// thing it checks. `loop-tick` now carries exactly such a comment, so this is not
/// hypothetical.
fn code_only(text: &str) -> String {
    text.lines()
        .filter(|l| {
            let s = l.trim_start();
            !(s.starts_with("//") || s.starts_with("///") || s.starts_with("//!"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn pid_kill_sites(root: &std::path::Path) -> Option<Vec<String>> {
    let out = std::process::Command::new("git")
        .args(["ls-files", "crates"])
        .current_dir(root)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut hits = Vec::new();
    for rel in String::from_utf8_lossy(&out.stdout).lines() {
        if !rel.ends_with(".rs") || rel.contains("/tests/") {
            continue;
        }
        // subprocess-contract is the kernel: its group-kill IS the reference implementation,
        // and its fallback kill is deliberate. Declared, not inferred.
        if rel.contains("subprocess-contract") {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(rel)) else { continue };
        for (i, line) in code_only(&text).lines().enumerate() {
            // `kill_on_drop(true)` is tokio's own teardown, not a pid signal.
            if line.contains(".kill()") && !line.contains("kill_on_drop") {
                hits.push(format!("{rel}:{}", i + 1));
            }
        }
    }
    Some(hits)
}

#[test]
fn the_pid_only_kill_count_only_falls() {
    let Some(root) = repo_root() else {
        eprintln!("SKIP the_pid_only_kill_count_only_falls: no repo root");
        return;
    };
    let Some(hits) = pid_kill_sites(&root) else {
        panic!("git ls-files failed; an unreadable listing must not read as a pass");
    };

    // ANTI-VACUITY: ~30 sites measured. A collapse means the scan broke, not that the
    // workspace stopped killing processes. `citation_integrity` failed exactly this way today,
    // matching 0 of 32 citations because it wanted punctuation the document never used.
    assert!(
        hits.len() >= 10,
        "ANTI-VACUITY: found only {} pid-kill sites; ~30 were measured 2026-09-01. The scan \
         broke, or the comment filter is eating code.",
        hits.len()
    );

    assert!(
        hits.len() <= PID_KILL_CEILING,
        "pid-only kill sites rose to {} against a ceiling of {PID_KILL_CEILING}. A bare \
         child.kill() leaves grandchildren at ppid=1 holding whatever the parent held, and the \
         orphan outlives the timeout that created it - so the NEXT attempt fails too. Signal \
         the GROUP (-pid, TERM then graced KILL) and set process_group(0) at the spawn site; \
         see crates/loop-tick/src/lib.rs for the reference fix. Do NOT raise the ceiling.\n  {}",
        hits.len(),
        hits.iter().take(6).cloned().collect::<Vec<_>>().join("\n  ")
    );
}

/// The reference fix must stay reachable — and it CHANGED SHAPE mid-session, which is why
/// this test is worth reading before copying it.
///
/// It first asserted that `loop-tick` carried its own `-TERM`/`-KILL` on the group. That was
/// right for about an hour. Then `loop-tick` was routed through
/// `subprocess-contract::bounded_status`, its private `wait_deadline` was deleted as a
/// 64-line duplicate, and this assertion went RED — correctly. Having your own group-kill is
/// the SECOND-best outcome; calling the kernel's is the best one, and a gate that demands the
/// second-best blocks the first.
///
/// So it now accepts either: route through the kernel, or signal the group yourself.
#[test]
fn the_reference_fix_signals_the_group_and_leads_it() {
    let Some(root) = repo_root() else { return };
    let text = std::fs::read_to_string(root.join("crates/loop-tick/src/lib.rs"))
        .expect("loop-tick lib.rs must exist");
    let code = code_only(&text);

    let routes_through_kernel = code.contains("subprocess_contract::bounded_status")
        || code.contains("subprocess_contract::bounded_output");
    let signals_group_itself = code.contains("\"-TERM\"") && code.contains("\"-KILL\"");

    assert!(
        routes_through_kernel || signals_group_itself,
        "loop-tick must either route through subprocess-contract (preferred) or signal the \
         process group itself. It currently does neither, so a deadline there kills a pid and \
         leaves grandchildren at ppid=1 holding whatever the parent held."
    );

    // The mutation that proves the kernel route is live: breaking bounded_status's `-pid`
    // signal makes loop-tick's grandchild test fail. That is the check this cannot make
    // statically, and it is recorded here so nobody mistakes this test for that proof.
    assert!(
        !code.contains("fn wait_deadline"),
        "the private wait_deadline duplicate is back. It drifted from the kernel in exactly \
         the way that matters — it killed the pid, not the group — and re-adding it re-opens \
         that drift."
    );
}
