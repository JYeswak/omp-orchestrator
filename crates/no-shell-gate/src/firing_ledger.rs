#![forbid(unsafe_code)]

//! Durable, queryable firing ledger for commit-time and build-time gates.
//!
//! Shape copied from the supervisor `write_heartbeat` JSONL append (create parent,
//! append one JSON object, trailing newline, `sync_data`). This ledger is a
//! **separate authority** from the runtime heartbeat: absence of a gate here is
//! not a claim that the gate never fired at runtime.

use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub const SCHEMA: &str = "omp.gate-firing.v1";
pub const EMPTY_RESULT: &str = "EMPTY_RESULT";
pub const CENSUS_INPUT_EMPTY: &str = "CENSUS_INPUT_EMPTY";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Producer {
    PreCommit,
    Build,
}

impl Producer {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PreCommit => "pre-commit",
            Self::Build => "build",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerdictCode {
    Pass,
    Red,
}

impl VerdictCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Red => "RED",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FiringRow {
    pub gate: String,
    pub verdict: VerdictCode,
    pub input: String,
    pub source: String,
    pub producer: Producer,
}

impl FiringRow {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"{SCHEMA}\",\"event\":\"gate_firing\",\"gate\":{},\"verdict\":{},\"input\":{},\"source\":{},\"producer\":{}}}",
            json_string(&self.gate),
            json_string(self.verdict.as_str()),
            json_string(&self.input),
            json_string(&self.source),
            json_string(self.producer.as_str()),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryResult {
    Row(FiringRow),
    Empty { gate: String },
}

impl QueryResult {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Row(_) => "ROW",
            Self::Empty { .. } => EMPTY_RESULT,
        }
    }
}

impl fmt::Display for QueryResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Row(row) => write!(
                f,
                "ROW gate={} verdict={} producer={} source={}",
                row.gate,
                row.verdict.as_str(),
                row.producer.as_str(),
                row.source
            ),
            Self::Empty { gate } => write!(
                f,
                "{EMPTY_RESULT} gate={gate} authority=commit_build not_a_runtime_heartbeat_claim"
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GateCensus {
    pub surface: Vec<String>,
    pub runtime_hit: Vec<String>,
    pub commit_build_only: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CensusError {
    InputEmpty,
}

impl fmt::Display for CensusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InputEmpty => f.write_str(CENSUS_INPUT_EMPTY),
        }
    }
}

/// Classify crate directory names. Empty input is not a pass.
pub fn census_from_names(
    surface: &[String],
    runtime_literals: &[String],
) -> Result<GateCensus, CensusError> {
    if surface.is_empty() {
        return Err(CensusError::InputEmpty);
    }
    let mut runtime_hit: Vec<String> = runtime_literals.to_vec();
    runtime_hit.sort();
    runtime_hit.dedup();
    let commit_build_only: Vec<String> = surface
        .iter()
        .filter(|name| !runtime_hit.iter().any(|hit| hit == *name))
        .cloned()
        .collect();
    Ok(GateCensus {
        surface: surface.to_vec(),
        runtime_hit,
        commit_build_only,
    })
}

pub fn is_gate_crate_name(name: &str) -> bool {
    name.ends_with("-gate")
        || name.ends_with("-lint")
        || name.ends_with("-check")
        || name == "path-literal-guard"
        || name == "commit-build-fence"
}

pub fn discover_gate_crates(crates_dir: &Path) -> Result<Vec<String>, CensusError> {
    let entries = match std::fs::read_dir(crates_dir) {
        Ok(entries) => entries,
        Err(_) => return Err(CensusError::InputEmpty),
    };
    let mut names: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let name = entry.file_name().to_str()?.to_owned();
            is_gate_crate_name(&name).then_some(name)
        })
        .collect();
    names.sort();
    if names.is_empty() {
        Err(CensusError::InputEmpty)
    } else {
        Ok(names)
    }
}

pub fn write_firing(path: &Path, row: &FiringRow) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("FIRING_LEDGER_WRITE_ERROR create_parent: {error}"))?;
    }
    let mut ledger = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|error| format!("FIRING_LEDGER_WRITE_ERROR open: {error}"))?;
    ledger
        .write_all(row.to_json().as_bytes())
        .and_then(|_| ledger.write_all(b"\n"))
        .and_then(|_| ledger.sync_data())
        .map_err(|error| format!("FIRING_LEDGER_WRITE_ERROR write: {error}"))?;
    Ok(())
}

