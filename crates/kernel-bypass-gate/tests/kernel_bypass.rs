#![forbid(unsafe_code)]

//! Specimen-based legs for the kernel-bypass gate (bead -ilt acceptance 1-3).

use std::path::{Path, PathBuf};
use std::process::Command;

use kernel_bypass_gate::{
    debt_verdict, lint_source, lint_workspace,
    SystemicBypassAllowance, BYPASS_DEBT, KERNEL_REGISTRY,
};
use text_structure::code_only;

/// KNOWN-BAD: a raw tmux send-keys outside the kernel crate -> RED naming the kernel.
#[test]
fn known_bad_raw_send_keys_outside_kernel_is_flagged() {
    let source = "\
fn send_work(pane: &str, msg: &str) {
    Command::new(\"tmux\")
        .args([\"send-keys\", \"-t\", pane, \"-l\", msg])
        .output()
        .expect(\"send\");
}
";
    let hits = lint_source("crates/my-crate/src/lib.rs", source);
    assert!(
        !hits.is_empty(),
        "raw tmux send-keys outside tick-monitor must be flagged"
    );
    assert_eq!(
        hits[0].kernel,
        "tick-monitor pane access",
        "the violation must NAME the exact kernel: {hits:?}"
    );
}

/// KNOWN-BAD: a bare br create outside the kernel crate -> RED naming the kernel.
#[test]
fn known_bad_bare_br_create_is_flagged() {
    let source = "\
fn file_gap(context: &str) -> String {
    let out = Command::new(\"br\")
        .args([\"create\", context])
        .output()
        .expect(\"create\");
    String::from_utf8_lossy(&out.stdout).into_owned()
}
";
    let hits = lint_source("crates/my-crate/src/lib.rs", source);
    assert!(
        !hits.is_empty(),
        "bare br create outside the kernel crate must be flagged"
    );
    assert_eq!(
        hits[0].kernel,
        "beads-workflow bead filing",
        "the violation must NAME the exact kernel: {hits:?}"
    );
}

/// KNOWN-GOOD: the kernel crate itself calling its own interface is NOT a violation.

#[test]
fn empty_crates_scan_exits_gate_error_not_success() {
    let root = std::env::temp_dir().join(format!(
        "kernel-bypass-empty-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("empty scan root");
    let output = Command::new(env!("CARGO_BIN_EXE_kernel-bypass-gate"))
        .arg(&root)
        .output()
        .expect("kernel gate binary must run");
    assert_eq!(output.status.code(), Some(3), "empty scan must be a gate error: {output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("empty scan set"),
        "empty scan refusal must be named: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).ok();
}
#[test]
fn kernel_own_call_site_is_allowlisted() {
    let source = "\
fn observe_panes() -> String {
    let out = Command::new(\"tmux\")
        .args([\"capture-pane\", \"-p\", \"-t\", \"%1409\"])
        .output()
        .expect(\"capture\");
    String::from_utf8_lossy(&out.stdout).into_owned()
}
";
    let hits = lint_source("crates/tick-monitor/src/main.rs", source);
    // tick-monitor owns tmux access, so Command::new("tmux") is allowlisted
    let tmux_violations: Vec<_> = hits.iter().filter(|b| b.pattern.contains("tmux")).collect();
    assert!(
        tmux_violations.is_empty(),
        "tick-monitor's own tmux calls must not be flagged: {tmux_violations:?}"
    );
}

/// COMMENT-STRIPPING: the hazard documentation comment must not trigger the lint.
#[test]
fn hazard_documentation_comment_does_not_trigger() {
    let source = "\
// THIS IS THE OLD WAY: raw tmux capture-pane is a kernel bypass. Use tick-monitor observe instead.
fn not_a_violation() {}
";
    let hits = lint_source("crates/my-crate/src/lib.rs", source);
    assert!(
        hits.is_empty(),
        "comment mentioning tmux capture-pane must not trigger: {hits:?}"
    );
}

/// KNOWN-GOOD: a comment naming a kernel pattern inside the KERNEL crate is fine.
#[test]
fn kernel_crate_comments_pass() {
    let source = "\
// Use tick-monitor observe instead of raw tmux capture-pane.
fn documented() {}
";
    let hits = lint_source("crates/tick-monitor/src/main.rs", source);
    assert!(
        hits.is_empty(),
        "kernel crate's own comments pass: {hits:?}"
    );
}

/// ROUTING: `code_only` preserves string literals and strips comments, so the
/// gate's matchers never see prose. This leg pins the routing, not a local helper.
#[test]
fn strip_preserves_string_content_with_slashes() {
    // A string containing // must NOT be stripped as a comment.
    let line = r#"    let url = "https://example.com"; // this is a comment"#;
    let code = code_only(line);
    assert!(
        code.contains("https://example.com"),
        "string content preserved"
    );
    assert!(!code.contains("this is a comment"), "comment stripped");
}

/// KNOWN-GOOD on the SHIPPED path: `lint_workspace` must honour the allowlist.
///
/// This is the leg whose absence was the defect. The specimen legs above all call
/// `lint_source`; CI calls `lint_workspace`, which had a private near-duplicate matcher
/// that discarded the registry's owning-crate column. The tested path allowlisted
/// `tick-monitor`'s own calls and the shipped path did not, so no leg in this file could
/// have caught it — and the dead-code warning naming the unused allowlist resolver was
/// printed in every CI log for twelve runs.
#[test]
fn workspace_scan_honours_the_allowlist_and_still_flags_outsiders() {
    let root = scratch_tree("allowlist");
    write_crate_source(
        &root,
        "tick-monitor",
        "observe.rs",
        "fn observe() { Command::new(\"tmux\").arg(\"list-panes\"); }\n",
    );
    write_crate_source(
        &root,
        "some-consumer",
        "handroll.rs",
        "fn observe() { Command::new(\"tmux\").arg(\"list-panes\"); }\n",
    );

    let report = lint_workspace(&root);

    assert_eq!(
        report.scanned.len(),
        2,
        "positive control: both files must be scanned, else the zero below is vacuous"
    );
    let flagged: Vec<&str> = report
        .violations
        .iter()
        .map(|bypass| bypass.file.as_str())
        .collect();
    assert!(
        flagged.iter().all(|file| !file.contains("tick-monitor")),
        "the owning crate's own call must be allowlisted on the shipped path: {flagged:?}"
    );
    assert_eq!(
        report.violations.len(),
        1,
        "the outsider's handroll must still be flagged: {flagged:?}"
    );
    std::fs::remove_dir_all(&root).ok();
}

/// SELF-IMMUNITY: the gate's own source must produce ZERO bypasses.
///
/// MEASURED 2026-09-02, run 33585450134: seven of eighty-one rows were this crate's own
/// registry. The needles are now assembled with `concat!`, so the value is whole while
/// this file's text never contains one contiguously. This leg is what keeps that true —
/// the first split left THREE survivors hiding in the human-readable kernel-name column,
/// and only a count assertion found them.
#[test]
fn gate_own_source_is_immune_to_its_own_needles() {
    let src = repo_root().join("crates/kernel-bypass-gate/src");
    let mut scanned = 0usize;
    let mut hits = Vec::new();
    for entry in std::fs::read_dir(&src).expect("gate src dir").flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "rs") {
            let text = std::fs::read_to_string(&path).expect("read gate source");
            scanned += 1;
            hits.extend(lint_source(&path.display().to_string(), &text));
        }
    }
    assert!(
        scanned >= 2,
        "positive control: expected lib.rs and main.rs, scanned {scanned}"
    );
    assert!(
        hits.is_empty(),
        "the gate must not flag its own declaration: {hits:?}"
    );
}

/// The `REGISTRY_HOME`-free exemption is safe only while this crate never spawns.
///
/// The gate cannot scan itself for a real handroll (its needles are split), so that
/// coverage moves HERE. A gate crate is a pure text linter; the moment it grows a
/// subprocess this leg refuses, and the exemption's precondition is enforced rather
/// than promised.
#[test]
fn gate_crate_never_spawns_a_subprocess() {
    let src = repo_root().join("crates/kernel-bypass-gate/src");
    for entry in std::fs::read_dir(&src).expect("gate src dir").flatten() {
        let path = entry.path();
        if !path.extension().is_some_and(|ext| ext == "rs") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("read gate source");
        let code = code_only(&text);
        for (index, line) in code.lines().enumerate() {
            assert!(
                !line.contains("process::Command"),
                "{}:{} imports a subprocess; the self-immunity exemption is void",
                path.display(),
                index + 1
            );
        }
    }
}

/// BLOCK COMMENTS: prose inside `/* … */` must not register as a caller.
#[test]
fn block_comment_prose_does_not_trigger() {
    let source = "\
/* The old way was Command::new(\"tmux\") plus a raw
   tmux capture-pane and a br ready pipeline. */
fn documented() {}
";
    let blanked = code_only(source);
    assert_eq!(
        blanked.lines().count(),
        source.lines().count(),
        "blanking must preserve line count so line numbers stay honest"
    );
    let hits = lint_source("crates/my-crate/src/lib.rs", source);
    assert!(hits.is_empty(), "block-comment prose must not fire: {hits:?}");
}

/// A `/*` inside a `//` line comment must not open a block and swallow real code.
#[test]
fn line_comment_cannot_open_a_block_comment() {
    let source = "\
// a slash-star /* lives in this line comment
fn handroll() { Command::new(\"tmux\").arg(\"kill-server\"); }
";
    let hits = lint_source("crates/my-crate/src/lib.rs", source);
    assert_eq!(
        hits.len(),
        1,
        "the real handroll on line 2 must survive blanking: {hits:?}"
    );
    assert_eq!(hits[0].line, 2, "line number preserved through blanking");
}
/// ADOPTED-VERB PROOF (bead -9ub39): the packet instruction the fleet adopted
/// (`--robot-send-receipt=x`) must NOT register as a bypass.
///
/// This leg pins comment-routing, NOT row-absence: its specimen is a `//`
/// comment, blanked before any needle matches, so re-adding a bare
/// `robot-send` row leaves this leg GREEN (proven 2026-09-11 by re-adding it:
/// 15 passed / 2 failed, this leg unmoved). The row-readd tripwire is
/// `real_workspace_ledger_balances`, which reddens UNDECLARED_PATTERN.
#[test]
fn adopted_robot_send_verb_does_not_trigger() {
    let source = "\
// REPLY-VIA: ntm --robot-send=omp-orchestrator --panes=1 --msg-file <path>
fn report() {}
";
    let hits = lint_source("crates/my-crate/src/lib.rs", source);
    assert!(
        hits.is_empty(),
        "the adopted packet verb must not register as a bypass: {hits:?}"
    );
}

/// GENUINE-SPAWN PROOF (bead -9ub39, owner moved by io67x): a real raw `ntm`
/// spawn outside the owning crate must STILL be refused, naming the kernel —
/// and the owning crate's own spawn stays allowlisted.
///
/// io67x moved the declared owner from `tick-monitor` to `ntm-kernel`, the crate
/// that now builds the argv and spawns. `tick-monitor` held ZERO sites for this
/// needle when the row moved, so the allowlist it loses was protecting nothing.
/// This leg pins BOTH halves of the move, so a silent revert to the old owner
/// cannot pass: the new owner is allowlisted AND the former owner is not.
#[test]
fn genuine_ntm_spawn_outside_kernel_still_matches() {
    let source = "\
fn fire() {
    Command::new(\"ntm\").args([\"--robot-send\"]).output().unwrap();
}
";
    let hits = lint_source("crates/my-crate/src/lib.rs", source);
    assert_eq!(hits.len(), 1, "the genuine spawn must still be refused: {hits:?}");
    assert_eq!(
        hits[0].kernel, "ntm-kernel invocation",
        "the refusal must NAME the kernel: {hits:?}"
    );
    let own = lint_source("crates/ntm-kernel/src/lib.rs", source);
    assert!(
        own.is_empty(),
        "the owning crate's own spawn stays allowlisted: {own:?}"
    );
    let former_owner = lint_source("crates/tick-monitor/src/main.rs", source);
    assert_eq!(
        former_owner.len(),
        1,
        "the FORMER owner is no longer allowlisted for ntm: {former_owner:?}"
    );
}

/// The ratchet refuses in BOTH directions, and refuses an undeclared pattern outright.
#[test]
fn ratchet_refuses_new_debt_slack_and_undeclared() {
    let root = scratch_tree("ratchet");
    write_crate_source(
        &root,
        "some-consumer",
        "handroll.rs",
        "fn a() { Command::new(\"tmux\").arg(\"x\"); }\nfn b() { Command::new(\"tmux\").arg(\"y\"); }\n",
    );
    let report = lint_workspace(&root);
    assert_eq!(report.violations.len(), 2, "positive control: two sites");

    let pattern = KERNEL_REGISTRY
        .iter()
        .find(|(_, kernel, _)| *kernel == "tick-monitor pane access")
        .expect("spawn-tmux row")
        .0;
    let row = |ceiling| {
        vec![SystemicBypassAllowance {
            pattern,
            owner: "josh",
            dies_when: "never, this is a fixture",
            ceiling,
        }]
    };

    assert!(
        debt_verdict(&report, &row(2)).is_pass(),
        "an exact ceiling passes"
    );
    let over = debt_verdict(&report, &row(1));
    assert!(!over.is_pass(), "a live count above the ceiling refuses");
    assert!(
        format!("{}", over.faults[0]).starts_with("NEW_BYPASS"),
        "over-count is NEW_BYPASS: {:?}",
        over.faults
    );
    let under = debt_verdict(&report, &row(3));
    assert!(!under.is_pass(), "a ceiling with slack refuses");
    assert!(
        format!("{}", under.faults[0]).starts_with("CEILING_HAS_SLACK"),
        "under-count is CEILING_HAS_SLACK: {:?}",
        under.faults
    );
    let undeclared = debt_verdict(&report, &[]);
    assert!(!undeclared.is_pass(), "no row at all refuses");
    assert!(
        format!("{}", undeclared.faults[0]).starts_with("UNDECLARED_PATTERN"),
        "an absent row is UNDECLARED_PATTERN: {:?}",
        undeclared.faults
    );
    let zeroed = debt_verdict(&report, &row(0));
    assert!(
        format!("{}", zeroed.faults[0]).starts_with("EMPTY_ROW"),
        "a zero ceiling must be deleted, not zeroed: {:?}",
        zeroed.faults
    );
    std::fs::remove_dir_all(&root).ok();
}

/// WIRING / REAL TREE: the shipped ledger must balance against the shipped workspace.
///
/// This is the leg that makes the ledger self-maintaining: `cargo test -p
/// kernel-bypass-gate` and `cargo run -p kernel-bypass-gate -- .` render the SAME verdict
/// from the SAME functions, so the two-leg design cannot drift the way the tested and
/// shipped matchers just did.

#[test]
fn mutation_removing_registry_predicate_is_red_and_restores_green() {
    let root = scratch_tree("registry-mutation");
    write_crate_source(
        &root,
        "consumer",
        "handroll.rs",
        "fn send() { Command::new(\"tmux\").arg(\"x\"); }\n",
    );
    let report = lint_workspace(&root);
    let pattern = KERNEL_REGISTRY
        .iter()
        .find(|(_, kernel, _)| *kernel == "tick-monitor pane access")
        .expect("tmux registry row")
        .0;
    let restored = vec![SystemicBypassAllowance {
        pattern,
        owner: "josh",
        dies_when: "fixture mutation restores this row",
        ceiling: 1,
    }];
    assert!(debt_verdict(&report, &restored).is_pass(), "known-good registry row must pass");
    let mutated = debt_verdict(&report, &[]);
    assert!(!mutated.is_pass(), "removing the registry predicate must go RED");
    assert!(
        format!("{}", mutated.faults[0]).starts_with("UNDECLARED_PATTERN"),
        "mutation must name the missing registry row: {:?}",
        mutated.faults
    );
    assert!(debt_verdict(&report, &restored).is_pass(), "restoring the row must return GREEN");
    std::fs::remove_dir_all(root).ok();
}
#[test]
fn real_workspace_ledger_balances() {
    let report = lint_workspace(&repo_root());
    assert!(
        report.scanned.len() > 100,
        "positive control: expected the whole workspace, scanned {}",
        report.scanned.len()
    );
    let verdict = debt_verdict(&report, BYPASS_DEBT);
    assert!(
        verdict.is_pass(),
        "the ledger no longer describes the tree; each row names its own edit:\n{}",
        verdict
            .faults
            .iter()
            .map(|fault| format!("  {fault}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

/// A per-test tree under the session scratch home, per AGENTS.md's scratch rule.
fn scratch_tree(job: &str) -> PathBuf {
    let base = std::env::var("HOME").expect("HOME");
    let root = PathBuf::from(base)
        .join(".local/state/zeststream/scratch/omp-orchestrator/kernel-bypass-gate")
        .join(format!("{job}-{}", std::process::id()));
    std::fs::create_dir_all(&root).expect("scratch tree");
    root
}

fn write_crate_source(root: &Path, crate_name: &str, file: &str, body: &str) {
    let dir = root.join("crates").join(crate_name).join("src");
    std::fs::create_dir_all(&dir).expect("crate src dir");
    std::fs::write(dir.join(file), body).expect("write source");
}
