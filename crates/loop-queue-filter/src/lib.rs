#![forbid(unsafe_code)]

//! The dispatch selector's pure policy, ported from `bin/loop-queue-filter.py`.
//!
//! The Python file remains the differential oracle.  This module deliberately keeps the
//! selector's lexical rules and ordering visible: changing one rule must change a named test.

use regex::Regex;
use serde_json::{Map, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const HARVEST_CLASSES: &[&str] = &["DOCTRINE", "CONFORMANCE", "RECON"];
const GATE_MARKERS: &[&str] = &[
    "[DECISION]",
    "APPROVAL GATE",
    "JOSHUA MUST",
    "REQUIRES JOSHUA",
];
const REAL_BLOCKERS: &[&str] = &[
    "SPEND",
    "PURCHASE",
    "BUY ",
    "INVOICE",
    "BILLING",
    "SUBSCRIPTION",
    "PAID PLAN",
    "API KEY",
    "API CREDENTIAL",
    "CREDENTIAL",
    "SECRET KEY",
    "OAUTH TOKEN",
    "ACCESS TOKEN",
    "LOGIN",
    "SIGN IN",
    "DEPLOY",
    "PUBLISH",
    "RELEASE TO",
    "PRODUCTION",
    "CLIENT-FACING",
    "TESTER INVITE",
    "APP STORE",
    "UPSTREAM MUTATION",
    "THIRD-PARTY REPO",
];
const RESOURCE_BLOCKED: &[&str] = &[
    "BLOCKED ON DISK",
    "MUST BE FREED FIRST",
    "PROVISION BLOCKED",
    "REFUSES, CORRECTLY",
    "BLOCKED ON HEADROOM",
    "CANNOT BE PROVISIONED",
    "BELOW THE 20% FLOOR",
    "ABORTED AT FILE",
];
const UNBLOCKER_MARKERS: &[&str] = &[
    "ATTRIBUTE",
    "ATTRIBUTION",
    "RECLAIM",
    "FREE UP",
    "INVESTIGATE",
    "AUDIT",
    "MEASURE",
];

#[derive(Clone, Debug)]
pub struct Runtime {
    pub env: BTreeMap<String, String>,
    pub cwd: PathBuf,
    pub now: f64,
}

impl Runtime {
    pub fn from_process() -> Self {
        Self {
            env: env::vars().collect(),
            cwd: env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            now: unix_now(),
        }
    }

    fn get(&self, key: &str) -> Option<&str> {
        self.env.get(key).map(String::as_str)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct RunOutput {
    pub stdout: String,
    pub stderr: String,
    pub code: i32,
}

#[derive(Debug)]
struct Config {
    want: usize,
    harvest_class: Option<String>,
    harvest_exclude: bool,
    count_only: bool,
    epic: String,
    cooldown_sec: f64,
    cooldown_file: PathBuf,
    cooldown_commit: bool,
    cooldown_expect_id: Option<String>,
    repo_dir: PathBuf,
    now: f64,
}

fn unix_now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0)
}

fn env_int(runtime: &Runtime, key: &str, default: i64) -> Result<i64, String> {
    match runtime.get(key) {
        None => Ok(default),
        Some(value) => value
            .parse::<i64>()
            .map_err(|_| format!("QUEUE_CONFIG_ERROR\t{key} must be an integer")),
    }
}

fn parse_config(args: &[String], runtime: &Runtime) -> Result<Config, RunOutput> {
    let count_only = args.iter().any(|a| a == "--count");
    let filtered: Vec<&String> = args.iter().filter(|a| a.as_str() != "--count").collect();
    let epic = filtered.first().map(|s| s.to_string()).unwrap_or_default();

    let want = match env_int(runtime, "QUEUE_WANT", 2) {
        Ok(v) if (1..=20).contains(&v) => v as usize,
        Ok(_) => {
            return Err(RunOutput {
                stdout: String::new(),
                stderr: "QUEUE_CONFIG_ERROR\tQUEUE_WANT must be between 1 and 20\n".to_string(),
                code: 2,
            })
        }
        Err(message) => {
            return Err(RunOutput {
                stdout: String::new(),
                stderr: format!("{message}\n"),
                code: 2,
            })
        }
    };

    let harvest_class = runtime.get("HARVEST_CLASS").map(str::to_uppercase);
    if let Some(class) = &harvest_class {
        if !class.is_empty() && !HARVEST_CLASSES.contains(&class.as_str()) {
            return Err(RunOutput {
                stdout: String::new(),
                stderr: format!("QUEUE_CONFIG_ERROR\tunknown HARVEST_CLASS={class}\n"),
                code: 2,
            });
        }
    }

    let cooldown_sec = runtime
        .get("QUEUE_COOLDOWN_SEC")
        .unwrap_or("5400")
        .parse::<f64>()
        .unwrap_or(5400.0);
    let cooldown_file = runtime
        .get("QUEUE_COOLDOWN_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(default_cooldown_file);
    let repo_dir = runtime
        .get("REPO_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| runtime.cwd.clone());

    Ok(Config {
        want,
        harvest_class: harvest_class.filter(|s| !s.is_empty()),
        harvest_exclude: runtime.get("HARVEST_EXCLUDE") == Some("1"),
        count_only,
        epic,
        cooldown_sec,
        cooldown_file,
        cooldown_commit: runtime.get("QUEUE_COOLDOWN_COMMIT") == Some("1"),
        cooldown_expect_id: runtime
            .get("QUEUE_COOLDOWN_EXPECT_ID")
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned),
        repo_dir,
        now: runtime.now,
    })
}

fn default_cooldown_file() -> PathBuf {
    env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".local/state/flywheel/queue-cooldown.json")
}

fn string_field(row: &Map<String, Value>, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn desc_prefix(row: &Map<String, Value>) -> String {
    string_field(row, "description").chars().take(900).collect()
}

fn has_any(haystack: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| haystack.contains(needle))
}

/// True only for a marker tied to one of the three human blockers.
pub fn is_gated(row: &Map<String, Value>) -> bool {
    let blob = format!("{} {}", string_field(row, "title"), desc_prefix(row)).to_uppercase();
    has_any(&blob, GATE_MARKERS) && has_any(&blob, REAL_BLOCKERS)
}

/// True when this bead itself cannot proceed because a scarce resource is absent.
pub fn is_resource_blocked(row: &Map<String, Value>) -> bool {
    let title = string_field(row, "title").to_uppercase();
    if has_any(&title, UNBLOCKER_MARKERS) {
        return false;
    }
    let blob = format!("{} {}", title, desc_prefix(row).to_uppercase());
    has_any(&blob, RESOURCE_BLOCKED)
}

fn is_false_gate(row: &Map<String, Value>) -> bool {
    let blob = format!("{} {}", string_field(row, "title"), desc_prefix(row)).to_uppercase();
    has_any(&blob, GATE_MARKERS) && !has_any(&blob, REAL_BLOCKERS)
}

fn milestone_regex() -> Regex {
    Regex::new(r"\*{0,2}\b([MWP]\d{1,2})\b\*{0,2}").expect("milestone regex is valid")
}

fn charter_milestones(repo_dir: &Path) -> BTreeSet<String> {
    let token_re = milestone_regex();
    for relative in ["CHARTER.md", ".flywheel/CHARTER.md"] {
        let path = repo_dir.join(relative);
        if !path.exists() {
            continue;
        }
        let contents = match fs::read_to_string(&path) {
            Ok(value) => value,
            Err(_) => return BTreeSet::new(),
        };
        let mut tokens = BTreeSet::new();
        for line in contents.lines() {
            let trimmed = line.trim_start();
            let is_row = trimmed.starts_with('|')
                || (trimmed.len() >= 2
                    && matches!(trimmed.as_bytes()[0], b'-' | b'*')
                    && trimmed.as_bytes()[1].is_ascii_whitespace());
            if !is_row {
                continue;
            }
            let opening: String = line.chars().take(60).collect();
            if let Some(caps) = token_re.captures(&opening) {
                if let Some(token) = caps.get(1) {
                    tokens.insert(token.as_str().to_uppercase());
                }
            }
        }
        if !tokens.is_empty() {
            return tokens;
        }
    }
    BTreeSet::new()
}

fn milestone_rank(row: &Map<String, Value>, milestones: &BTreeSet<String>) -> u8 {
    if milestones.is_empty() {
        return 1;
    }
    let title = string_field(row, "title").to_uppercase();
    let token_re = milestone_regex();
    let reference_re = Regex::new(
        r"\b(?:BLOCKS|UNBLOCKS|BLOCKED BY|FOR|TOWARD|TOWARDS|PART OF|SEE|PER|UNDER|AFTER|BEFORE|PREREQ(?:UISITE)? FOR|DEPENDS ON)\s+$",
    )
    .expect("reference regex is valid");
    for caps in token_re.captures_iter(&title) {
        let Some(token) = caps.get(1) else { continue };
        if !milestones.contains(token.as_str()) {
            continue;
        }
        let before = &title[..token.start()];
        let suffix: String = before
            .chars()
            .rev()
            .take(24)
            .collect::<String>()
            .chars()
            .rev()
            .collect();
        if reference_re.is_match(&suffix) {
            continue;
        }
        return 0;
    }
    1
}

fn harvest_class(row: &Map<String, Value>) -> String {
    let title = string_field(row, "title");
    let Some(rest) = title.strip_prefix("harvest[") else {
        return String::new();
    };
    let Some((class, _)) = rest.split_once("]:") else {
        return String::new();
    };
    class.to_string()
}

fn row_id(row: &Map<String, Value>) -> String {
    string_field(row, "id")
}

fn priority_key(row: &Map<String, Value>) -> String {
    match row.get("priority") {
        None => "9".to_string(),
        Some(Value::Null) => "None".to_string(),
        Some(Value::String(value)) => value.clone(),
        Some(value) => value.to_string(),
    }
}

fn title_line(row: &Map<String, Value>) -> String {
    let title: String = string_field(row, "title").chars().take(90).collect();
    format!("{}\t{}\n", row_id(row), title)
}

fn load_cooldown(path: &Path) -> BTreeMap<String, f64> {
    let Ok(contents) = fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let Ok(Value::Object(object)) = serde_json::from_str::<Value>(&contents) else {
        return BTreeMap::new();
    };
    object
        .into_iter()
        .filter_map(|(key, value)| value.as_f64().map(|number| (key, number)))
        .collect()
}

fn save_cooldown(path: &Path, state: &BTreeMap<String, f64>) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    if fs::create_dir_all(parent).is_err() {
        return false;
    }
    let object = state
        .iter()
        .map(|(key, value)| (key.clone(), Value::from(*value)))
        .collect::<Map<String, Value>>();
    fs::write(path, Value::Object(object).to_string()).is_ok()
}

