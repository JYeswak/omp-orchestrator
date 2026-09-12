#![forbid(unsafe_code)]

//! Machine-readable census of gate mechanisms, executable triggers, document mentions, and read state.
//!
//! This is a reachability measurement, not a semantic proof. A gate row can be executable and
//! still fail; an observed verdict is evidence that a reader saw it, not that it was correct.

use serde_yaml_ng::Value as YamlValue;
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use text_structure::{code_only, toml_code_only, yaml_code_only};

const SELF_SOURCE: &str = "crates/no-shell-gate/src/bin/gate-reachability.rs";
const SELF_TEST: &str = "crates/no-shell-gate/tests/gate_reachability.rs";
const SELF_OUTPUT: &str = ".flywheel/6nhj-stage-gate-census.json";
const SELF_BEAD: &str = "omp-orchestrator-6nhj";

#[derive(Debug, Clone)]
struct Row {
    name: String,
    kind: &'static str,
    triggers: Vec<String>,
    documents: Vec<String>,
    reachable: bool,
    verdict: &'static str,
    verdict_observed: bool,
    read_state: &'static str,
    proof_command: String,
}

#[derive(Debug, Clone)]
struct ReadState {
    population: &'static str,
    bytes: usize,
    lines: usize,
    window: &'static str,
    excluded_bead_ids: Vec<String>,
    observed_rows: Vec<String>,
    observed_subjects: Vec<String>,
}

#[derive(Debug, Clone, Copy)]
struct Controls {
    ntm: usize,
    orchestrator_tick: usize,
    guaranteed_absent: usize,
}

#[derive(Debug)]
struct Census {
    rows: Vec<Row>,
    excluded_paths: Vec<String>,
    read_state: ReadState,
    controls: Controls,
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            ch if ch.is_control() => escaped.push_str(&format!("\\u{:04x}", ch as u32)),
            ch => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

fn json_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| json_string(value))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn json_row(row: &Row) -> String {
    format!(
        "{{\"name\":{},\"kind\":{},\"triggers\":{},\"documents\":{},\"reachable\":{},\"verdict\":{},\"verdict_observed\":{},\"read_state\":{},\"proof_command\":{}}}",
        json_string(&row.name),
        json_string(row.kind),
        json_array(&row.triggers),
        json_array(&row.documents),
        row.reachable,
        json_string(row.verdict),
        row.verdict_observed,
        json_string(row.read_state),
        json_string(&row.proof_command),
    )
}

fn json_rows(rows: &[Row]) -> String {
    format!(
        "[{}]",
        rows.iter().map(json_row).collect::<Vec<_>>().join(",")
    )
}

fn machine_name() -> String {
    for key in ["HOSTNAME", "COMPUTERNAME"] {
        if let Ok(value) = env::var(key) {
            let value = value.trim();
            if !value.is_empty() {
                return value.to_owned();
            }
        }
    }
    "unknown".to_owned()
}

// Comment handling routes through `text_structure` (bead -9ub39): a generic
// marker-stripper beside the kernel is a lint finding, not a helper. Each former
// call site now names its language: `code_only` for Rust source, `toml_code_only`
// per line for `#`-comment files. Two drive-by corrections fall out: the old
// helper closed a `/*` block at the NEXT `/*` rather than at `*/`, swallowing
// code between them; and it stripped `#` inside plist XML, where `#` is content.

fn json_row_value(line: &str) -> Option<serde_json::Value> {
    serde_json::from_str(line).ok()
}

