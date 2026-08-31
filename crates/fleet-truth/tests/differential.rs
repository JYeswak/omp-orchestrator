//! Differential vs `bin/fleet-truth.sh` scoring. Same sensor JSON on both sides.
//! Empty comparison set is an ERROR (fh C86).

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fleet-truth"))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OracleStatus {
    Ready,
    MissingInterpreter,
}

fn oracle_status() -> OracleStatus {
    match Command::new("python3")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => OracleStatus::Ready,
        _ => OracleStatus::MissingInterpreter,
    }
}

fn announce_skip(test: &str, status: &OracleStatus) {
    let reason = match status {
        OracleStatus::MissingInterpreter => "missing_interpreter",
        OracleStatus::Ready => "ready",
    };
    println!(
        "DIFFERENTIAL DID NOT RUN: test={test} reason={reason} detail=inline python3 -c\n  \
         This is a development-only comparison, not a gate. The Rust gate for this crate is \
         src/lib.rs unit tests.\n  \
         0 cases compared. This is NOT a passing differential."
    );
}

fn py_score(input: &str) -> String {
    // Replica of bin/fleet-truth.sh truth_row scoring (the python-free arithmetic).
    let script = r#"
import json,sys
s=json.load(sys.stdin)
vstate=s.get("vstate","OK")
if vstate!="OK":
    print("999|%s|%s|?|?|?|?|?|UNKNOWN|identity_unknown|%s (fail-closed: rank-high, inspect)"%(s.get("session","s"),s.get("repo","/r"),vstate))
    raise SystemExit
ac=int(s.get("commits",0)); dirty=int(s.get("dirty",0)); behind=int(s.get("behind",0))
ctx=str(s.get("ctx","UNKNOWN")); bclose=str(s.get("bclose","UNKNOWN"))
save_age=str(s.get("save_age","UNKNOWN")); save_alert=str(s.get("save_alert","save_unknown"))
score=0; reason=""
if ac==0:
    score+=100; reason="0 commits in window"
else:
    reason="shipping (%s commits)"%ac
if dirty>=50 and ac==0:
    score+=40; reason=reason+"; %s dirty undelivered"%dirty
if behind>=20:
    score+=30; reason=reason+"; behind %s"%behind
if ctx!="UNKNOWN":
    try:
        if float(ctx)>=85:
            score+=25; reason=reason+"; ctx %s%%"%ctx
    except Exception:
        pass
if bclose=="UNKNOWN":
    score+=15; reason=reason+"; UNKNOWN bead-db"
if save_alert=="ok":
    pass
elif save_alert.startswith("not_on_main:"):
    score+=30; reason=reason+"; fleet-ops "+save_alert
elif save_alert.startswith("save_stale:"):
    score+=20; reason=reason+"; fleet-ops "+save_alert
elif save_alert=="save_unknown":
    score+=10; reason=reason+"; fleet-ops save_unknown"
print("%s|%s|%s|%s|%s|%s|%s|%s|%s|%s|%s"%(score,s.get("session","s"),s.get("repo","/r"),ac,bclose,dirty,behind,ctx,save_age,save_alert,reason))
"#;
    let mut child = Command::new("python3")
        .arg("-c")
        .arg(script)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("py");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

fn rust_score(input: &str) -> String {
    let mut child = Command::new(rust_bin())
        .arg("--eval-row")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("rust");
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

/// A healthy bead-close sensor is recent; keep its timestamp relative to the test clock.
fn now_rfc3339() -> String {
    let out = Command::new("/bin/date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .expect("date -u");
    String::from_utf8_lossy(&out.stdout).trim().to_string()
}

#[test]
fn comparator_sees_manufactured_disagreement() {
    let status = oracle_status();
    let OracleStatus::Ready = status else {
        announce_skip("comparator_sees_manufactured_disagreement", &status);
        return;
    };
    let input = r#"{"session":"s","repo":"/r","vstate":"UNKNOWN:no-git-at-cwd","commits":0,"dirty":0,"behind":0,"ctx":"?","bclose":"?","save_age":"UNKNOWN","save_alert":"identity_unknown","ntm_state":""}"#;
    let py = py_score(input);
    let mut child = Command::new(rust_bin())
        .args([
            "--eval-row",
            "--mutation",
            "--disable-rule",
            "identity_unknown_ranks_high",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    let rs = String::from_utf8_lossy(&child.wait_with_output().unwrap().stdout)
        .trim()
        .to_string();
    assert!(
        py.starts_with("999|"),
        "probe setup: python identity-unknown must rank 999, got {py}"
    );
    assert!(
        rs.starts_with("0|"),
        "probe setup: rust with identity_unknown_ranks_high disabled must rank 0, got {rs}"
    );
    assert_ne!(
        py, rs,
        "rule comparator_not_vacuous: a manufactured disagreement must be visible"
    );
}

#[test]
fn rust_matches_shell_scoring_on_nonempty_case_set() {
    let status = oracle_status();
    let OracleStatus::Ready = status else {
        announce_skip("rust_matches_shell_scoring_on_nonempty_case_set", &status);
        return;
    };
    let healthy_bclose = now_rfc3339();
    let healthy = format!(
        r#"{{"session":"fix-healthy","repo":"/h","vstate":"OK","commits":1,"dirty":0,"behind":0,"ctx":"10.0","bclose":"{healthy_bclose}","save_age":"0.1","save_alert":"ok","ntm_state":"ERROR"}}"#
    );
    let cases = [
        r#"{"session":"fix-stale","repo":"/stale","vstate":"OK","commits":0,"dirty":60,"behind":0,"ctx":"UNKNOWN","bclose":"UNKNOWN","save_age":"UNKNOWN","save_alert":"save_unknown","ntm_state":"BUSY"}"#,
        healthy.as_str(),
        r#"{"session":"fix-nogit","repo":"/nonexistent-repo-xyz","vstate":"UNKNOWN:no-git-at-cwd","commits":0,"dirty":0,"behind":0,"ctx":"?","bclose":"?","save_age":"UNKNOWN","save_alert":"identity_unknown","ntm_state":""}"#,
        r#"{"session":"hot","repo":"/r","vstate":"OK","commits":0,"dirty":0,"behind":0,"ctx":"90.0","bclose":"UNKNOWN","save_age":"UNKNOWN","save_alert":"save_unknown","ntm_state":"ERROR"}"#,
        r#"{"session":"behind","repo":"/r","vstate":"OK","commits":3,"dirty":0,"behind":25,"ctx":"UNKNOWN","bclose":"ts","save_age":"1.0","save_alert":"ok","ntm_state":""}"#,
        r#"{"session":"branch","repo":"/r","vstate":"OK","commits":0,"dirty":0,"behind":0,"ctx":"UNKNOWN","bclose":"UNKNOWN","save_age":"UNKNOWN","save_alert":"not_on_main:feat","ntm_state":""}"#,
        r#"{"session":"stale-save","repo":"/r","vstate":"OK","commits":2,"dirty":0,"behind":0,"ctx":"UNKNOWN","bclose":"ts","save_age":"30.0","save_alert":"save_stale:30.0h","ntm_state":"WAITING"}"#,
    ];
    let mut compared = 0usize;
    let mut disagreements = Vec::new();
    for (i, body) in cases.iter().enumerate() {
        compared += 1;
        let py = py_score(body);
        let rs = rust_score(body);
        if py != rs {
            disagreements.push(format!("case {i}: py={py} rust={rs}"));
        }
    }
    assert!(
        compared > 0,
        "rule anti_vacuity: a differential that compares ZERO cases is an ERROR, not a pass"
    );
    assert!(
        disagreements.is_empty(),
        "{} disagreement(s) of {compared} cases:\n{}",
        disagreements.len(),
        disagreements.join("\n")
    );
    assert!(
        rust_score(cases[1]).contains(&healthy_bclose),
        "RULE recent-bead-close-preserved: the healthy sensor's runtime-relative timestamp must remain visible in the scored row"
    );
    println!("DIFFERENTIAL PASS cases={compared} disagreements=0");
}