fn rows_from_input(data: Value) -> Vec<Map<String, Value>> {
    let values = match data {
        Value::Array(values) => values,
        Value::Object(object) => object
            .get("issues")
            .or_else(|| object.get("beads"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    values
        .into_iter()
        .filter_map(|value| value.as_object().cloned())
        .collect()
}

/// Execute one selector invocation.  Invalid JSON intentionally returns the Python oracle's
/// quiet success, while configuration errors are visible on stderr with their original codes.
pub fn run(input: &str, args: &[String], runtime: &Runtime) -> RunOutput {
    let config = match parse_config(args, runtime) {
        Ok(config) => config,
        Err(output) => return output,
    };
    let Ok(data) = serde_json::from_str::<Value>(input) else {
        return RunOutput {
            stdout: String::new(),
            stderr: String::new(),
            code: 0,
        };
    };
    let mut open_rows: Vec<Map<String, Value>> = rows_from_input(data)
        .into_iter()
        .filter(|row| string_field(row, "status") == "open")
        .filter(|row| string_field(row, "issue_type") != "epic")
        .collect();

    if let Some(class) = &config.harvest_class {
        open_rows.retain(|row| harvest_class(row) == *class);
    } else if config.harvest_exclude {
        open_rows.retain(|row| harvest_class(row).is_empty());
    }

    let milestones = charter_milestones(&config.repo_dir);
    let mut leaves: Vec<Map<String, Value>> = if config.epic.is_empty() {
        Vec::new()
    } else {
        open_rows
            .iter()
            .filter(|row| {
                let id = row_id(row);
                id.starts_with(&config.epic) && id != config.epic
            })
            .cloned()
            .collect()
    };
    leaves.sort_by_key(|row| (milestone_rank(row, &milestones), row_id(row)));

    let mut picked: Vec<Map<String, Value>> =
        leaves.into_iter().filter(|row| !is_gated(row)).collect();
    if picked.len() < config.want {
        let seen: BTreeSet<String> = picked.iter().map(row_id).collect();
        let mut rest: Vec<Map<String, Value>> = open_rows
            .iter()
            .filter(|row| !seen.contains(&row_id(row)))
            .cloned()
            .collect();
        rest.sort_by_key(|row| {
            (
                if is_gated(row) { 1_u8 } else { 0_u8 },
                priority_key(row),
                milestone_rank(row, &milestones),
                row_id(row),
            )
        });
        picked.extend(rest);
    }
    picked.retain(|row| !is_gated(row) && !is_resource_blocked(row));

    if config.count_only {
        return RunOutput {
            stdout: format!("{}\n", picked.len()),
            stderr: String::new(),
            code: 0,
        };
    }

    let mut cooldown = load_cooldown(&config.cooldown_file);
    cooldown.retain(|_, timestamp| config.now - *timestamp < config.cooldown_sec);
    if let Some(expected) = &config.cooldown_expect_id {
        if !config.cooldown_commit {
            return RunOutput {
                stdout: String::new(),
                stderr: "QUEUE_CONFIG_ERROR\tQUEUE_COOLDOWN_EXPECT_ID requires commit\n"
                    .to_string(),
                code: 4,
            };
        }
        let matches: Vec<&Map<String, Value>> = picked
            .iter()
            .filter(|row| row_id(row) == *expected)
            .collect();
        if matches.len() != 1 {
            let eligible = picked
                .iter()
                .take(5)
                .map(row_id)
                .collect::<Vec<_>>()
                .join(",");
            return RunOutput {
                stdout: String::new(),
                stderr: format!("QUEUE_COMMIT_MISMATCH\texpected={expected} eligible={eligible}\n"),
                code: 4,
            };
        }
        cooldown.insert(expected.clone(), config.now);
        if !save_cooldown(&config.cooldown_file, &cooldown) {
            return RunOutput {
                stdout: String::new(),
                stderr: format!("QUEUE_COMMIT_FAILED\t{expected}\n"),
                code: 5,
            };
        }
        return RunOutput {
            stdout: String::new(),
            stderr: String::new(),
            code: 0,
        };
    }

    let mut suppressed = Vec::new();
    let fresh: Vec<Map<String, Value>> = picked
        .into_iter()
        .filter(|row| {
            let id = row_id(row);
            if cooldown.contains_key(&id) {
                suppressed.push(id);
                false
            } else {
                true
            }
        })
        .take(config.want)
        .collect();
    let mut stdout = String::new();
    let mut stderr = String::new();
    if config.cooldown_commit {
        for row in &fresh {
            cooldown.insert(row_id(row), config.now);
        }
        let _ = save_cooldown(&config.cooldown_file, &cooldown);
    }
    if !suppressed.is_empty() {
        stderr.push_str(&format!(
            "COOLDOWN_SUPPRESSED\t{}\t{}\n",
            suppressed.len(),
            suppressed
                .iter()
                .take(5)
                .cloned()
                .collect::<Vec<_>>()
                .join(",")
        ));
    }
    for row in &fresh {
        stdout.push_str(&title_line(row));
    }

    let gated: Vec<&Map<String, Value>> = open_rows.iter().filter(|row| is_gated(row)).collect();
    if !gated.is_empty() {
        stderr.push_str(&format!(
            "GATED_AWAITING_JOSHUA\t{}\t(spend/credential/deploy only)\n",
            gated.len()
        ));
        for row in gated.iter().take(5) {
            let title: String = string_field(row, "title").chars().take(70).collect();
            stderr.push_str(&format!("  {}  {}\n", row_id(row), title));
        }
    }
    let resources: Vec<&Map<String, Value>> = open_rows
        .iter()
        .filter(|row| is_resource_blocked(row))
        .collect();
    if !resources.is_empty() {
        stderr.push_str(&format!(
            "RESOURCE_BLOCKED\t{}\tnot ready yet — unblock the resource, not the bead\n",
            resources.len()
        ));
        for row in resources.iter().take(5) {
            let title: String = string_field(row, "title").chars().take(70).collect();
            stderr.push_str(&format!("  {}  {}\n", row_id(row), title));
        }
    }
    let false_gates: Vec<&Map<String, Value>> =
        open_rows.iter().filter(|row| is_false_gate(row)).collect();
    if !false_gates.is_empty() {
        stderr.push_str(&format!(
            "FALSE_GATES\t{}\tmarker but no spend/credential/deploy — FIX THE BEAD\n",
            false_gates.len()
        ));
        for row in false_gates.iter().take(5) {
            let title: String = string_field(row, "title").chars().take(70).collect();
            stderr.push_str(&format!("  {}  {}\n", row_id(row), title));
        }
    }
    RunOutput {
        stdout,
        stderr,
        code: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn row(json: &str) -> Map<String, Value> {
        serde_json::from_str(json).expect("fixture row is valid JSON")
    }

    fn runtime(dir: &Path) -> Runtime {
        let mut env = BTreeMap::new();
        env.insert(
            "QUEUE_COOLDOWN_FILE".into(),
            dir.join("cooldown.json").display().to_string(),
        );
        env.insert("REPO_DIR".into(), dir.display().to_string());
        Runtime {
            env,
            cwd: dir.to_path_buf(),
            now: 1_000.0,
        }
    }

    #[test]
    fn autonomy_policy_real_human_blocker_is_refused() {
        let real = row(
            r#"{"id":"real","title":"[DECISION] purchase API access","description":"","status":"open"}"#,
        );
        let false_gate = row(
            r#"{"id":"false","title":"[DECISION] measure queue","description":"","status":"open"}"#,
        );
        assert!(
            is_gated(&real),
            "autonomy policy: a real purchase blocker must be refused"
        );
        assert!(
            !is_gated(&false_gate),
            "autonomy policy: a non-human decision marker remains worker work"
        );
    }

    #[test]
    fn resource_block_rule_refuses_casualty_but_keeps_unblocker() {
        let casualty = row(
            r#"{"id":"casualty","title":"Provision worker","description":"BLOCKED ON DISK until ballast is freed","status":"open"}"#,
        );
        let unblocker = row(
            r#"{"id":"unblock","title":"Attribute disk usage","description":"BLOCKED ON DISK is the measured condition","status":"open"}"#,
        );
        assert!(
            is_resource_blocked(&casualty),
            "resource block: a disk casualty must be held back"
        );
        assert!(
            !is_resource_blocked(&unblocker),
            "resource block: the attribution unblocker must remain dispatchable"
        );
    }

    #[test]
    fn epic_exclusion_rule_never_selects_parent() {
        let dir = tempfile_dir("epic");
        let input = r#"{"issues":[{"id":"EPIC-1","title":"parent","status":"open","issue_type":"epic","priority":0},{"id":"EPIC-1a","title":"leaf","status":"open","priority":0}]}"#;
        let out = run(input, &[], &runtime(&dir));
        assert!(
            !out.stdout.contains("EPIC-1\t"),
            "epic exclusion: parent accounting node must never be dispatched"
        );
        assert!(
            out.stdout.contains("EPIC-1a\t"),
            "epic exclusion: an eligible leaf must be selected"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn charter_priority_rule_promotes_defined_milestone() {
        let dir = tempfile_dir("charter");
        fs::write(dir.join("CHARTER.md"), "- M1 ship the selector\n")
            .expect("write charter fixture");
        let input = r#"{"issues":[{"id":"cp-z","title":"ordinary","status":"open","priority":0},{"id":"cp-a","title":"M1 selector","status":"open","priority":0}]}"#;
        let out = run(input, &[], &runtime(&dir));
        assert!(
            out.stdout.starts_with("cp-a\t"),
            "charter priority: a defined M1 bead must win a tied priority"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn cooldown_rule_suppresses_only_after_recorded_delivery() {
        let dir = tempfile_dir("cooldown");
        let mut rt = runtime(&dir);
        rt.env.insert("QUEUE_WANT".into(), "1".into());
        rt.env.insert("QUEUE_COOLDOWN_COMMIT".into(), "1".into());
        let input = r#"{"issues":[{"id":"cp-a","title":"work","status":"open","priority":0},{"id":"cp-b","title":"next","status":"open","priority":1}]}"#;
        let first = run(input, &[], &rt);
        assert!(
            first.stdout.starts_with("cp-a\t"),
            "cooldown: first delivery must select the highest-priority bead"
        );
        rt.now += 1.0;
        let second = run(input, &[], &rt);
        assert!(
            second.stdout.starts_with("cp-b\t"),
            "cooldown: a recorded delivery must suppress only that bead"
        );
        assert!(
            second.stderr.contains("COOLDOWN_SUPPRESSED"),
            "cooldown: suppression must be visible"
        );
        let _ = fs::remove_dir_all(dir);
    }

    fn tempfile_dir(label: &str) -> PathBuf {
        let path =
            env::temp_dir().join(format!("loop-queue-filter-{label}-{}", std::process::id()));
        fs::create_dir_all(&path).expect("create fixture directory");
        path
    }
}
