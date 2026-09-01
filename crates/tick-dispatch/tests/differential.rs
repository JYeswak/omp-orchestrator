//! Differential vs bin/tick-dispatch.sh case statements on admission.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_tick-dispatch"))
}

fn eval(input: &str, extra: &[&str]) -> (i32, String) {
    let mut child = Command::new(rust_bin())
        .arg("--eval-admission")
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    (
        out.status.code().unwrap_or(99),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

fn shell_expected(
    verdict: &str,
    force: bool,
    disc: i32,
    empty: bool,
    check: i32,
    ready: i32,
) -> i32 {
    match verdict {
        "DONE" | "IDLE" => {}
        _ if force => {}
        _ => return 1,
    }
    match disc {
        0 => {}
        _ => return 1,
    }
    if empty {
        return 1;
    }
    if check != 0 {
        return 1;
    }
    if ready != 0 {
        return 1;
    }
    0
}

#[test]
fn rust_matches_shell_admission_table() {
    let cases: &[(&str, bool, i32, bool, i32, i32)] = &[
        ("DONE", false, 0, false, 0, 0),
        ("IDLE", false, 0, false, 0, 0),
        ("WORKING", false, 0, false, 0, 0),
        ("WORKING", true, 0, false, 0, 0),
        ("UNREADABLE", false, 0, false, 0, 0),
        ("NO_PANE", false, 0, false, 0, 0),
        ("IDLE", false, 1, false, 0, 0),
        ("IDLE", false, 2, false, 0, 0),
        ("IDLE", false, 99, false, 0, 0),
        ("DONE", false, 0, true, 0, 0),
        ("DONE", false, 0, false, 1, 0),
        ("DONE", false, 0, false, 0, 1),
    ];
    let mut disagrees = 0;
    for (v, f, d, e, c, r) in cases {
        let input = format!(
            "verdict={v}\nforce_busy={}\ndisc_rc={d}\nrendered_empty={}\ncheck_rc={c}\nready_rc={r}\npane=2\nsend_rc=0\njq_success=1\n",
            if *f { 1 } else { 0 },
            if *e { 1 } else { 0 },
        );
        let (rc, out) = eval(&input, &[]);
        let want = shell_expected(v, *f, *d, *e, *c, *r);
        if rc != want {
            disagrees += 1;
            eprintln!("DISAGREE v={v} force={f} disc={d} empty={e} check={c} ready={r}: rust={rc} want={want} out={out}");
        }
    }
    assert_eq!(
        disagrees,
        0,
        "{} cases, {disagrees} disagreements",
        cases.len()
    );
    println!(
        "DIFFERENTIAL tick-dispatch: {} cases, 0 disagreements",
        cases.len()
    );
}

#[test]
fn comparator_sees_working_refuse() {
    let (rc, out) = eval(
        "verdict=WORKING\nforce_busy=0\ndisc_rc=0\nrendered_empty=0\ncheck_rc=0\nready_rc=0\n",
        &[],
    );
    assert_eq!(rc, 1);
    assert!(out.contains("REFUSED"));
    let (rc2, out2) = eval(
        "verdict=WORKING\nforce_busy=0\ndisc_rc=0\nrendered_empty=0\ncheck_rc=0\nready_rc=0\n",
        &["--mutation", "--disable-rule", "refuse_busy"],
    );
    assert_eq!(rc2, 0, "blinded WORKING must ALLOW, got {out2}");
    println!(
        "KNOWN-BAD PROBE tick-dispatch: WORKING REFUSED rc=1; blinded refuse_busy -> ALLOW rc=0"
    );
}
