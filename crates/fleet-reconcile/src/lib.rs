#![forbid(unsafe_code)]

//! NTM vs tmux ground-truth compare, ported from `bin/fleet-reconcile.sh`.
//!
//! The shell file is the differential oracle and is not edited by this crate.
//! Detector order is cheapest-to-deeper (fh C25). Each failure names itself (fh C31).
//! FT silence is fail-open-and-loud and NEVER changes the ntm/tmux verdict.
//!
//! Verdicts go to STDOUT at column 0 in both directions (fh G1).

use serde_json::{json, Map, Value};
use std::collections::BTreeSet;
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// Spawn a child with stdin=null (O_CLOEXEC on every other fd) and an explicit deadline.
/// Drop of the Child on kill/wait releases pipes. No unbounded wait.
pub fn spawn_timeout(mut cmd: Command, timeout: Duration) -> Option<Output> {
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().ok()?;
    // DRAIN THE PIPES ON DEDICATED THREADS.  `try_wait` in a poll loop CANNOT be paired with
    // undrained pipes: a child that writes more than the OS pipe buffer (~64 KiB, and stdout and
    // stderr each have their own) blocks in `write` forever, so it never exits, so `try_wait`
    // never returns Some, and the call burns its entire timeout at 0% CPU before being killed.
    //
    // MEASURED 2026-08-27: `git -C <repo> log --since "24 hours ago" --oneline` completes in
    // 0.6-0.9s from a shell, and sat at 0.0% CPU for 104s as a child here -- reproduced exactly by
    // polling `try_wait` without reading the pipes.  Six crates shared this shape; fixing only the
    // one that fired would have left five live.
    let out = child.stdout.take().map(|mut r| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            buf
        })
    });
    let err = child.stderr.take().map(|mut r| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = r.read_to_end(&mut buf);
            buf
        })
    });
    let start = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(s)) => break s,
            Ok(None) if start.elapsed() >= timeout => {
                let _ = child.kill();
                break child.wait().ok()?;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(20)),
            Err(_) => return None,
        }
    };
    // The readers end when the child's fds close, which the kill above guarantees.
    let stdout = out.and_then(|h| h.join().ok()).unwrap_or_default();
    let stderr = err.and_then(|h| h.join().ok()).unwrap_or_default();
    Some(Output { status, stdout, stderr })
}

pub const FAIL_MODE: &str =
    "fail_closed_on_ntm_tmux_disagree; fail_open_and_loud_on_ft_unavailable";
pub const SCHEMA: &str = "zs.fleet-reconcile.v1";

/// fh C75: one enum is the authority for mutation-rule names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FleetReconcileRule {
    EmptySuccessFailsClosed,
    ListEmptyTextFailsClosed,
    NameSetsMustAgree,
    UnparseableIsFail,
}

impl FleetReconcileRule {
    pub const ALL: &'static [FleetReconcileRule] = &[
        FleetReconcileRule::EmptySuccessFailsClosed,
        FleetReconcileRule::ListEmptyTextFailsClosed,
        FleetReconcileRule::NameSetsMustAgree,
        FleetReconcileRule::UnparseableIsFail,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            FleetReconcileRule::EmptySuccessFailsClosed => "empty_success_fails_closed",
            FleetReconcileRule::ListEmptyTextFailsClosed => "list_empty_text_fails_closed",
            FleetReconcileRule::NameSetsMustAgree => "name_sets_must_agree",
            FleetReconcileRule::UnparseableIsFail => "unparseable_is_fail",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.as_str() == name)
    }
}

#[derive(Clone, Debug)]
pub struct FleetReconcileRules {
    pub empty_success_fails_closed: bool,
    pub list_empty_text_fails_closed: bool,
    pub name_sets_must_agree: bool,
    pub unparseable_is_fail: bool,
}

impl Default for FleetReconcileRules {
    fn default() -> Self {
        Self {
            empty_success_fails_closed: true,
            list_empty_text_fails_closed: true,
            name_sets_must_agree: true,
            unparseable_is_fail: true,
        }
    }
}