fn read_state(root: &Path) -> ReadState {
    let excluded_bead_ids = vec![SELF_BEAD.to_owned()];
    let path = root.join(".beads/issues.jsonl");
    let Ok(text) = fs::read_to_string(path) else {
        return ReadState {
            population: "full-file",
            bytes: 0,
            lines: 0,
            window: "unavailable",
            excluded_bead_ids,
            observed_rows: Vec::new(),
            observed_subjects: Vec::new(),
        };
    };
    let mut observed_rows = BTreeSet::new();
    let mut observed_subjects = BTreeSet::new();
    for line in text.lines() {
        let Some(value) = json_row_value(line) else {
            continue;
        };
        let Some(id) = value.get("id").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if excluded_bead_ids.iter().any(|excluded| excluded == id) {
            continue;
        }
        let body = value.to_string().to_ascii_lowercase();
        if body.contains("verdict_observed")
            || (body.contains("gate-reachability")
                && ["status check", "merge gate", "acts on", "observed"]
                    .iter()
                    .any(|needle| body.contains(needle)))
        {
            observed_rows.insert(id.to_owned());
            observed_subjects.insert(body.clone());
        }
    }
    ReadState {
        population: "full-file",
        bytes: text.len(),
        lines: text.lines().count(),
        window: "full-file",
        excluded_bead_ids,
        observed_rows: observed_rows.into_iter().collect(),
        observed_subjects: observed_subjects.into_iter().collect(),
    }
}

fn duplicate_block_mapping_key(text: &str) -> Option<(String, usize)> {
    let mut stack: Vec<(usize, BTreeSet<String>)> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let code = yaml_code_only(raw);
        let trimmed = code.trim_start();
        if trimmed.is_empty() {
            continue;
        }
        let indent = code.chars().take_while(|c| *c == ' ').count();
        while stack.last().is_some_and(|(seen, _)| *seen > indent) {
            stack.pop();
        }
        if stack.last().map(|(seen, _)| *seen) != Some(indent) {
            stack.push((indent, BTreeSet::new()));
        }
        if trimmed.starts_with('-') || trimmed.starts_with('{') {
            continue;
        }
        let Some(colon) = trimmed.find(':') else {
            continue;
        };
        let key = trimmed[..colon].trim();
        if key.is_empty() {
            continue;
        }
        let keys = &mut stack.last_mut().expect("indent frame").1;
        if !keys.insert(key.to_owned()) {
            return Some((key.to_owned(), index + 1));
        }
    }
    None
}

fn strict_workflow_parse(path: &Path, text: &str) -> Result<(), String> {
    if let Some((key, line)) = duplicate_block_mapping_key(text) {
        return Err(format!(
            "STRICT_YAML_PARSE path={} detail=duplicate mapping key {key:?} at line {line}",
            path.display()
        ));
    }
    serde_yaml_ng::from_str::<YamlValue>(text)
        .map(|_| ())
        .map_err(|error| format!("STRICT_YAML_PARSE path={} detail={error}", path.display()))
}

fn validate_workflows(root: &Path) -> Result<(), String> {
    let Ok(entries) = fs::read_dir(root.join(".github/workflows")) else {
        return Ok(());
    };
    let mut files = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| matches!(path.extension().and_then(|ext| ext.to_str()), Some("yml" | "yaml")))
        .collect::<Vec<_>>();
    files.sort();
    for file in files {
        let text = fs::read_to_string(&file)
            .map_err(|error| format!("WORKFLOW_READ_FAILED path={} detail={error}", file.display()))?;
        strict_workflow_parse(&file, &text)?;
    }
    Ok(())
}

fn workflow_triggers(root: &Path, package: &str) -> Result<Vec<String>, String> {
    let mut triggers = Vec::new();
    let Ok(entries) = fs::read_dir(root.join(".github/workflows")) else {
        return Ok(triggers);
    };
    let mut files = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| matches!(path.extension().and_then(|ext| ext.to_str()), Some("yml" | "yaml")))
        .collect::<Vec<_>>();
    files.sort();
    for file in files {
        let text = fs::read_to_string(&file)
            .map_err(|error| format!("WORKFLOW_READ_FAILED path={} detail={error}", file.display()))?;
        let package_flag = format!("-p {package}");
        let package_equals = format!("-p={package}");
        // Workflow files are YAML: `#` opens a comment only after whitespace,
        // and `//` is content (URLs). `yaml_code_only` encodes both.
        let hit = text.lines().any(|line| {
            let code = yaml_code_only(line);
            code.contains(&package_flag) || code.contains(&package_equals)
        });
        if hit {
            triggers.push(format!("CI {}", file.strip_prefix(root).unwrap_or(&file).display()));
        }
    }
    triggers.sort();
    triggers.dedup();
    Ok(triggers)
}