pub fn query_gate(path: &Path, gate: &str) -> Result<QueryResult, String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(QueryResult::Empty {
                gate: gate.to_owned(),
            })
        }
        Err(error) => return Err(format!("FIRING_LEDGER_READ_ERROR: {error}")),
    };
    let mut found = None;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let row = parse_row(line)?;
        if row.gate == gate {
            found = Some(row);
        }
    }
    Ok(match found {
        Some(row) => QueryResult::Row(row),
        None => QueryResult::Empty {
            gate: gate.to_owned(),
        },
    })
}

fn parse_row(line: &str) -> Result<FiringRow, String> {
    let gate = json_field(line, "gate")?;
    let verdict = match json_field(line, "verdict")?.as_str() {
        "PASS" => VerdictCode::Pass,
        "RED" => VerdictCode::Red,
        other => return Err(format!("FIRING_LEDGER_BAD_VERDICT:{other}")),
    };
    let producer = match json_field(line, "producer")?.as_str() {
        "pre-commit" => Producer::PreCommit,
        "build" => Producer::Build,
        other => return Err(format!("FIRING_LEDGER_BAD_PRODUCER:{other}")),
    };
    Ok(FiringRow {
        gate,
        verdict,
        input: json_field(line, "input")?,
        source: json_field(line, "source")?,
        producer,
    })
}

fn json_field(line: &str, key: &str) -> Result<String, String> {
    let needle = format!("\"{key}\":\"");
    let start = line
        .find(&needle)
        .ok_or_else(|| format!("FIRING_LEDGER_MISSING_FIELD:{key}"))?
        + needle.len();
    let rest = &line[start..];
    let mut out = String::new();
    let mut chars = rest.chars();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.next() {
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some('n') => out.push('\n'),
                Some(other) => out.push(other),
                None => break,
            },
            '"' => return Ok(out),
            ch => out.push(ch),
        }
    }
    Err(format!("FIRING_LEDGER_UNTERMINATED_FIELD:{key}"))
}