impl FleetReconcileRules {
    pub fn disable(&mut self, name: &str) -> bool {
        let Some(rule) = FleetReconcileRule::parse(name) else {
            return false;
        };
        match rule {
            FleetReconcileRule::EmptySuccessFailsClosed => self.empty_success_fails_closed = false,
            FleetReconcileRule::ListEmptyTextFailsClosed => self.list_empty_text_fails_closed = false,
            FleetReconcileRule::NameSetsMustAgree => self.name_sets_must_agree = false,
            FleetReconcileRule::UnparseableIsFail => self.unparseable_is_fail = false,
        }
        true
    }

    pub fn known_names_csv() -> String {
        FleetReconcileRule::ALL
            .iter()
            .map(|r| r.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InnerVerdict {
    pub detector: String,
    pub verdict: String,
    pub tmux_count: i64,
    pub ntm_count: i64,
    pub detail: String,
}

/// Controller-tick / loop-tick gate: non-PASS or nonzero rc BLOCKS dispatch.
pub fn observe_blocks_dispatch(verdict: &str, rc: i32) -> bool {
    rc != 0 || verdict != "PASS"
}

/// Provenance by process lineage, matching `bin/lib/invoker-lineage.sh`.
/// SCHEDULED requires uid 0 AND ppid 1 AND comm `/usr/sbin/cron` on one ancestor row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FleetReconcileInvoker {
    pub invoker: &'static str,
    pub proof: &'static str,
}

impl FleetReconcileInvoker {
    pub const MANUAL: FleetReconcileInvoker = FleetReconcileInvoker {
        invoker: "MANUAL",
        proof: "unproven_parent",
    };
    pub const SCHEDULED: FleetReconcileInvoker = FleetReconcileInvoker {
        invoker: "SCHEDULED",
        proof: "cron_parent",
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetReconcileAncestorRow {
    pub uid: u32,
    pub ppid: u32,
    pub comm: String,
}

pub fn parse_ancestor_rows(text: &str) -> Vec<FleetReconcileAncestorRow> {
    let mut out = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let (Some(uid), Some(ppid), Some(comm)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        let (Ok(uid), Ok(ppid)) = (uid.parse::<u32>(), ppid.parse::<u32>()) else {
            continue;
        };
        out.push(FleetReconcileAncestorRow {
            uid,
            ppid,
            comm: comm.to_string(),
        });
    }
    out
}

pub fn invoker_from_chain(chain: &[FleetReconcileAncestorRow]) -> FleetReconcileInvoker {
    for row in chain {
        if row.uid == 0 && row.ppid == 1 && row.comm == "/usr/sbin/cron" {
            return FleetReconcileInvoker::SCHEDULED;
        }
    }
    FleetReconcileInvoker::MANUAL
}

/// Honor an inherited SCHEDULED/cron_parent pair only when this process's own chain also reaches cron.
pub fn invoker_resolve_env(inv: &str, proof: &str, own_chain: &[FleetReconcileAncestorRow]) -> FleetReconcileInvoker {
    if inv == "SCHEDULED" && proof == "cron_parent" {
        let own = invoker_from_chain(own_chain);
        if own == FleetReconcileInvoker::SCHEDULED {
            return FleetReconcileInvoker::SCHEDULED;
        }
        return FleetReconcileInvoker::MANUAL;
    }
    invoker_from_chain(own_chain)
}

pub fn parse_session_names(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect()
}

pub fn classify_ft(raw: &str) -> &'static str {
    if raw.trim().is_empty() {
        return "unavailable";
    }
    match serde_json::from_str::<Value>(raw) {
        Ok(d) if d.get("ok") == Some(&Value::Bool(true)) => "ok",
        Ok(_) => "degraded",
        Err(_) => "degraded",
    }
}

/// Pure ntm-vs-tmux verdict. Classifier busy/error labels are never consulted.
pub fn reconcile_inner(
    tmux_text: &str,
    list_text: &str,
    snap_raw: &str,
    rules: &FleetReconcileRules,
) -> InnerVerdict {
    let tmux = parse_session_names(tmux_text);
    let mut ntm_names = BTreeSet::new();

    let snap: Option<Value> = if snap_raw.trim().is_empty() {
        None
    } else {
        match serde_json::from_str(snap_raw) {
            Ok(v) => Some(v),
            Err(_) => {
                return fail(
                    "ntm_snapshot_unparseable",
                    "ntm --robot-snapshot did not parse as JSON",
                    &tmux,
                    &ntm_names,
                    rules.unparseable_is_fail,
                );
            }
        }
    };

    let Some(snap) = snap else {
        return fail(
            "ntm_snapshot_unparseable",
            "ntm --robot-snapshot was not a JSON object",
            &tmux,
            &ntm_names,
            rules.unparseable_is_fail,
        );
    };
    if !snap.is_object() {
        return fail(
            "ntm_snapshot_unparseable",
            "ntm --robot-snapshot was not a JSON object",
            &tmux,
            &ntm_names,
            rules.unparseable_is_fail,
        );
    }

    let successish = snap.get("success") == Some(&Value::Bool(true))
        || snap.get("ok") == Some(&Value::Bool(true));
    let summary = snap.get("summary").cloned().unwrap_or(Value::Null);
    let total = summary
        .get("total_sessions")
        .and_then(|v| v.as_i64())
        .or_else(|| {
            summary
                .get("total_sessions")
                .and_then(|v| v.as_u64())
                .map(|n| n as i64)
        });
    if let Some(Value::Array(sessions)) = snap.get("sessions") {
        for row in sessions {
            if let Some(name) = row.get("name").and_then(|v| v.as_str()) {
                if !name.is_empty() {
                    ntm_names.insert(name.to_string());
                }
            }
        }
    }

    let list_lc = list_text.to_ascii_lowercase();
    if rules.list_empty_text_fails_closed
        && list_lc.contains("no tmux sessions")
        && !tmux.is_empty()
    {
        return fail(
            "ntm_list_empty_text",
            &format!(
                "ntm list printed empty-fleet text while tmux has {} session(s); exit-code checks cannot see this",
                tmux.len()
            ),
            &tmux,
            &ntm_names,
            true,
        );
    }

    let empty_success = successish
        && (total == Some(0) || (total.is_none() && ntm_names.is_empty()))
        && !tmux.is_empty();
    if rules.empty_success_fails_closed && empty_success {
        let total_s = match total {
            Some(n) => n.to_string(),
            None => "None".into(),
        };
        return fail(
            "ntm_empty_success_with_live_tmux",
            &format!(
                "ntm --robot-snapshot success/ok with total_sessions={} names={} while tmux has {} live session(s)",
                total_s,
                ntm_names.len(),
                tmux.len()
            ),
            &tmux,
            &ntm_names,
            true,
        );
    }

    if rules.name_sets_must_agree && ntm_names != tmux {
        let only_tmux: Vec<_> = tmux.difference(&ntm_names).cloned().collect();
        let only_ntm: Vec<_> = ntm_names.difference(&tmux).cloned().collect();
        return fail(
            "ntm_tmux_disagree",
            &format!("session name sets differ; only_tmux={only_tmux:?} only_ntm={only_ntm:?}"),
            &tmux,
            &ntm_names,
            true,
        );
    }

    InnerVerdict {
        detector: "ntm_tmux_agree".into(),
        verdict: "PASS".into(),
        tmux_count: tmux.len() as i64,
        ntm_count: ntm_names.len() as i64,
        detail: format!("ntm and tmux agree on {} session(s)", tmux.len()),
    }
}

fn fail(
    detector: &str,
    detail: &str,
    tmux: &BTreeSet<String>,
    ntm: &BTreeSet<String>,
    actually_fail: bool,
) -> InnerVerdict {
    if !actually_fail {
        return InnerVerdict {
            detector: "ntm_tmux_agree".into(),
            verdict: "PASS".into(),
            tmux_count: tmux.len() as i64,
            ntm_count: ntm.len() as i64,
            detail: "mutation disabled this fail-closed detector".into(),
        };
    }
    InnerVerdict {
        detector: detector.into(),
        verdict: "FAIL".into(),
        tmux_count: tmux.len() as i64,
        ntm_count: ntm.len() as i64,
        detail: detail.into(),
    }
}

pub fn emit_envelope(
    inner: &InnerVerdict,
    ft_status: &str,
    invoker: &str,
    invoker_proof: &str,
) -> Value {
    let mut detail = inner.detail.clone();
    if ft_status != "ok" {
        detail.push_str(&format!(
            "; FT {ft_status} (fail-open-and-loud: NTM remains dispatch authority)"
        ));
    }
    let mut m = Map::new();
    m.insert("detail".into(), json!(detail));
    m.insert("detector".into(), json!(inner.detector));
    m.insert("fail_mode".into(), json!(FAIL_MODE));
    m.insert("ft_status".into(), json!(ft_status));
    m.insert("invoker".into(), json!(invoker));
    m.insert("invoker_proof".into(), json!(invoker_proof));
    m.insert("ntm_count".into(), json!(inner.ntm_count));
    m.insert("schema".into(), json!(SCHEMA));
    m.insert("tmux_count".into(), json!(inner.tmux_count));
    m.insert("verdict".into(), json!(inner.verdict));
    Value::Object(m)
}

pub fn exit_for(verdict: &str) -> i32 {
    if verdict == "PASS" {
        0
    } else {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules() -> FleetReconcileRules {
        FleetReconcileRules::default()
    }

    fn snap(names: &[&str], total: i64, success: bool) -> String {
        let sess: Vec<_> = names.iter().map(|n| json!({"name": n})).collect();
        json!({
            "success": success,
            "summary": {"total_sessions": total},
            "sessions": sess
        })
        .to_string()
    }

    #[test]
    fn agree_is_pass() {
        let v = reconcile_inner(
            "alpha\nbeta\n",
            "  alpha: 2\n",
            &snap(&["alpha", "beta"], 2, true),
            &rules(),
        );
        assert_eq!(v.detector, "ntm_tmux_agree");
        assert_eq!(v.verdict, "PASS");
        assert!(!observe_blocks_dispatch(&v.verdict, exit_for(&v.verdict)));
    }

    #[test]
    fn empty_success_with_live_tmux_fails() {
        let v = reconcile_inner(
            "control-plane\nalpsinsurance\n",
            "  control-plane: 3 panes\n",
            &snap(&[], 0, true),
            &rules(),
        );
        assert_eq!(
            v.detector, "ntm_empty_success_with_live_tmux",
            "rule empty_success_fails_closed: ntm#254 empty-success must FAIL"
        );
        assert_eq!(v.verdict, "FAIL");
        assert!(observe_blocks_dispatch(&v.verdict, exit_for(&v.verdict)));
    }

    #[test]
    fn list_empty_text_fails_even_when_snapshot_agrees() {
        let v = reconcile_inner(
            "control-plane\n",
            "No tmux sessions running\n",
            &snap(&["control-plane"], 1, true),
            &rules(),
        );
        assert_eq!(
            v.detector, "ntm_list_empty_text",
            "rule list_empty_text_fails_closed: empty-text is not an rc check"
        );
        assert_eq!(v.verdict, "FAIL");
    }

    #[test]
    fn name_set_disagree_fails() {
        let v = reconcile_inner(
            "alpha\nbeta\n",
            "  alpha: 1 pane\n",
            &snap(&["alpha"], 1, true),
            &rules(),
        );
        assert_eq!(v.detector, "ntm_tmux_disagree");
        assert_eq!(v.verdict, "FAIL");
        assert!(
            observe_blocks_dispatch(&v.verdict, 1),
            "rule name_sets_must_agree: a non-PASS reconcile verdict must BLOCK dispatch"
        );
    }

    #[test]
    fn unparseable_snapshot_fails_not_pass() {
        let v = reconcile_inner("alpha\n", "alpha:\n", "not json", &rules());
        assert_eq!(v.detector, "ntm_snapshot_unparseable");
        assert_eq!(
            v.verdict, "FAIL",
            "rule unparseable_is_fail: an unobservable snapshot yields FAIL, never a false PASS"
        );
    }

    #[test]
    fn classifier_label_is_not_consulted() {
        let snap = json!({
            "success": true,
            "summary": {"total_sessions": 1},
            "sessions": [{"name":"alpha","agents":[{"state":"error","pane":"1"}]}]
        })
        .to_string();
        let v = reconcile_inner("alpha\n", "  alpha: 1 pane\n", &snap, &rules());
        assert_eq!(
            v.verdict, "PASS",
            "rule ground_truth_not_classifier: ntm error label alone never establishes disagreement"
        );
        assert_eq!(v.detector, "ntm_tmux_agree");
    }

    #[test]
    fn ft_degraded_does_not_flip_pass() {
        let inner = InnerVerdict {
            detector: "ntm_tmux_agree".into(),
            verdict: "PASS".into(),
            tmux_count: 2,
            ntm_count: 2,
            detail: "ntm and tmux agree on 2 session(s)".into(),
        };
        let env = emit_envelope(&inner, "degraded", "MANUAL", "unproven_parent");
        assert_eq!(env["verdict"], "PASS");
        assert!(env["detail"].as_str().unwrap().contains("FT degraded"));
    }

    #[test]
    fn disabling_empty_success_falls_through_to_name_set() {
        let mut r = rules();
        assert!(r.disable("empty_success_fails_closed"));
        let v = reconcile_inner(
            "control-plane\n",
            "  control-plane: 3 panes\n",
            &snap(&[], 0, true),
            &r,
        );
        assert_eq!(
            v.detector, "ntm_tmux_disagree",
            "empty_success is a named ntm#254 detector; name-set still fail-closes the same inventory lie"
        );
        assert_eq!(v.verdict, "FAIL");
        assert!(observe_blocks_dispatch(&v.verdict, exit_for(&v.verdict)));
    }

    #[test]
    fn disabling_list_empty_text_false_passes_when_snapshot_agrees() {
        let mut r = rules();
        assert!(r.disable("list_empty_text_fails_closed"));
        let v = reconcile_inner(
            "control-plane\n",
            "No tmux sessions running\n",
            &snap(&["control-plane"], 1, true),
            &r,
        );
        assert_eq!(
            v.verdict, "PASS",
            "mutation list_empty_text_fails_closed: deleting it admits ntm list empty-text"
        );
    }

    #[test]
    fn disabling_unparseable_false_passes() {
        let mut r = rules();
        assert!(r.disable("unparseable_is_fail"));
        let v = reconcile_inner("alpha\n", "alpha:\n", "not json", &r);
        assert_eq!(
            v.verdict, "PASS",
            "mutation unparseable_is_fail: deleting it yields a false PASS on an unobservable snapshot"
        );
    }

    #[test]
    fn non_pass_verdict_blocks_dispatch_even_at_rc0() {
        assert!(
            observe_blocks_dispatch("FAIL", 0),
            "rule non_pass_blocks_dispatch: a FAIL verdict with rc=0 must still block"
        );
        assert!(observe_blocks_dispatch("PASS", 1));
        assert!(!observe_blocks_dispatch("PASS", 0));
        // Delete the verdict half of the gate (rc-only) and a FAIL at rc=0 would GREEN.
        let rc_only_would_block = |rc: i32| rc != 0;
        assert!(
            !rc_only_would_block(0),
            "rule non_pass_blocks_dispatch: rc-only is the known-bad of deleting the verdict check"
        );
    }

    #[test]
    fn invoker_genuine_cron_certifies() {
        let chain = parse_ancestor_rows("501 233 /bin/sh\n0 1 /usr/sbin/cron\n");
        assert_eq!(invoker_from_chain(&chain), FleetReconcileInvoker::SCHEDULED);
    }

    #[test]
    fn invoker_uid_forgery_refused() {
        let chain = parse_ancestor_rows("501 1 /usr/sbin/cron\n");
        assert_eq!(invoker_from_chain(&chain), FleetReconcileInvoker::MANUAL);
    }

    #[test]
    fn invoker_env_scheduled_without_lineage_is_demoted() {
        let chain = parse_ancestor_rows("501 1 /bin/zsh\n");
        assert_eq!(
            invoker_resolve_env("SCHEDULED", "cron_parent", &chain),
            FleetReconcileInvoker::MANUAL,
            "inherited SCHEDULED/cron_parent is forgeable unless this process's chain also reaches cron"
        );
    }

    #[test]
    fn disabling_name_sets_false_passes_disagree() {
        let mut r = rules();
        assert!(r.disable("name_sets_must_agree"));
        let v = reconcile_inner("alpha\nbeta\n", "", &snap(&["alpha"], 1, true), &r);
        assert_eq!(
            v.verdict, "PASS",
            "mutation name_sets_must_agree: disabling it must admit a disagree"
        );
    }

    #[test]
    fn every_named_rule_is_disableable() {
        assert!(!FleetReconcileRule::ALL.is_empty());
        for rule in FleetReconcileRule::ALL {
            let mut g = FleetReconcileRules::default();
            assert!(g.disable(rule.as_str()), "{}", rule.as_str());
        }
    }

    #[test]
    fn spawn_timeout_kills_a_hung_child() {
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        let start = Instant::now();
        let out = spawn_timeout(cmd, Duration::from_millis(250));
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "rule bounded_waits: a hung child must not be waited on unbounded, elapsed={:?}",
            start.elapsed()
        );
        assert!(
            out.is_some(),
            "rule bounded_waits: timeout path must still return"
        );
    }

    #[test]
    fn spawn_timeout_child_does_not_inherit_our_file_fd() {
        use std::os::unix::io::AsRawFd;
        let dir = std::env::temp_dir().join(format!("fr-fd-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let held = dir.join("held");
        let guard = std::fs::File::create(&held).expect("held file");
        let fd = guard.as_raw_fd();
        // macOS `ls /dev/fd` lists extra dirents even for CLOEXEC fds (measured);
        // fstat/open of the raw fd is the real inherit check.
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", "exec 3<>/dev/fd/$CHECK_FD"])
            .env("CHECK_FD", fd.to_string());
        let out = spawn_timeout(cmd, Duration::from_secs(2)).expect("sh open-fd");
        assert!(
            !out.status.success(),
            "rule lock_not_inheritable: child opened our File fd {fd} (inherited, not CLOEXEC)"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crate_takes_no_run_lock_so_never_emits_unknown_holder() {
        let main = include_str!("main.rs");
        let token = format!("{}_{}", "holder", "pid");
        assert!(
            !main.contains(&token),
            "rule names_its_blocker: this observer takes no lock and must not emit holder_pid=unknown"
        );
    }

    #[test]
    fn crate_does_not_widen_admission() {
        let main = include_str!("main.rs");
        let token = format!("{}_{}", "ADMISSION", "FRESH");
        assert!(
            !main.contains(&token),
            "rule no_widened_admission: observers do not own the standing verdict window"
        );
    }

    #[test]
    fn crate_does_not_claim_liveness_from_a_capture() {
        let main = include_str!("main.rs");
        let live = format!("{}{}", "Working", " (");
        let worked = format!("{}{}", "Worked", " for");
        assert!(
            !main.contains(&live) && !main.contains(&worked),
            "rule no_single_capture_liveness: this crate never classifies pane liveness"
        );
    }

    #[test]
    fn empty_snapshot_is_named_fail_not_pass() {
        let v = reconcile_inner("alpha\n", "alpha:\n", "", &rules());
        assert_eq!(
            v.detector, "ntm_snapshot_unparseable",
            "rule anti_vacuity: an empty snapshot is a named FAIL, not a silent pass"
        );
        assert_eq!(v.verdict, "FAIL");
    }
}
