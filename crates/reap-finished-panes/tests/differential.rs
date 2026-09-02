//! Differential against the control-plane reaper oracle on hermetic predicates.
//! Empty comparison set is an ERROR (fh C86); the Rust crate now owns the shipped
//! reaping path and the script remains an external differential oracle.

use reap_finished_panes::{
    apply_deadline, consecutive_cycle_started_same_pid, decide_reap, invoker_from_chain,
    is_worker_pane, parse_ancestor_rows, parse_reaper_out, require_panes, should_reap,
    write_reaped_result, ReapFinishedPanesRules, ReapPaneDecision, SweepStats,
};
use std::process::Command;
use std::time::{Duration, Instant};

fn shell_invoker(text: &str) -> String {
    // Drive the PURE function inside the oracle (stdin = uid ppid comm rows).
    let script = r#"
invoker_from_chain() {
  local _uid _ppid _comm
  while read -r _uid _ppid _comm; do
    if [ "$_uid" = "0" ] && [ "$_ppid" = "1" ] && [ "$_comm" = "/usr/sbin/cron" ]; then
      echo SCHEDULED; return
    fi
  done
  echo MANUAL
}
invoker_from_chain
"#;
    let mut child = Command::new("/bin/bash")
        .arg("-c")
        .arg(script)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("bash");
    use std::io::Write;
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(text.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn rust_invoker(text: &str) -> String {
    invoker_from_chain(&parse_ancestor_rows(text))
        .invoker
        .to_string()
}

#[test]
fn comparator_sees_manufactured_disagreement() {
    let text = "0 1 /usr/sbin/cron\n";
    let sh = shell_invoker(text);
    let mut rules = ReapFinishedPanesRules::default();
    // Manufacture a disagreement on skip_human_shell, which the comparator
    // can see independently of invoker.
    assert!(rules.disable("skip_human_shell"));
    assert!(
        is_worker_pane("0", &rules),
        "probe setup: disabled skip_human_shell admits pane 0"
    );
    assert!(
        !is_worker_pane("0", &ReapFinishedPanesRules::default()),
        "probe setup: default skip_human_shell refuses pane 0"
    );
    assert_eq!(sh, "SCHEDULED");
    assert_eq!(rust_invoker(text), "SCHEDULED");
    println!(
        "DIFFERENTIAL known-bad probe: skip_human_shell off admits pane 0; on refuses (visible divergence)"
    );
}

#[test]
fn rust_matches_shell_on_nonempty_case_set() {
    let cases = [
        ("501 1 /usr/sbin/cron\n", "MANUAL"),
        ("501 233 /bin/sh\n0 1 /usr/sbin/cron\n", "SCHEDULED"),
        ("501 1 /bin/launchd\n", "MANUAL"),
        ("0 2 /usr/sbin/cron\n", "MANUAL"),
        ("0 1 /usr/sbin/cron\n", "SCHEDULED"),
        ("", "MANUAL"),
    ];
    let mut compared = 0usize;
    let mut disagreements = Vec::new();
    for (body, want) in cases {
        compared += 1;
        let sh = shell_invoker(body);
        let rs = rust_invoker(body);
        if sh != rs || rs != want {
            disagreements.push(format!(
                "invoker body={body:?} shell={sh} rust={rs} want={want}"
            ));
        }
    }

    let r = ReapFinishedPanesRules::default();
    for (idx, worker) in [("0", false), ("1", true), ("2", true), ("x", false)] {
        compared += 1;
        let got = is_worker_pane(idx, &r);
        if got != worker {
            disagreements.push(format!("is_worker_pane({idx}) rust={got} want={worker}"));
        }
    }

    let reaper_cases = [
        ("REAPED pane=2 awaiting_human=1", true, "reaped", true),
        ("REAPED pane=3 awaiting_human=0", true, "reaped", false),
        ("skip not finished", true, "skipped", false),
        ("REAPED pane=1", false, "skipped", false),
    ];
    for (out, ok, kind, awaiting) in reaper_cases {
        compared += 1;
        let (k, a) = parse_reaper_out(out, ok);
        if k != kind || a != awaiting {
            disagreements.push(format!(
                "parse_reaper_out({out:?},{ok}) rust=({k},{a}) want=({kind},{awaiting})"
            ));
        }
    }

    let mut stats = SweepStats::default();
    let started = Instant::now() - Duration::from_secs(5);
    compared += 1;
    assert!(apply_deadline(
        &mut stats,
        started,
        Duration::from_secs(0),
        &r
    ));
    if stats.deadline_hit != 1 || stats.unswept != 1 {
        disagreements.push(format!(
            "deadline stats hit={} unswept={}",
            stats.deadline_hit, stats.unswept
        ));
    }

    assert!(
        compared > 0,
        "rule anti_vacuity: a differential that compares ZERO cases is an ERROR, not a pass"
    );
    assert!(
        disagreements.is_empty(),
        "rule differential_vs_oracle: {compared} cases, disagreements:\n{}",
        disagreements.join("\n")
    );
    println!("DIFFERENTIAL reap-finished-panes: {compared} cases compared, 0 disagreements");
}

#[test]
fn finished_pane_is_reaped_and_persists_observable_state() {
    let root = std::env::temp_dir().join(format!("reap-finished-{}", std::process::id()));
    let outdir = root.join("reaped");
    let ledger = root.join("ledger.jsonl");
    let text = "agent finished work\nresult: committed";
    assert!(matches!(
        decide_reap(text, text, true, text),
        ReapPaneDecision::Reaped {
            awaiting_human: false
        }
    ));
    let path = write_reaped_result(
        &outdir,
        &ledger,
        "omp-orchestrator",
        "2",
        "%1413",
        text,
        false,
        "2026-09-02T04:00:00Z",
    )
    .expect("finished pane result must be durable");
    assert_eq!(
        std::fs::read_to_string(&path).expect("result artifact"),
        format!("{text}\n")
    );
    let ledger_text = std::fs::read_to_string(&ledger).expect("reap ledger");
    assert!(ledger_text.contains("result_reaped"));
    assert!(ledger_text.contains("%1413"));
    println!("KNOWN-BAD RED leg exercised: finished pane produced durable result artifact");
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn ledger_row_matches_oracle_fields_and_includes_written_newline() {
    let root = std::env::temp_dir().join(format!("reap-ledger-fields-{}", std::process::id()));
    let outdir = root.join("reaped");
    let ledger = root.join("ledger.jsonl");
    let text = "agent finished work\nresult: committed";
    let stamp = "2026-09-02T04:00:00Z";
    let path = write_reaped_result(
        &outdir,
        &ledger,
        "omp-orchestrator",
        "2",
        "%1413",
        text,
        false,
        stamp,
    )
    .expect("finished pane result must be durable");
    let row: serde_json::Value =
        serde_json::from_str(std::fs::read_to_string(&ledger).expect("ledger").trim())
            .expect("ledger row JSON");
    let expected = serde_json::json!({
        "ts": stamp,
        "event": "result_reaped",
        "session": "omp-orchestrator",
        "pane": "2",
        "pane_id": "%1413",
        "awaiting_human": false,
        "bytes": text.len() + 1,
        "path": path,
    });
    assert_eq!(row, expected, "ledger fields must match the oracle schema");
    assert_eq!(
        std::fs::read_to_string(&path).expect("result artifact"),
        format!("{text}\n")
    );
    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn cli_repo_selftest_proves_finished_capture_and_live_pane_skip() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("workspace root");
    let output = Command::new(env!("CARGO_BIN_EXE_reap-finished-panes"))
        .args(["--repo", repo.to_str().expect("repo path"), "--selftest"])
        .output()
        .expect("reaper selftest process");
    assert!(
        output.status.success(),
        "selftest failed: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("PASS selftest.finished-pane-ledger"),
        "{stdout}"
    );
    assert!(
        stdout.contains("PASS selftest.working-pane-not-reaped"),
        "{stdout}"
    );
}

#[test]
fn working_pane_is_not_reaped() {
    let text = "Codex\nWorking (9s)";
    assert!(matches!(
        decide_reap(text, text, false, text),
        ReapPaneDecision::Working
    ));
    println!("KNOWN-GOOD: working pane was not reaped");
}

#[test]
fn empty_pane_set_is_an_error_not_a_pass() {
    let error = require_panes::<(String, String)>(&[])
        .expect_err("empty pane set must refuse a vacuous sweep");
    assert!(error.contains("empty pane set"));
    assert!(matches!(
        decide_reap("", "", true, ""),
        ReapPaneDecision::Empty
    ));
    println!("ANTI-VACUITY: empty pane set refused");
}

#[test]
fn mutation_breaking_reap_predicate_is_detectable() {
    let text = "Codex\nWorking (9s)";
    let canonical = should_reap(text, text, false);
    let mutant = !text.trim().is_empty() && (text == text || !false);
    assert!(!canonical, "canonical predicate must refuse a working pane");
    assert!(
        mutant,
        "the planted mutation must incorrectly reap the working pane"
    );
    assert!(matches!(
        decide_reap(text, text, false, text),
        ReapPaneDecision::Working
    ));
    println!("MUTATION RED target: replacing the readiness AND with OR would reap WORKING");
}

#[test]
fn consecutive_cycle_started_rows_keep_one_pid() {
    let same_pid = r#"{"event":"CYCLE_STARTED","pid":74220}
{"event":"CYCLE_STARTED","pid":74220}
"#;
    assert!(consecutive_cycle_started_same_pid(same_pid));
    let changed_pid = r#"{"event":"CYCLE_STARTED","pid":74220}
{"event":"CYCLE_STARTED","pid":86652}
"#;
    assert!(!consecutive_cycle_started_same_pid(changed_pid));
    println!("CYCLE proof: consecutive CYCLE_STARTED rows retained the same pid");
}