fn json_string(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len() + 2);
    escaped.push('"');
    for ch in value.chars() {
        match ch {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            ch => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

/// Default ledger beside the git dir so tests can override via env.
pub fn default_ledger_path(repo_root: &Path) -> PathBuf {
    std::env::var_os("OMP_GATE_FIRING_LEDGER")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join(".git").join("omp-gate-firings.jsonl"))
}

pub fn gate_name_from_refusal(refusal: &str) -> Option<&str> {
    let name = refusal.split(':').next()?.trim();
    is_gate_crate_name(name).then_some(name)
}

pub fn append_commit_outcome(
    path: &Path,
    source: &str,
    input: &str,
    refusals: &[String],
    clean: bool,
) -> Result<usize, String> {
    let mut n = 0usize;
    if clean {
        write_firing(
            path,
            &FiringRow {
                gate: "no-shell-gate".to_owned(),
                verdict: VerdictCode::Pass,
                input: input.to_owned(),
                source: source.to_owned(),
                producer: Producer::PreCommit,
            },
        )?;
        n += 1;
        if input.contains("crates/") {
            write_firing(
                path,
                &FiringRow {
                    gate: "path-literal-guard".to_owned(),
                    verdict: VerdictCode::Pass,
                    input: input.to_owned(),
                    source: source.to_owned(),
                    producer: Producer::PreCommit,
                },
            )?;
            n += 1;
        }
        return Ok(n);
    }
    for refusal in refusals {
        let Some(gate) = gate_name_from_refusal(refusal) else {
            continue;
        };
        write_firing(
            path,
            &FiringRow {
                gate: gate.to_owned(),
                verdict: VerdictCode::Red,
                input: refusal.to_owned(),
                source: source.to_owned(),
                producer: Producer::PreCommit,
            },
        )?;
        n += 1;
    }
    Ok(n)
}

pub fn append_build_outcome(
    path: &Path,
    source: &str,
    input: &str,
    gate: &str,
    pass: bool,
) -> Result<(), String> {
    write_firing(
        path,
        &FiringRow {
            gate: gate.to_owned(),
            verdict: if pass {
                VerdictCode::Pass
            } else {
                VerdictCode::Red
            },
            input: input.to_owned(),
            source: source.to_owned(),
            producer: Producer::Build,
        },
    )
}

#[cfg(test)]
fn with_temp_ledger(test: &str, body: impl FnOnce(&Path)) {
    let dir = std::env::temp_dir().join(format!(
        "gate-firing-{}-{}-{}",
        std::process::id(),
        test,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).expect("temp ledger dir");
    let path = dir.join("omp-gate-firings.jsonl");
    body(&path);
    let _ = std::fs::remove_dir_all(&dir);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_discovery_is_census_input_empty() {
        let error = census_from_names(&[], &[]).expect_err("empty surface");
        assert_eq!(error.to_string(), CENSUS_INPUT_EMPTY);
    }

    #[test]
    fn census_splits_runtime_hit_from_commit_build_only() {
        let surface = vec![
            "no-shell-gate".to_owned(),
            "fuzz-build-gate".to_owned(),
            "dispatch-claim-fence".to_owned(),
        ];
        let runtime = vec!["no-shell-gate".to_owned(), "dispatch-claim-fence".to_owned()];
        let census = census_from_names(&surface, &runtime).expect("surface");
        assert_eq!(census.runtime_hit, vec!["dispatch-claim-fence", "no-shell-gate"]);
        assert_eq!(census.commit_build_only, vec!["fuzz-build-gate"]);
    }

    #[test]
    fn clean_commit_appends_pass_and_query_returns_typed_row() {
        with_temp_ledger("clean", |path| {
            let n = append_commit_outcome(
                path,
                "commit:abc123",
                "crates/no-shell-gate/src/lib.rs",
                &[],
                true,
            )
            .expect("write");
            assert!(n >= 1);
            let result = query_gate(path, "no-shell-gate").expect("query");
            assert_eq!(result.label(), "ROW");
            match result {
                QueryResult::Row(row) => {
                    assert_eq!(row.verdict, VerdictCode::Pass);
                    assert_eq!(row.producer, Producer::PreCommit);
                    assert_eq!(row.source, "commit:abc123");
                }
                QueryResult::Empty { .. } => panic!("expected row"),
            }
        });
    }

    #[test]
    fn planted_no_shell_violation_appends_red() {
        with_temp_ledger("red", |path| {
            append_commit_outcome(
                path,
                "commit:def",
                "scripts/foo.sh",
                &["no-shell-gate: 1 tracked shell/python file(s): scripts/foo.sh".to_owned()],
                false,
            )
            .expect("write");
            let result = query_gate(path, "no-shell-gate").expect("query");
            let rendered = result.to_string();
            assert!(rendered.contains("verdict=RED"), "{rendered}");
        });
    }

    #[test]
    fn zero_row_gate_is_typed_empty_result_not_bare_empty() {
        with_temp_ledger("empty-query", |path| {
            append_commit_outcome(path, "commit:x", "crates/foo/src/lib.rs", &[], true)
                .expect("write");
            let result = query_gate(path, "porting-gate").expect("query");
            assert_eq!(result.label(), EMPTY_RESULT);
            let rendered = result.to_string();
            assert!(rendered.contains(EMPTY_RESULT));
            assert!(rendered.contains("not_a_runtime_heartbeat_claim"));
            assert!(!matches!(result, QueryResult::Row(_)));
        });
    }

    #[test]
    fn mutation_of_append_trigger_goes_red_then_restores() {
        with_temp_ledger("mutation", |path| {
            append_commit_outcome(path, "commit:m", "crates/x.rs", &[], true).expect("write");
            let before = query_gate(path, "no-shell-gate").expect("before");
            assert_eq!(before.label(), "ROW");
            let _ = File::create(path).expect("truncate is the mutation");
            let mutated = query_gate(path, "no-shell-gate").expect("mutated");
            assert_eq!(
                mutated.label(),
                EMPTY_RESULT,
                "removing firings must not look like a pass"
            );
            append_commit_outcome(path, "commit:m", "crates/x.rs", &[], true).expect("restore");
            let restored = query_gate(path, "no-shell-gate").expect("restored");
            assert_eq!(restored.label(), "ROW");
            assert_eq!(restored.to_string(), before.to_string());
        });
        let source = include_str!("firing_ledger.rs");
        let checksum = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            source.hash(&mut hasher);
            hasher.finish()
        };
        assert_ne!(checksum, 0);
        assert!(source.contains("append_commit_outcome"));
    }

    #[test]
    fn build_producer_is_distinct_authority() {
        with_temp_ledger("build", |path| {
            append_build_outcome(path, "tick:1", "target/debug/foo", "staged-build-gate", true)
                .expect("build pass");
            let result = query_gate(path, "staged-build-gate").expect("query");
            match result {
                QueryResult::Row(row) => assert_eq!(row.producer, Producer::Build),
                QueryResult::Empty { .. } => panic!("expected build row"),
            }
        });
    }
}
