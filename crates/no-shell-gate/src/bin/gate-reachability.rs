#![forbid(unsafe_code)]

//! Machine-readable census of gate crates and test gates against their real triggers.
//!
//! This is a reachability report, not a semantic proof. A row can be reachable while its gate is
//! weak, and an unreachable row is not made healthy by having a caller somewhere in the tree.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
struct Row {
    name: String,
    kind: &'static str,
    triggers: Vec<String>,
    reachable: bool,
    proof_command: String,
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
        "{{\"name\":{},\"kind\":{},\"triggers\":{},\"reachable\":{},\"proof_command\":{}}}",
        json_string(&row.name),
        json_string(row.kind),
        json_array(&row.triggers),
        row.reachable,
        json_string(&row.proof_command),
    )
}

fn json_rows(rows: &[Row]) -> String {
    format!(
        "[{}]",
        rows.iter().map(json_row).collect::<Vec<_>>().join(","),
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

fn gate_crates(root: &Path) -> Vec<String> {
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
            let name = path.file_name()?.to_str()?.to_owned();
            (name.ends_with("-gate")
                || name.ends_with("-lint")
                || name.ends_with("-check")
                || name == "path-literal-guard"
                || name == "commit-build-fence")
                .then_some(name)
        })
        .collect::<Vec<_>>();
    names.sort();
    names
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
            path.file_name()?.to_str().map(ToOwned::to_owned)
        })
        .collect::<Vec<_>>();
    names.sort();
    names
}

fn workflow_triggers(root: &Path, package: &str) -> Vec<String> {
    let mut triggers = Vec::new();
    let Ok(entries) = fs::read_dir(root.join(".github/workflows")) else {
        return triggers;
    };
    let mut files = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            matches!(
                path.extension().and_then(|ext| ext.to_str()),
                Some("yml" | "yaml")
            )
        })
        .collect::<Vec<_>>();
    files.sort();
    for file in files {
        let Ok(text) = fs::read_to_string(&file) else {
            continue;
        };
        for (line_index, line) in text.lines().enumerate() {
            if line.trim_start().starts_with('#') {
                continue;
            }
            let package_flag = format!("-p {package}");
            if line.contains(&package_flag) || line.contains(&format!("-p={package}")) {
                triggers.push(format!(
                    "CI {}:{}",
                    file.strip_prefix(root).unwrap_or(&file).display(),
                    line_index + 1
                ));
            }
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
    let hyphen = source_text.contains(package);
    let underscore = source_text.contains(&package.replace('-', "_"));
    (hyphen || underscore).then(|| "git pre-commit hook".to_owned())
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
            let marker = filename.contains("gate")
                || filename.contains("ledger")
                || filename.contains("census")
                || filename.contains("reachability")
                || ["KNOWN-GOOD", "KNOWN-BAD", "ANTI-VACUITY", "MUTATION"]
                    .iter()
                    .any(|marker| text.contains(marker));
            marker.then_some(filename)
        })
        .collect()
}

fn add_test_gate_rows(root: &Path, package: &str, rows: &mut Vec<Row>) {
    let mut test_triggers = workflow_triggers(root, package);
    test_triggers.sort();
    test_triggers.dedup();
    for filename in test_gate_files(root, package) {
        rows.push(Row {
            name: format!("{package}/tests/{filename}"),
            kind: "test_gate",
            reachable: !test_triggers.is_empty(),
            proof_command: format!(
                "cargo test -p {package} --test {}",
                filename.trim_end_matches(".rs")
            ),
            triggers: test_triggers.clone(),
        });
    }
}

fn census(root: &Path) -> Result<(Vec<Row>, Vec<String>), String> {
    let gate_names = gate_crates(root);
    if gate_names.is_empty() {
        return Err("EMPTY_GATE_SET".to_owned());
    }
    let mut rows = Vec::new();
    let mut excluded = vec![
        "crates/no-shell-gate/src/bin/gate-reachability.rs".to_owned(),
        "crates/no-shell-gate/tests/gate_reachability.rs".to_owned(),
    ];
    for package in &gate_names {
        let mut triggers = workflow_triggers(root, package);
        if let Some(trigger) = hook_trigger(root, package) {
            triggers.push(trigger);
        }
        triggers.sort();
        triggers.dedup();
        let proof_command = triggers
            .first()
            .map(|trigger| {
                if trigger.starts_with("CI ") {
                    format!("cargo test -p {package}")
                } else {
                    "git commit (pre-commit hook)".to_owned()
                }
            })
            .unwrap_or_else(|| format!("cargo test -p {package}"));
        rows.push(Row {
            name: package.clone(),
            kind: "crate",
            reachable: !triggers.is_empty(),
            triggers,
            proof_command,
        });
    }
    for package in workspace_packages(root) {
        add_test_gate_rows(root, &package, &mut rows);
    }
    excluded.sort();
    excluded.dedup();
    Ok((rows, excluded))
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
                println!(
                    "{{\"status\":\"ok\",\"usage\":\"gate-reachability --root <repo> [--out <json>]\"}}"
                );
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
    let (status, rows, excluded, error) = match census(&root) {
        Ok((rows, excluded)) => ("ok", rows, excluded, None),
        Err(error) => ("error", Vec::new(), Vec::new(), Some(error)),
    };
    let reachable = rows.iter().filter(|row| row.reachable).count();
    let unreachable = rows.len() - reachable;
    let positive = Row {
        name: "no-shell-gate".to_owned(),
        kind: "positive_control",
        triggers: vec!["git pre-commit hook".to_owned()],
        reachable: root.join(".git/hooks/pre-commit").is_file(),
        proof_command: "stage a .sh and run .git/hooks/pre-commit; expect exit 1".to_owned(),
    };
    let machine = machine_name();
    let mut report = format!(
        "{{\"schema_version\":\"omp-gate-reachability/v1\",\"status\":{},\"machine\":{},\"root\":{},\"rows\":{},\"excluded_paths\":{},\"positive_control\":{},\"summary\":{{\"rows\":{},\"reachable\":{},\"unreachable\":{}}}",
        json_string(status),
        json_string(&machine),
        json_string(&root.display().to_string()),
        json_rows(&rows),
        json_array(&excluded),
        json_row(&positive),
        rows.len(),
        reachable,
        unreachable,
    );
    if let Some(error) = error {
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
        std::process::exit(2);
    }
}