fn launchd_triggers(root: &Path, package: &str) -> Vec<String> {
    let mut triggers = Vec::new();
    let Ok(entries) = fs::read_dir(root.join("launchd")) else {
        return triggers;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        // Plist files are XML: `#` is content (URL fragments), not a comment.
        // The old marker set stripped it, hiding triggers in fragment URLs.
        if code_only(&text).contains(package) {
            triggers.push(format!("launchd {}", path.strip_prefix(root).unwrap_or(&path).display()));
        }
    }
    triggers.sort();
    triggers.dedup();
    triggers
}

fn hook_trigger(root: &Path, package: &str) -> Option<String> {
    let hook = root.join(".git/hooks/pre-commit");
    let source = root.join("crates/no-shell-gate/src/bin/pre-commit-gate.rs");
    if !hook.is_file() || !source.is_file() {
        return None;
    }
    let source_text = fs::read_to_string(source).ok()?;
    let source_text = code_only(&source_text).into_owned();
    let hyphen = source_text.contains(package);
    let underscore = source_text.contains(&package.replace('-', "_"));
    (hyphen || underscore).then(|| "git pre-commit hook".to_owned())
}

fn crontab_text(root: &Path) -> String {
    let fixture = root.join(".omp/crontab");
    if let Ok(text) = fs::read_to_string(fixture) {
        return text;
    }
    Command::new("crontab")
        .arg("-l")
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
        .unwrap_or_default()
}

fn crontab_matches(text: &str, token: &str) -> Vec<String> {
    // Crontab `#` is a line comment; `toml_code_only` strips it per line with
    // quote awareness the old marker loop lacked for `"` strings.
    text.lines()
        .map(|line| toml_code_only(line).into_owned())
        .enumerate()
        .filter(|(_, line)| line.split_whitespace().any(|field| field == token) || line.contains(token))
        .map(|(index, line)| format!("crontab:{}:{}", index + 1, line.trim()))
        .collect()
}

fn document_mentions(root: &Path, package: &str) -> Vec<String> {
    let mut paths = vec![root.join("README.md"), root.join("AGENTS.md"), root.join("CLAUDE.md")];
    if let Ok(entries) = fs::read_dir(root.join(".flywheel")) {
        paths.extend(entries.filter_map(Result::ok).map(|entry| entry.path()).filter(|path| path.is_file()));
    }
    paths.sort();
    paths
        .into_iter()
        .filter_map(|path| {
            let text = fs::read_to_string(&path).ok()?;
            text.contains(package).then(|| path.strip_prefix(root).unwrap_or(&path).display().to_string())
        })
        .collect()
}

fn test_gate_files(root: &Path, package: &str) -> Vec<String> {
    let tests_dir = root.join("crates").join(package).join("tests");
    let Ok(entries) = fs::read_dir(tests_dir) else {
        return Vec::new();
    };
    let mut files = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .collect::<Vec<_>>();
    files.sort();
    files
        .into_iter()
        .filter_map(|path| {
            let filename = path.file_name()?.to_str()?.to_owned();
            if package == "no-shell-gate" && filename == "gate_reachability.rs" {
                return None;
            }
            let text = fs::read_to_string(&path).ok()?;
            let code = code_only(&text);
            let marker = filename.contains("gate")
                || filename.contains("ledger")
                || filename.contains("census")
                || filename.contains("reachability")
                || ["KNOWN-GOOD", "KNOWN-BAD", "ANTI-VACUITY", "MUTATION"]
                    .iter()
                    .any(|mark| code.contains(mark));
            marker.then_some(filename)
        })
        .collect()
}

