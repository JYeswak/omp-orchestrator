//! wr2 legs: handroll gate over fixture repos. Every needle below is assembled
//! from parts so this file never contains a raw needle contiguously — the
//! self-leg scans this crate and demands zero.

use kernel_only_gate::{scan_paths, scan_tree, Hit, Verdict};
use std::path::{Path, PathBuf};
use std::process::Command;

// Part assembly: every builder below emits real quotes into the FIXTURE via
// `format!` positional args, while this SOURCE never contains a raw needle
// contiguously (`{0}` sits inside every needle span). `concat!` must NOT be
// used here: it does not substitute, so the quotes would stay literal `{Q}`.
fn send_argv() -> String {
    format!("let argv = [{0}tmux{0}, {0}send-keys{0}, {0}-t{0}];\n", '"')
}

fn spawn_tmux() -> String {
    format!("let mut child = Command::new({0}tmux{0});\n", '"')
}

fn send_shell() -> String {
    format!(
        "let _ = format!({0}tmux send{1}-keys -t %1 hi{0});\n",
        '"', ""
    )
}
fn capture_argv() -> String {
    format!(
        "let argv = [{0}tmux{0}, {0}capture-pane{0}, {0}-p{0}];\n",
        '"'
    )
}

fn create_shell() -> String {
    format!("let _ = format!({0}br{1} create --title hi{0});\n", '"', "")
}

fn run_git(root: &Path, args: &[&str]) {
    let status = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .expect("git runs on the test host")
        .status;
    assert!(status.success(), "git {args:?} must succeed");
}

fn fresh_repo(label: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("kernel-only-gate-{label}-{}", std::process::id()));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).expect("clear stale fixture");
    }
    std::fs::create_dir_all(&dir).expect("fixture root");
    run_git(&dir, &["init", "-q"]);
    run_git(&dir, &["config", "user.email", "fixture@test"]);
    run_git(&dir, &["config", "user.name", "fixture"]);
    dir
}

fn write_tracked(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().expect("parent")).expect("fixture dirs");
    std::fs::write(&path, contents).expect("fixture file");
    run_git(root, &["add", "-A"]);
}

fn kernels(hits: &[Hit]) -> Vec<&str> {
    let mut kernels: Vec<&str> = hits.iter().map(|hit| hit.kernel.as_str()).collect();
    kernels.sort_unstable();
    kernels
}

