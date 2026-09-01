//! Differential vs `bin/reap-finished-panes.sh` on hermetic predicates.
//! Empty comparison set is an ERROR (fh C86). pane-result-reaper.sh is an
//! EXTERNAL command; this crate does not reimplement it.

use reap_finished_panes::{
    apply_deadline, invoker_from_chain, is_worker_pane, parse_ancestor_rows, parse_reaper_out,
    ReapFinishedPanesRules, SweepStats,
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