fn workspace_packages(root: &Path) -> Vec<String> {
    let mut names = fs::read_dir(root.join("crates"))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            path.file_name()
                .and_then(|name| name.to_str())
                .map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn gate_crates(root: &Path) -> Vec<String> {
    let mut names = workspace_packages(root)
        .into_iter()
        .filter(|name| {
            name.ends_with("-gate")
                || name.ends_with("-lint")
                || name.ends_with("-check")
                || name == "path-literal-guard"
                || name == "commit-build-fence"
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}
fn read_state_for(package: &str, read_state: &ReadState) -> (bool, &'static str) {
    let observed = read_state
        .observed_subjects
        .iter()
        .any(|subject| subject.contains(package));
    (observed, if observed { "OBSERVED" } else { "UNREAD" })
}

fn row(
    name: String,
    kind: &'static str,
    triggers: Vec<String>,
    documents: Vec<String>,
    observed: bool,
    proof_command: String,
) -> Row {
    let reachable = !triggers.is_empty();
    let (verdict, read_state) = if kind == "negative_control" {
        ("ABSENT", "NOT_APPLICABLE")
    } else if reachable {
        ("UNRUN", if observed { "OBSERVED" } else { "UNREAD" })
    } else {
        ("INERT", "NOT_APPLICABLE")
    };
    Row {
        name,
        kind,
        triggers,
        documents,
        reachable,
        verdict,
        verdict_observed: observed,
        read_state,
        proof_command,
    }
}

fn census(root: &Path) -> Result<Census, String> {
    validate_workflows(root)?;
    let gate_names = gate_crates(root);
    if gate_names.is_empty() {
        return Err("EMPTY_GATE_SET".to_owned());
    }
    let read_state = read_state(root);
    let crontab = crontab_text(root);
    let mut rows = Vec::new();
    let mut excluded = vec![SELF_SOURCE.to_owned(), SELF_TEST.to_owned(), SELF_OUTPUT.to_owned()];
    for package in &gate_names {
        let mut triggers = workflow_triggers(root, package)?;
        if let Some(trigger) = hook_trigger(root, package) {
            triggers.push(trigger);
        }
        triggers.extend(launchd_triggers(root, package));
        triggers.extend(crontab_matches(&crontab, package));
        triggers.sort();
        triggers.dedup();
        let documents = document_mentions(root, package);
        let (observed, _) = read_state_for(package, &read_state);
        let proof_command = triggers
            .first()
            .cloned()
            .unwrap_or_else(|| format!("cargo test -p {package}"));
        rows.push(row(package.clone(), "crate", triggers, documents, observed, proof_command));
    }
    for package in workspace_packages(root) {
        let mut triggers = workflow_triggers(root, &package)?;
        if let Some(trigger) = hook_trigger(root, &package) {
            triggers.push(trigger);
        }
        triggers.extend(launchd_triggers(root, &package));
        triggers.extend(crontab_matches(&crontab, &package));
        triggers.sort();
        triggers.dedup();
        let documents = document_mentions(root, &package);
        let (observed, _) = read_state_for(&package, &read_state);
        for filename in test_gate_files(root, &package) {
            rows.push(row(
                format!("{package}/tests/{filename}"),
                "test_gate",
                triggers.clone(),
                documents.clone(),
                observed,
                format!("cargo test -p {package} --test {}", filename.trim_end_matches(".rs")),
            ));
        }
    }
    rows.push(row(
        "zzz-cannot-exist".to_owned(),
        "negative_control",
        Vec::new(),
        Vec::new(),
        false,
        "guaranteed absent control".to_owned(),
    ));
    excluded.sort();
    excluded.dedup();
    let controls = Controls {
        ntm: crontab_matches(&crontab, "ntm").len(),
        orchestrator_tick: crontab_matches(&crontab, "orchestrator-tick").len(),
        guaranteed_absent: crontab_matches(&crontab, "zzz-cannot-exist").len(),
    };
    Ok(Census {
        rows,
        excluded_paths: excluded,
        read_state,
        controls,
    })
}

fn parse_args() -> Result<(PathBuf, Option<PathBuf>), String> {
    let mut args = env::args().skip(1);
    let mut root = PathBuf::from(".");
    let mut output = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--root" => root = PathBuf::from(args.next().ok_or("MISSING_ROOT")?),
            "--out" => output = Some(PathBuf::from(args.next().ok_or("MISSING_OUTPUT")?)),
            "--help" | "-h" => {
                println!("{{\"status\":\"ok\",\"usage\":\"gate-reachability --root <repo> [--out <json>]\"}}");
                return Ok((root, output));
            }
            other => return Err(format!("UNKNOWN_ARGUMENT:{other}")),
        }
    }
    Ok((root, output))
}

fn main() {
    let (root, output) = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            println!("{{\"status\":\"error\",\"error\":{}}}", json_string(&error));
            std::process::exit(2);
        }
    };
    let root = fs::canonicalize(&root).unwrap_or(root);
    let (status, census, error) = match census(&root) {
        Ok(census) => ("ok", Some(census), None),
        Err(error) => ("error", None, Some(error)),
    };
    let empty = Census {
        rows: Vec::new(),
        excluded_paths: vec![SELF_SOURCE.to_owned(), SELF_TEST.to_owned(), SELF_OUTPUT.to_owned()],
        read_state: ReadState {
            population: "full-file",
            bytes: 0,
            lines: 0,
            window: "unavailable",
            excluded_bead_ids: vec![SELF_BEAD.to_owned()],
            observed_rows: Vec::new(),
            observed_subjects: Vec::new(),
        },
        controls: Controls {
            ntm: 0,
            orchestrator_tick: 0,
            guaranteed_absent: 0,
        },
    };
    let census = census.unwrap_or(empty);
    let reachable = census.rows.iter().filter(|row| row.reachable).count();
    let unreachable = census.rows.len() - reachable;
    let positive = row(
        "no-shell-gate".to_owned(),
        "positive_control",
        census
            .rows
            .iter()
            .find(|row| row.name == "no-shell-gate")
            .map(|row| row.triggers.clone())
            .unwrap_or_default(),
        Vec::new(),
        false,
        "run gate-reachability against a real no-shell trigger".to_owned(),
    );
    let machine = machine_name();
    let read_state_excluded = json_array(&census.read_state.excluded_bead_ids);
    let read_state_observed = json_array(&census.read_state.observed_rows);
    let mut report = format!(
        "{{\"schema_version\":\"omp-gate-reachability/v1\",\"status\":{},\"machine\":{},\"root\":{},\"rows\":{},\"excluded_paths\":{},\"positive_control\":{},\"controls\":{{\"ntm\":{},\"orchestrator_tick\":{},\"zzz_cannot_exist\":{}}},\"read_state\":{{\"population\":{},\"bytes\":{},\"lines\":{},\"window\":{},\"excluded_bead_ids\":{},\"observed_rows\":{} }},\"summary\":{{\"rows\":{},\"reachable\":{},\"unreachable\":{}}}",
        json_string(status),
        json_string(&machine),
        json_string(&root.display().to_string()),
        json_rows(&census.rows),
        json_array(&census.excluded_paths),
        json_row(&positive),
        census.controls.ntm,
        census.controls.orchestrator_tick,
        census.controls.guaranteed_absent,
        json_string(census.read_state.population),
        census.read_state.bytes,
        census.read_state.lines,
        json_string(census.read_state.window),
        read_state_excluded,
        read_state_observed,
        census.rows.len(),
        reachable,
        unreachable,
    );
    if let Some(ref error) = error {
        report.push_str(&format!(",\"error\":{}", json_string(&error)));
    }
    report.push('}');
    println!("{report}");
    if let Some(output) = output {
        if let Some(parent) = output.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(error) = fs::write(output, &report) {
            eprintln!("gate-reachability output write failed: {error}");
            std::process::exit(2);
        }
    }
    if status != "ok" {
        let code = error
            .as_deref()
            .filter(|reason| reason.starts_with("STRICT_YAML_PARSE"))
            .map_or(2, |_| 1);
        std::process::exit(code);
    }
}
