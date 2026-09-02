#![forbid(unsafe_code)]

//! Typed admission refusal. Port of `bin/admission_reason.py`.
//!
//! A completed fresh PASS prints NOTHING. An expired PASS is not a RED gate.
//! A published RED whose live gate PASSES is REPAIRED_BUT_UNPUBLISHED — that
//! distinction is opt-in (`--publication-check`) so default output stays
//! byte-faithful to the oracle (controller-tick cases on EXPIRED / COMPLETE /
//! NOT-ADMISSIBLE). dispatch-stall-profile.sh remains the chain profiler and
//! is not inlined.

use regex::Regex;
use serde::de::Deserialize;
use serde::Deserialize as DeserializeDerive;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use subprocess_contract::{bounded_output, BoundedOutcome};
use std::sync::OnceLock;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    NameTheFailingGate,
    ExpiredPassIsNotRed,
    IncompleteIsNotHealthy,
    RepairedButUnpublished,
}

impl Rule {
    pub const ALL: &'static [Rule] = &[
        Rule::NameTheFailingGate,
        Rule::ExpiredPassIsNotRed,
        Rule::IncompleteIsNotHealthy,
        Rule::RepairedButUnpublished,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Rule::NameTheFailingGate => "name_the_failing_gate",
            Rule::ExpiredPassIsNotRed => "expired_pass_is_not_red",
            Rule::IncompleteIsNotHealthy => "incomplete_is_not_healthy",
            Rule::RepairedButUnpublished => "repaired_but_unpublished",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.as_str() == name)
    }
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub name_the_failing_gate: bool,
    pub expired_pass_is_not_red: bool,
    pub incomplete_is_not_healthy: bool,
    pub repaired_but_unpublished: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            name_the_failing_gate: true,
            expired_pass_is_not_red: true,
            incomplete_is_not_healthy: true,
            repaired_but_unpublished: true,
        }
    }
}

impl Rules {
    pub fn disable(&mut self, name: &str) -> bool {
        let Some(rule) = Rule::parse(name) else {
            return false;
        };
        match rule {
            Rule::NameTheFailingGate => self.name_the_failing_gate = false,
            Rule::ExpiredPassIsNotRed => self.expired_pass_is_not_red = false,
            Rule::IncompleteIsNotHealthy => self.incomplete_is_not_healthy = false,
            Rule::RepairedButUnpublished => self.repaired_but_unpublished = false,
        }
        true
    }
}

#[derive(Debug, DeserializeDerive)]
struct Ledger {
    #[serde(default)]
    overall: Option<String>,
    #[serde(default)]
    completed_ts: Option<String>,
    #[serde(default)]
    unknown_until_ts: Option<String>,
    #[serde(default)]
    entries: Vec<Entry>,
}

#[derive(Debug, DeserializeDerive)]
struct Entry {
    #[serde(default)]
    gate: String,
    #[serde(default)]
    verdict: String,
    #[serde(default)]
    detail: String,
}
/// Errors encountered while loading a ledger from a path.
#[derive(Debug, PartialEq, Eq)]
pub enum LedgerError {
    Missing { path: PathBuf, reason: String },
    Unreadable { path: PathBuf, reason: String },
    Malformed { path: PathBuf, reason: String },
}

impl LedgerError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Missing { .. } => "ledger_missing",
            Self::Unreadable { .. } => "ledger_unreadable",
            Self::Malformed { .. } => "ledger_malformed",
        }
    }
}

impl std::fmt::Display for LedgerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (path, reason) = match self {
            Self::Missing { path, reason }
            | Self::Unreadable { path, reason }
            | Self::Malformed { path, reason } => (path, reason),
        };
        write!(formatter, "path='{}' reason={}", path.display(), reason)
    }
}

impl std::error::Error for LedgerError {}

fn code_detail_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| {
        Regex::new(
            r#""code"\s*:\s*"([A-Z]\d+)"(?:\s*,\s*"[a-z_]+"\s*:\s*"[^"]*")*?\s*,\s*"detail"\s*:\s*"([^"]{0,120})""#,
        )
        .expect("CODE_DETAIL")
    })
}

fn row_only_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r#""row"\s*:\s*"([^"]{1,60})""#).expect("ROW_ONLY"))
}

