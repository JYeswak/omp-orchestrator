#![forbid(unsafe_code)]

//! Specimen-based legs for the kernel-bypass gate (bead -ilt acceptance 1-3).

use kernel_bypass_gate::{lint_source, strip_line_comment};

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
    assert!(
        hits.iter().any(|b| b.kernel.contains("dispatch") || b.kernel.contains("tick-monitor")),
        "the violation must NAME the kernel: {hits:?}"
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
    assert!(
        hits.iter().any(|b| b.kernel.contains("omp-orchestrator") || b.kernel.contains("beads")),
        "the violation must NAME the kernel: {hits:?}"
    );
}

/// KNOWN-GOOD: the kernel crate itself calling its own interface is NOT a violation.
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
    assert!(hits.is_empty(), "kernel crate's own comments pass: {hits:?}");
}

#[test]
fn strip_preserves_string_content_with_slashes() {
    // A string containing // must NOT be stripped as a comment.
    let line = r#"    let url = "https://example.com"; // this is a comment"#;
    let stripped = strip_line_comment(line);
    assert!(stripped.contains("https://example.com"), "string content preserved");
    assert!(!stripped.contains("this is a comment"), "comment stripped");
}