#[test]
fn known_bad_send_shapes_refused_naming_dispatch() {
    let root = fresh_repo("send");
    let body = "use std::process::Command;\n\nfn main() {\n".to_owned()
        + &spawn_tmux()
        + &send_argv()
        + &send_shell()
        + "}\n";
    write_tracked(&root, "crates/app/src/main.rs", &body);
    let report = scan_tree(&root);
    assert_eq!(report.verdict(), Verdict::Violation);
    // Lines: 1 use, 2 blank, 3 fn, 4 spawn, 5 argv, 6 shell, 7 close.
    let mut lines: Vec<(usize, &str)> = report
        .hits
        .iter()
        .map(|hit| (hit.line, hit.kernel.as_str()))
        .collect();
    lines.sort_unstable();
    assert_eq!(
        lines,
        vec![
            (4, "tick-monitor pane access"),
            (5, "dispatch robot-send"),
            (6, "dispatch robot-send"),
        ],
        "every hit names its replacement kernel at file:line: {lines:?}",
    );
    for hit in &report.hits {
        assert!(
            !hit.kernel.is_empty(),
            "a finding without a kernel is not actionable"
        );
        assert!(
            hit.to_string().contains(&hit.kernel),
            "Display names the kernel"
        );
    }
    std::fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn known_bad_capture_and_create_name_their_kernels() {
    let root = fresh_repo("capture-create");
    let body = "fn main() {\n".to_owned() + &capture_argv() + &create_shell() + "}\n";
    write_tracked(&root, "crates/app/src/main.rs", &body);
    let report = scan_tree(&root);
    assert_eq!(report.verdict(), Verdict::Violation);
    assert_eq!(
        kernels(&report.hits),
        vec!["beads-workflow bead filing", "tick-monitor observe"],
    );
    std::fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn known_good_owners_pass_on_identical_bytes() {
    let root = fresh_repo("owners");
    // Ownership is declared, never inferred from the text: the same shapes
    // pass under their owning crate. Each owner file carries ONLY its owned
    // shapes -- a finding crate with tmux needles still fires, correctly.
    let tick_body = "use std::process::Command;\n\nfn main() {\n".to_owned()
        + &spawn_tmux()
        + &capture_argv()
        + "}\n";
    write_tracked(&root, "crates/tick-monitor/src/probe.rs", &tick_body);
    let tick_report = scan_tree(&root);
    assert!(
        tick_report.hits.is_empty(),
        "owning crate passes: {:?}",
        tick_report.hits
    );
    let finding_body =
        "use std::process::Command;\n\nfn main() {\n".to_owned() + &create_shell() + "}\n";
    write_tracked(&root, "crates/finding/src/probe.rs", &finding_body);
    let both = scan_tree(&root);
    assert!(both.hits.is_empty(), "both owners pass: {:?}", both.hits);
    assert_eq!(both.verdict(), Verdict::Clean);
    // And the negative: tmux shapes under finding still refuse.
    write_tracked(&root, "crates/finding/src/other.rs", &tick_body);
    let cross = scan_tree(&root);
    assert!(cross
        .hits
        .iter()
        .any(|hit| hit.kernel == "tick-monitor observe"));
    assert_eq!(cross.verdict(), Verdict::Violation);
    std::fs::remove_dir_all(root).expect("fixture cleanup");
}
fn comments_do_not_match() {
    let root = fresh_repo("comments");
    // Needles below are assembled from parts so this leg's source never
    // contains one contiguously; the FIXTURE carries them inside comments,
    // which the gate must blank.
    let comment_src = [
        "/",
        "/ tm",
        "ux send-keys -t %1 hi\n",
        "/* tm",
        "ux capture-pane -p\n",
        "br ",
        "create --title hi */\n",
        "fn main() {}\n",
    ]
    .concat();
    write_tracked(&root, "crates/app/src/main.rs", &comment_src);
    let report = scan_tree(&root);
    assert!(
        report.hits.is_empty(),
        "comments must not match: {:?}",
        report.hits
    );
    std::fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn empty_scan_set_is_typed_error() {
    let root = fresh_repo("empty");
    let report = scan_tree(&root);
    assert_eq!(report.verdict(), Verdict::VacuousError);
    assert!(report.scanned.is_empty());
    assert!(report.scope_line().contains("operator shell"));
    std::fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn staged_mode_with_no_eligible_paths_has_no_opinion() {
    let root = fresh_repo("staged");
    write_tracked(&root, "README.md", "no rust here\n");
    let report = scan_paths(&root, &["README.md".to_owned()]);
    assert_eq!(report.verdict(), Verdict::NothingToCheck);
    let empty = scan_paths::<String>(&root, &[]);
    assert_eq!(empty.verdict(), Verdict::NothingToCheck);
    std::fs::remove_dir_all(root).expect("fixture cleanup");
}

#[test]
fn own_crate_is_clean_including_tests() {
    // Self-match would be a finding about this gate's own legs. Needles are
    // part-assembled and assertions name kernels, never raw shapes. Each own
    // file is scored under its real crates/ identity (the walker cannot see
    // it: crate-relative paths carry no crates/ prefix).
    use kernel_only_gate::scan_source;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut scanned = 0;
    for relative in ["src/lib.rs", "tests/handroll_gate.rs"] {
        let text = std::fs::read_to_string(root.join(relative)).expect("own source reads");
        let file = format!("crates/kernel-only-gate/{relative}");
        let hits = scan_source(&file, &text);
        assert!(
            hits.is_empty(),
            "self-leg must be clean on {file}: {hits:?}"
        );
        scanned += 1;
    }
    assert_eq!(scanned, 2, "self-leg must actually scan");
}

#[test]
fn scope_line_states_operator_limit() {
    let root = fresh_repo("scope");
    let report = scan_tree(&root);
    assert!(
        report.scope_line().contains("operator shell"),
        "{}",
        report.scope_line()
    );
    std::fs::remove_dir_all(root).expect("fixture cleanup");
}