pub fn spawn_timeout(mut cmd: Command, timeout: Duration) -> BoundedOutcome {
    bounded_output(&mut cmd, timeout)
}

fn structured_reasons(detail: &str) -> Vec<String> {
    let bytes = detail.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            let mut de = serde_json::Deserializer::from_str(&detail[i..]);
            if let Ok(value) = Value::deserialize(&mut de) {
                if let Some(violations) = value.get("violations").and_then(|v| v.as_array()) {
                    let mut reasons = Vec::new();
                    for v in violations {
                        let Some(obj) = v.as_object() else { continue };
                        let Some(code) = obj
                            .get("code")
                            .and_then(|c| c.as_str())
                            .filter(|s| !s.is_empty())
                        else {
                            continue;
                        };
                        let mut fields = vec![code.to_string()];
                        if let Some(row) = obj
                            .get("row")
                            .and_then(|r| r.as_str())
                            .filter(|s| !s.is_empty())
                        {
                            fields.push(format!("row={row}"));
                        }
                        if let Some(msg) = obj
                            .get("detail")
                            .and_then(|d| d.as_str())
                            .filter(|s| !s.is_empty())
                        {
                            fields.push(msg.to_string());
                        }
                        reasons.push(fields.join(" "));
                    }
                    if !reasons.is_empty() {
                        return reasons;
                    }
                }
            }
        }
        i += 1;
    }
    Vec::new()
}

/// A path input that the environment could not name. Bead `control-plane-7ai`'s invariant:
/// resolve at runtime or return a typed error — never substitute a hardcoded home directory,
/// which silently probes another operator's machine layout.
#[derive(Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// `$HOME` is unset or empty, so `~/.local/bin` cannot be derived.
    HomeUnset,
}

impl ConfigError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::HomeUnset => "home_unset",
        }
    }
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::HomeUnset => write!(
                formatter,
                "$HOME is unset; cannot derive ~/{HOOKS_REGISTRY_CHECK_HOME_RELATIVE}"
            ),
        }
    }
}

const HOOKS_REGISTRY_CHECK_HOME_RELATIVE: &str = ".local/bin/hooks-registry-check";

/// Resolve a home directory from a candidate environment value. Pure with respect to the
/// process so it is unit-testable without mutating process-global env, which would poison
/// every sibling test in this binary.
fn resolve_home(value: Option<std::ffi::OsString>) -> Result<PathBuf, ConfigError> {
    value
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .ok_or(ConfigError::HomeUnset)
}

/// `$HOME`, or a typed error. Never a guessed literal.
fn home_dir() -> Result<PathBuf, ConfigError> {
    resolve_home(std::env::var_os("HOME"))
}

/// Locate the live closure probe: the `$HOME`-relative install if it exists, otherwise the
/// bare name resolved through `PATH`. When `$HOME` is unset the home-relative candidate is
/// skipped — the probe degrades to `PATH` rather than guessing a home directory.
fn resolve_hooks_registry_check(home: Result<PathBuf, ConfigError>) -> PathBuf {
    if let Ok(home) = home {
        let candidate = home.join(HOOKS_REGISTRY_CHECK_HOME_RELATIVE);
        if candidate.is_file() {
            return candidate;
        }
    }
    PathBuf::from("hooks-registry-check")
}

fn hooks_registry_check_path() -> PathBuf {
    resolve_hooks_registry_check(home_dir())
}

fn probe_live(gate: &str) -> Vec<String> {
    if gate != "domain-closure" {
        return Vec::new();
    }
    let path = hooks_registry_check_path();
    let mut cmd = Command::new(path);
    cmd.arg("--json");
    let out = match spawn_timeout(cmd, Duration::from_secs(60)) {
        BoundedOutcome::Completed(output) => output,
        BoundedOutcome::TimedOut => {
            eprintln!("admission-reason live hook probe timed out before its deadline");
            return Vec::new();
        }
        BoundedOutcome::Unspawned(error) => {
            eprintln!("admission-reason live hook probe could not spawn: {error}");
            return Vec::new();
        }
    };
    let Ok(data) = serde_json::from_slice::<Value>(&out.stdout) else {
        return Vec::new();
    };
    let mut reasons = Vec::new();
    if let Some(vs) = data.get("violations").and_then(|v| v.as_array()) {
        for v in vs {
            let code = v.get("code").and_then(|c| c.as_str()).unwrap_or("?");
            let row = v.get("row").and_then(|r| r.as_str()).unwrap_or("");
            let detail = v.get("detail").and_then(|d| d.as_str()).unwrap_or("");
            let clipped: String = detail.chars().take(110).collect();
            reasons.push(format!("{code} {row} {clipped}").trim().to_string());
        }
    }
    reasons
}

fn reasons_for(detail: &str, gate: &str) -> Vec<String> {
    let out = structured_reasons(detail);
    if !out.is_empty() {
        return out;
    }
    let out: Vec<String> = code_detail_re()
        .captures_iter(detail)
        .map(|c| format!("{} {}", &c[1], &c[2]))
        .collect();
    if !out.is_empty() {
        return out;
    }
    let live = probe_live(gate);
    if !live.is_empty() {
        return live;
    }
    let out: Vec<String> = row_only_re()
        .captures_iter(detail)
        .map(|c| format!("row={}", &c[1]))
        .collect();
    if !out.is_empty() {
        return out;
    }
    let collapsed = regex::Regex::new(r"\s+")
        .ok()
        .map(|r| r.replace_all(detail, " ").into_owned())
        .unwrap_or_else(|| detail.to_string());
    let take: String = collapsed.chars().take(140).collect();
    vec![if take.is_empty() {
        "(no detail recorded)".into()
    } else {
        take
    }]
}

fn parse_utc_epoch(stamp: &str) -> Option<i64> {
    // YYYY-MM-DDTHH:MM:SSZ
    if stamp.len() < 20 || !stamp.ends_with('Z') {
        return None;
    }
    let y: i64 = stamp.get(0..4)?.parse().ok()?;
    let mo: i64 = stamp.get(5..7)?.parse().ok()?;
    let d: i64 = stamp.get(8..10)?.parse().ok()?;
    let h: i64 = stamp.get(11..13)?.parse().ok()?;
    let mi: i64 = stamp.get(14..16)?.parse().ok()?;
    let s: i64 = stamp.get(17..19)?.parse().ok()?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    // days since Unix epoch via civil from days (Howard Hinnant)
    let y = if mo <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if mo > 2 { mo - 3 } else { mo + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    let days = era * 146_097 + doe as i64 - 719_468;
    Some(days * 86400 + h * 3600 + mi * 60 + s)
}

fn now_epoch() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn fresh_seconds() -> f64 {
    std::env::var("ADMISSION_FRESH_SECONDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1500.0)
}

fn standing_verdict_reason(data: &Ledger, rules: &Rules) -> Option<String> {
    if data.overall.is_none()
        && !data.entries.is_empty()
        && data.entries.iter().all(|e| e.verdict == "PASS")
    {
        if rules.incomplete_is_not_healthy {
            return Some(
                "  check.sh did not COMPLETE — no terminal verdict; entries are partial".into(),
            );
        }
        return None;
    }
    if data.overall.as_deref() != Some("PASS") {
        return None;
    }
    let Some(stamp) = data.completed_ts.as_deref() else {
        if rules.incomplete_is_not_healthy {
            return Some(
                "  check.sh did not COMPLETE — no terminal verdict; entries are partial".into(),
            );
        }
        return None;
    };
    let Some(when) = parse_utc_epoch(stamp) else {
        return Some("  check.sh NOT-ADMISSIBLE — PASS has an invalid completed_ts".into());
    };
    let age = (now_epoch() - when) as f64;
    let limit = fresh_seconds();
    if age > limit {
        if rules.expired_pass_is_not_red {
            return Some(format!(
                "  check.sh EXPIRED — standing PASS age={}s > window={}s; re-run required",
                age as i64, limit as i64
            ));
        }
        return Some("  check.sh did not PASS".into());
    }
    if age < 0.0 {
        return Some(
            "  check.sh NOT-ADMISSIBLE — standing PASS completed_ts is in the future".into(),
        );
    }
    None
}

fn live_override(gate: &str) -> Option<String> {
    let raw = std::env::var("ADMISSION_LIVE_OVERRIDE").ok()?;
    for part in raw.split(',') {
        if let Some((g, v)) = part.split_once(':') {
            if g == gate {
                return Some(v.to_string());
            }
        }
    }
    None
}

fn age_seconds(data: &Ledger) -> Option<f64> {
    let stamp = data.completed_ts.as_deref()?;
    let when = parse_utc_epoch(stamp)?;
    Some((now_epoch() - when) as f64)
}

/// Load and explain a ledger from a path.
///
/// Unlike the compatibility wrapper explain, this API preserves missing, unreadable, and
/// malformed ledger input as typed errors for callers that must fail closed.
pub fn explain_checked(
    path: &Path,
    publication_check: bool,
    rules: &Rules,
) -> Result<String, LedgerError> {
    let text = fs::read_to_string(path).map_err(|error| {
        let reason = error.to_string();
        if error.kind() == std::io::ErrorKind::NotFound {
            LedgerError::Missing {
                path: path.to_path_buf(),
                reason,
            }
        } else {
            LedgerError::Unreadable {
                path: path.to_path_buf(),
                reason,
            }
        }
    })?;
    let data: Ledger = serde_json::from_str(&text).map_err(|error| LedgerError::Malformed {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    if data.entries.is_empty() {
        return Err(LedgerError::Malformed {
            path: path.to_path_buf(),
            reason: "ledger contains no entries".to_owned(),
        });
    }
    Ok(explain_data(&data, publication_check, rules))
}

fn explain_data(data: &Ledger, publication_check: bool, rules: &Rules) -> String {
    let mut out = String::new();
    if let Some(s) = standing_verdict_reason(data, rules) {
        out.push_str(&s);
        out.push('\n');
    }
    if data.overall.as_deref() == Some("UNKNOWN") {
        let until = data.unknown_until_ts.as_deref().unwrap_or("unspecified");
        out.push_str(&format!(
            "  check.sh          UNKNOWN standing verdict; recheck horizon until {until}\n"
        ));
    }
    if rules.name_the_failing_gate {
        let bad: Vec<&Entry> = data
            .entries
            .iter()
            .filter(|e| e.verdict != "PASS")
            .collect();
        for entry in &bad {
            let rs = reasons_for(&entry.detail, &entry.gate);
            let first = rs
                .first()
                .map(|s| s.as_str())
                .unwrap_or("(no detail recorded)");
            out.push_str(&format!(
                "  {:<16} {:<5} {}\n",
                entry.gate, entry.verdict, first
            ));
            for extra in rs.iter().skip(1).take(3) {
                out.push_str(&format!("  {:<16} {:<5} {}\n", "", "", extra));
            }
        }
    }

    if publication_check && rules.repaired_but_unpublished {
        if let Some(red) = data.entries.iter().find(|e| e.verdict == "RED") {
            let live = live_override(&red.gate);
            let stale = age_seconds(data)
                .map(|a| a > fresh_seconds())
                .unwrap_or(false);
            if live.as_deref() == Some("PASS") && stale {
                out.push_str(&format!(
                    "  REPAIRED_BUT_UNPUBLISHED — published names gate '{}' but that gate PASSES live; publication is the failure\n",
                    red.gate
                ));
            }
        }
    }
    out
}

/// Top-level explain, retaining the historical string-returning API.
pub fn explain(path: &Path, publication_check: bool, rules: &Rules) -> String {
    match explain_checked(path, publication_check, rules) {
        Ok(output) => output,
        Err(LedgerError::Missing { path, .. }) => format!(
            "  (no check.sh ledger at '{}' — run check.sh with CHECK_SH_LEDGER set to see the reason)\n",
            path.display()
        ),
        Err(LedgerError::Unreadable { reason, .. } | LedgerError::Malformed { reason, .. }) => {
            format!("  (ledger unreadable: {reason})\n")
        }
    }
}

/// The SAME explanation, from ledger TEXT rather than a path.
///
/// This exists because the caller that most needs it — `controller-tick`'s `standing_pass` — has
/// already read the ledger it is refusing on. Re-reading by path would let the emitter explain a
/// DIFFERENT file's contents than the one that produced the refusal, and this repo has two files
/// named `check-sh-ledger.json` (the tick's `~/.local/state/flywheel` copy and `loop-tick`'s
/// in-repo copy) that routinely hold different verdicts. Passing the text makes explaining the
/// wrong subject inexpressible.
///
/// Both path and text explanations share one rendering implementation;
/// the differential oracle in tests/differential.rs keeps covering both.
pub fn explain_text(text: &str, publication_check: bool, rules: &Rules) -> String {
    let data: Ledger = match serde_json::from_str(text) {
        Ok(d) => d,
        Err(e) => return format!("  (ledger unreadable: {e})\n"),
    };
    explain_data(&data, publication_check, rules)
}
/// ONE-LINE refusal detail for a ledger ROW, from the ledger text the refuser actually read.
///
/// MEASURED 2026-08-27 (bead cp-jsgiu): `controller-tick` emitted 62 `admission_refused` rows
/// carrying `reason=verdict_not_pass` and NO `detail` field at all, while the very ledger it had
/// just parsed said `docs-staleness RED file=.flywheel/GOAL.md commits_since_last_touch=56
/// limit=50`. The loop recorded THAT it stopped and never WHY, for over an hour, and diagnosing it
/// required a human to know a second ledger existed at a different path. A refusal that cannot be
/// diagnosed from its own row is a stopped clock with a timestamp.
///
/// This is a repeat of a known class: two prior truncation bugs (955c974, 834257b) clipped this
/// same field mid-word. An ABSENT reason is worse than a clipped one — a clipped reason is visibly
/// clipped, an absent one reads as "no further information exists".
///
/// QUIET ON THE HEALTHY PATH. Returns `None` when the standing verdict is a fresh, complete PASS
/// with no failing gate, so a caller may emit this unconditionally: it speaks only when the verdict
/// is actionable. A monitor that nags on PASS gets disabled and then protects nothing.
///
/// The lines come from `explain_text`, so the row and the operator-facing `admission-reason.sh`
/// output cannot drift into two vocabularies.
/// THE CASCADE IS DROPPED, AND THAT IS NOT TRUNCATION.
///
/// `check.sh` is fail-fast: the first RED gate aborts the chain and every later gate is emitted
/// `UNRUN skipped-after-<first>`, which its own source (`bin/check.sh:323`) calls *"bulk
/// decoration"*. Carrying those into the row costs ~700 characters and buries the one line that
/// names the cause -- measured on a real 2026-08-26 ledger: one `docs-staleness RED` line followed
/// by twelve identical `skipped-after-docs-staleness` entries. Burying the actionable line is the
/// same failure as clipping it, so the derived-from-the-cascade rows are dropped rather than
/// summarised, and the row still ends with the count so nothing is silently disappeared.
///
/// `MAX` is a LAST-RESORT bound on a pathological gate detail, not the normal path: prior bugs
/// (955c974, 834257b) clipped mid-word, so when it does bite the row says `[truncated]` rather
/// than ending mid-sentence and reading as complete.
pub fn refusal_detail(text: &str, rules: &Rules) -> Option<String> {
    const MAX: usize = 600;
    let explained = explain_text(text, false, rules);
    let all: Vec<&str> = explained
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    let kept: Vec<&str> = all
        .iter()
        .copied()
        .filter(|l| !l.contains("skipped-after-"))
        .collect();
    let dropped = all.len() - kept.len();
    // If EVERY line was cascade decoration, keep them rather than going mute: silence would be a
    // worse answer than noise, and this branch is why "filter it out" cannot become "say nothing".
    let mut joined = if kept.is_empty() { all } else { kept }.join("; ");
    if dropped > 0 {
        joined.push_str(&format!(
            " (+{dropped} gate(s) UNRUN, skipped after the above)"
        ));
    }
    if joined.chars().count() > MAX {
        joined = joined.chars().take(MAX).collect::<String>() + " …[truncated]";
    }
    (!joined.is_empty()).then_some(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bead's row-1 acceptance, at the unit boundary: a PLANTED failing gate must produce a
    /// non-empty detail that NAMES the gate. A refusal with an empty detail fails here.
    #[test]
    fn planted_red_gate_yields_a_named_nonempty_detail() {
        let planted = r#"{"schema":"control-plane.check.v1","overall":"FAIL","completed_ts":"2026-08-27T20:20:54Z","entries":[{"gate":"docs-staleness","verdict":"RED","detail":"docs-staleness RED file=.flywheel/GOAL.md commits_since_last_touch=56 limit=50"}]}"#;
        let detail = refusal_detail(planted, &Rules::default())
            .expect("a planted RED gate MUST produce a detail");
        assert!(!detail.trim().is_empty(), "detail was empty: {detail:?}");
        assert!(
            detail.contains("docs-staleness"),
            "detail must NAME the failing gate, got {detail:?}"
        );
        assert!(
            detail.contains("GOAL.md") && detail.contains("commits_since_last_touch=56"),
            "detail must carry the gate's own reason, got {detail:?}"
        );
    }

    /// INVERSE LEG: a fresh complete PASS is SILENT. Without this, the fix would nag every healthy
    /// tick and be disabled, protecting nothing.
    #[test]
    fn fresh_complete_pass_is_silent() {
        let now = now_epoch();
        let stamp = epoch_to_stamp(now - 10);
        let healthy = format!(
            r#"{{"schema":"control-plane.check.v1","overall":"PASS","completed_ts":"{stamp}","entries":[{{"gate":"docs-staleness","verdict":"PASS","detail":"fine"}},{{"gate":"tests","verdict":"PASS","detail":"ok"}}]}}"#
        );
        assert_eq!(
            refusal_detail(&healthy, &Rules::default()),
            None,
            "the healthy path must stay QUIET"
        );
    }

    /// An UNRUN gate is not a RED gate, but it is still not a PASS — and a refusal on it must say
    /// so rather than defaulting to silence.
    #[test]
    fn unrun_gate_is_named_not_silent() {
        let text = r#"{"schema":"control-plane.check.v1","overall":"FAIL","completed_ts":"2026-08-27T20:20:54Z","entries":[{"gate":"drift","verdict":"UNRUN","detail":"no-invocation-contract"}]}"#;
        let detail = refusal_detail(text, &Rules::default()).expect("UNRUN must be named");
        assert!(detail.contains("drift"), "got {detail:?}");
    }

    /// The fail-fast cascade must not bury the cause. Measured on a real 2026-08-26 ledger: one
    /// `docs-staleness RED` line followed by twelve `skipped-after-docs-staleness` entries, ~900
    /// characters in which the actionable line was the first 15%.
    #[test]
    fn the_cascade_is_dropped_but_counted() {
        let mut entries = vec![r#"{"gate":"docs-staleness","verdict":"RED","detail":"docs-staleness RED file=CLAUDE.md commits_since_last_touch=57 limit=50"}"#.to_string()];
        // Keep the cascade detail in the structured form consumed by structured_reasons.
        // This makes the fixture deterministic: domain-closure otherwise invokes its live
        // probe before the plain-text fallback, so an installed probe can hide one cascade row.
        let cascade_detail =
            serde_json::to_string(r#"{"violations":[{"code":"skipped-after-docs-staleness"}]}"#)
                .expect("cascade detail must serialize");
        for g in [
            "domain-closure",
            "close-evidence",
            "session-repo",
            "tests",
            "mutation",
        ] {
            entries.push(format!(
                r#"{{"gate":"{g}","verdict":"UNRUN","detail":{cascade_detail}}}"#
            ));
        }
        let text = format!(
            r#"{{"schema":"control-plane.check.v1","overall":"FAIL","completed_ts":"2026-08-26T08:05:09Z","entries":[{}]}}"#,
            entries.join(",")
        );
        let d = refusal_detail(&text, &Rules::default()).expect("a RED gate must speak");
        assert!(d.contains("docs-staleness"), "{d:?}");
        assert!(d.contains("CLAUDE.md"), "the cause must survive: {d:?}");
        assert!(
            !d.contains("skipped-after-"),
            "cascade decoration must be dropped: {d:?}"
        );
        assert!(
            d.contains("+5 gate(s) UNRUN"),
            "the dropped rows must still be COUNTED, never silently disappeared: {d:?}"
        );
    }

    /// If the cascade were ALL there is, dropping it must not make the row mute. Silence is a worse
    /// answer than noise, and this is the leg that stops the filter from becoming the defect.
    #[test]
    fn an_all_cascade_ledger_still_speaks() {
        let text = r#"{"schema":"control-plane.check.v1","overall":"FAIL","completed_ts":"2026-08-26T08:05:09Z","entries":[{"gate":"tests","verdict":"UNRUN","detail":"skipped-after-publication-deadline"}]}"#;
        let d = refusal_detail(text, &Rules::default()).expect("must not go mute");
        assert!(d.contains("tests"), "{d:?}");
    }

    /// A single gate CANNOT breach the bound: `reasons_for` already clips one gate's detail to 140
    /// characters. Measured while writing this test -- a 9000-char detail came back at 154 and the
    /// `[truncated]` marker never fired. Pinning that here so the next reader does not "fix" the
    /// bound to chase a case the code already handles upstream.
    #[test]
    fn one_gate_cannot_breach_the_bound_because_reasons_for_already_clips() {
        let huge = "x".repeat(9000);
        let text = format!(
            r#"{{"schema":"control-plane.check.v1","overall":"FAIL","completed_ts":"2026-08-26T08:05:09Z","entries":[{{"gate":"tests","verdict":"RED","detail":"{huge}"}}]}}"#
        );
        let d = refusal_detail(&text, &Rules::default()).expect("must speak");
        assert!(d.chars().count() < 300, "got {} chars", d.chars().count());
        assert!(!d.contains("[truncated]"), "no bound needed here: {d:?}");
    }

    /// MANY failing gates IS what breaches the bound, and when it does the row must ANNOUNCE the
    /// clip rather than ending mid-word and reading as complete -- the failure mode of 955c974 and
    /// 834257b, which is the whole reason this bead exists.
    #[test]
    fn many_failing_gates_are_bounded_and_the_clip_is_announced() {
        let entries: Vec<String> = (0..40)
            .map(|i| {
                format!(
                    r#"{{"gate":"gate-{i:02}","verdict":"RED","detail":"gate-{i:02} RED some genuinely distinct reason text for gate number {i:02}"}}"#
                )
            })
            .collect();
        let text = format!(
            r#"{{"schema":"control-plane.check.v1","overall":"FAIL","completed_ts":"2026-08-26T08:05:09Z","entries":[{}]}}"#,
            entries.join(",")
        );
        let d = refusal_detail(&text, &Rules::default()).expect("must speak");
        assert!(
            d.chars().count() <= 640,
            "unbounded: {} chars",
            d.chars().count()
        );
        assert!(
            d.contains("[truncated]"),
            "a clipped detail must ANNOUNCE that it was clipped: {d:?}"
        );
        assert!(
            d.contains("gate-00"),
            "the FIRST failing gate must survive the clip -- it is the cause: {d:?}"
        );
    }

    /// An unparseable ledger is a refusal too, and must not come back silent.
    #[test]
    fn unparseable_ledger_is_not_silent() {
        assert!(refusal_detail("not json at all", &Rules::default()).is_some());
        assert!(refusal_detail("", &Rules::default()).is_some());
    }
    #[test]
    fn path_loading_reports_typed_errors_with_path_and_reason() {
        let dir = std::env::temp_dir().join(format!("ar-ledger-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let missing = dir.join("missing.json");
        let error = explain_checked(&missing, false, &Rules::default())
            .expect_err("missing ledger must fail");
        assert_eq!(error.code(), "ledger_missing");
        assert!(error.to_string().contains(&missing.display().to_string()));
        assert!(
            error.to_string().contains("No such file") || error.to_string().contains("not found")
        );

        let malformed = dir.join("malformed.json");
        std::fs::write(&malformed, "not json").expect("write malformed ledger");
        let error = explain_checked(&malformed, false, &Rules::default())
            .expect_err("malformed ledger must fail");
        assert_eq!(error.code(), "ledger_malformed");
        assert!(error.to_string().contains(&malformed.display().to_string()));
        assert!(error.to_string().contains("expected") || error.to_string().contains("key"));

        let empty = dir.join("empty.json");
        std::fs::write(&empty, "{}").expect("write empty ledger");
        let error =
            explain_checked(&empty, false, &Rules::default()).expect_err("empty ledger must fail");
        assert_eq!(error.code(), "ledger_malformed");
        assert!(error.to_string().contains("no entries"));

        let unreadable = dir.join("directory");
        std::fs::create_dir_all(&unreadable).expect("create unreadable ledger path");
        let error = explain_checked(&unreadable, false, &Rules::default())
            .expect_err("directory ledger must fail");
        assert_eq!(error.code(), "ledger_unreadable");
        match error {
            LedgerError::Unreadable { path, reason } => {
                assert_eq!(path, unreadable);
                assert!(!reason.is_empty());
            }
            other => panic!("expected unreadable ledger error, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    fn epoch_to_stamp(epoch: i64) -> String {
        // Inverse of parse_utc_epoch, enough for a test stamp.
        let days = epoch.div_euclid(86400);
        let secs = epoch.rem_euclid(86400);
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        let y = if m <= 2 { y + 1 } else { y };
        format!(
            "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
            secs / 3600,
            (secs % 3600) / 60,
            secs % 60
        )
    }

    #[test]
    fn epoch_stamp_roundtrips() {
        for e in [1577836800_i64, 1756326054, now_epoch()] {
            assert_eq!(parse_utc_epoch(&epoch_to_stamp(e)), Some(e), "epoch {e}");
        }
    }

    #[test]
    fn every_named_rule_is_disableable() {
        for rule in Rule::ALL {
            let mut g = Rules::default();
            assert!(g.disable(rule.as_str()), "{}", rule.as_str());
        }
    }

    #[test]
    fn parse_utc_roundtrip_known_stamp() {
        let e = parse_utc_epoch("2020-01-01T00:00:00Z").unwrap();
        assert_eq!(e, 1577836800);
    }

    #[test]
    fn spawn_timeout_kills_a_hung_child() {
        let mut cmd = Command::new("sleep");
        cmd.arg("30");
        let start = Instant::now();
        let out = spawn_timeout(cmd, Duration::from_millis(250));
        assert!(
            start.elapsed() < Duration::from_secs(3),
            "rule bounded_waits"
        );
        assert!(matches!(out, BoundedOutcome::TimedOut));
    }

    #[test]
    fn spawn_timeout_child_does_not_inherit_our_file_fd() {
        use std::os::unix::io::AsRawFd;
        let dir = std::env::temp_dir().join(format!("ar-fd-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let held = dir.join("held");
        let guard = std::fs::File::create(&held).expect("held");
        let fd = guard.as_raw_fd();
        let mut cmd = Command::new("/bin/sh");
        cmd.args(["-c", "exec 3<>/dev/fd/$CHECK_FD"])
            .env("CHECK_FD", fd.to_string());
        match spawn_timeout(cmd, Duration::from_secs(2)) {
            BoundedOutcome::Completed(output) => {
                assert!(!output.status.success(), "rule lock_not_inheritable");
            }
            BoundedOutcome::TimedOut => panic!("fd probe must complete before its deadline"),
            BoundedOutcome::Unspawned(error) => panic!("fd probe must spawn: {error}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn crate_does_not_widen_admission() {
        let lib = include_str!("lib.rs");
        let default = format!("unwrap_or({}.0)", 1500);
        let widened = format!("unwrap_or({})", 3600);
        assert!(
            lib.contains(&default),
            "rule no_widened_admission: default window stays 1500"
        );
        assert!(
            !lib.contains(&widened),
            "rule no_widened_admission: must not default to 3600"
        );
    }

    #[test]
    fn structured_reasons_tolerates_key_order() {
        let detail = r#"doctor: {"violations":[{"detail":"missing harm class: \"irreversible\"","row":"gate-x","code":"E010"}]}"#;
        let rs = structured_reasons(detail);
        assert!(rs.iter().any(|s| s.contains("E010")), "{rs:?}");
        assert!(rs.iter().any(|s| s.contains("row=gate-x")), "{rs:?}");
    }

    /// KNOWN-BAD: an unset or empty `$HOME` must not resolve to a hardcoded home directory.
    /// The pre-extraction source read `"/Users/<user>"` here, so on any other machine the
    /// probe silently pointed at a path that does not exist. Bead `control-plane-7ai` forbids
    /// the literal; this is the leg that fires if it comes back.
    #[test]
    fn unresolvable_home_degrades_to_path_not_a_literal() {
        assert_eq!(resolve_home(None), Err(ConfigError::HomeUnset));
        assert_eq!(
            resolve_home(Some(std::ffi::OsString::from(""))),
            Err(ConfigError::HomeUnset)
        );
        let bare = PathBuf::from("hooks-registry-check");
        assert_eq!(
            resolve_hooks_registry_check(Err(ConfigError::HomeUnset)),
            bare
        );
        assert_eq!(
            resolve_hooks_registry_check(Ok(PathBuf::from("/nonexistent-home-for-tests"))),
            bare,
            "a home without the install must fall through to PATH, never a guessed literal"
        );
        assert_eq!(ConfigError::HomeUnset.code(), "home_unset");
    }
}
