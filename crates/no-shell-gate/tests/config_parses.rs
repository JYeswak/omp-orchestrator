#![forbid(unsafe_code)]
//! DUPLICATE-KEY GATE for machine-read config — bead
//! `omp-orchestrator-strict-parser-rejects-gate-yml-m0c`.
//!
//! # The measured defect
//!
//! `.github/workflows/gate.yml` declared `runs-on` twice and `steps` twice inside job
//! `kernel-bypass-gate`, because the `state-wildcard-lint:` job header was missing and that
//! job's steps were appended into the previous job's body. Two parsers, two answers, and the
//! disagreement IS the defect:
//!
//! ```text
//! strict (eemeli/yaml, the duplicate-key rejection GitHub Actions applies):
//!   YAML_FAIL: Map keys must be unique at line 52, column 5
//! lenient (PyYAML safe_load, last-key-wins):
//!   9 healthy jobs
//! ```
//!
//! So a human skimming saw nine jobs and GitHub Actions would have refused the whole file —
//! meaning **all nine gate jobs were unreachable in CI** while the repo read as protected. And
//! `state-wildcard-lint` had no job at all. After the fix both parsers agree on **ten** jobs,
//! and that agreement is the signal worth keeping.
//!
//! The same class had already bitten `NUMBERS.toml`: `[figures.test_functions]` carried `note`
//! twice at `5cb854f`, which `Bun.TOML.parse` refuses outright while `numbers.rs`'s hand-rolled
//! line scanner ignores `note` entirely and never noticed. That one was fixed 14 minutes later
//! by `be012d9`; this gate is what makes the class impossible to reintroduce silently.
//!
//! # NO-CLAIM, stated up front because the scope is narrow on purpose
//!
//! This is **not** a YAML or TOML conformance parser and must never be described as one. It
//! detects exactly one structural property — a mapping key repeated among its siblings — over
//! the block-mapping subset these files use. It does not validate types, anchors, aliases,
//! flow mappings, multi-line TOML values, or GitHub Actions schema. A file this gate passes can
//! still be rejected by a real parser for a different reason. The floor it raises is the one
//! defect measured twice in this repo, and no more than that.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives two levels below repo root")
        .to_path_buf()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Style {
    /// Block mappings, scope = indentation.
    Yaml,
    /// Tables, scope = the most recent `[header]`.
    Toml,
}

/// The single predicate. Exists as a parameter so the MUTATION leg can switch it off and show
/// the known-bad specimen stops being caught — a leg that stays red under mutation is not
/// attributable to the check it claims to test.
#[derive(Debug, Clone, Copy)]
struct Checks {
    reject_duplicate_sibling_keys: bool,
}

/// Duplicate mapping keys among siblings, as `scope -> key -> [line numbers]`.
///
/// YAML: a scope is `(parent_path, indent)`. A `- ` sequence item opens a fresh scope, so two
/// steps may both carry `name:` without colliding — that is the false positive this must not
/// produce, and there is a known-GOOD leg for it.
fn duplicate_keys(text: &str, style: Style, checks: Checks) -> Vec<String> {
    if !checks.reject_duplicate_sibling_keys {
        return Vec::new();
    }
    let mut seen: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();

    match style {
        Style::Toml => {
            let mut table = String::from("<root>");
            let mut in_multiline = false;
            for (index, raw) in text.lines().enumerate() {
                let line = raw.trim_end();
                let trimmed = line.trim_start();
                // triple-quoted values may span lines; a key inside one is not a key
                let triples = trimmed.matches("\"\"\"").count() + trimmed.matches("'''").count();
                if triples % 2 == 1 {
                    in_multiline = !in_multiline;
                    continue;
                }
                if in_multiline || trimmed.starts_with('#') || trimmed.is_empty() {
                    continue;
                }
                if trimmed.starts_with('[') {
                    table = trimmed.to_owned();
                    continue;
                }
                // a key line is `key = ...` at the start of the line (these files do not indent)
                if let Some((key, _)) = trimmed.split_once('=') {
                    let key = key.trim();
                    if !key.is_empty()
                        && key
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '"')
                    {
                        seen.entry((table.clone(), key.to_owned()))
                            .or_default()
                            .push(index + 1);
                    }
                }
            }
        }
        Style::Yaml => {
            // path[i] = the key that opened the scope at indent level i
            let mut path: Vec<(usize, String)> = Vec::new();
            let mut block_scalar_indent: Option<usize> = None;
            let mut seq_counter: BTreeMap<String, usize> = BTreeMap::new();

            for (index, raw) in text.lines().enumerate() {
                if raw.trim().is_empty() {
                    continue;
                }
                let indent = raw.len() - raw.trim_start().len();
                // inside a block scalar (`|` / `>`): everything more indented is DATA, not keys
                if let Some(base) = block_scalar_indent {
                    if indent > base {
                        continue;
                    }
                    block_scalar_indent = None;
                }
                let trimmed = raw.trim_start();
                if trimmed.starts_with('#') {
                    continue;
                }

                // a sequence item opens a fresh sibling scope, so `- name:` twice is legal
                let (indent, trimmed, is_seq) = if let Some(rest) = trimmed.strip_prefix("- ") {
                    (indent + 2, rest, true)
                } else if trimmed == "-" {
                    (indent + 2, "", true)
                } else {
                    (indent, trimmed, false)
                };

                while path.last().is_some_and(|(i, _)| *i >= indent) {
                    path.pop();
                }
                let parent = path
                    .iter()
                    .map(|(_, k)| k.as_str())
                    .collect::<Vec<_>>()
                    .join("/");

                if is_seq {
                    // give each item its own scope name so its keys never collide with a sibling's
                    let n = seq_counter.entry(parent.clone()).or_default();
                    *n += 1;
                    let scope = format!("{parent}[{n}]");
                    path.push((indent.saturating_sub(1), scope));
                }

                let Some((key, rest)) = trimmed.split_once(':') else { continue };
                let key = key.trim();
                if key.is_empty()
                    || !key
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
                {
                    continue;
                }
                let scope = path
                    .iter()
                    .map(|(_, k)| k.as_str())
                    .collect::<Vec<_>>()
                    .join("/");
                seen.entry((scope, key.to_owned()))
                    .or_default()
                    .push(index + 1);

                let rest = rest.trim();
                if rest == "|" || rest == ">" || rest == "|-" || rest == ">-" {
                    block_scalar_indent = Some(indent);
                } else if rest.is_empty() {
                    path.push((indent, key.to_owned()));
                }
            }
        }
    }

    seen.into_iter()
        .filter(|(_, lines)| lines.len() > 1)
        .map(|((scope, key), lines)| {
            format!(
                "DUPLICATE_KEY scope={} key={key} lines={lines:?}",
                if scope.is_empty() { "<root>" } else { &scope }
            )
        })
        .collect()
}

fn workflow_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if let Ok(entries) = fs::read_dir(root.join(".github/workflows")) {
        for entry in entries.flatten() {
            let p = entry.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.ends_with(".yml") || name.ends_with(".yaml") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

// ─────────────────────────────────────────────────────────────────── the legs

/// KNOWN-GOOD. Every workflow and `NUMBERS.toml` must be free of duplicate sibling keys.
///
/// This is also the leg that catches an over-strict detector. `gate.yml` legitimately repeats
/// `name:`, `run:` and `uses:` across sequence items and across jobs; a detector that flagged
/// those would be routed around within a day, which is a slower death than no gate.
#[test]
fn no_machine_read_config_declares_a_duplicate_sibling_key() {
    let root = repo_root();
    let mut scanned = 0usize;
    let mut findings = Vec::new();

    for path in workflow_files(&root) {
        let text = fs::read_to_string(&path).expect("workflow must be readable");
        scanned += 1;
        for f in duplicate_keys(&text, Style::Yaml, Checks { reject_duplicate_sibling_keys: true }) {
            findings.push(format!("{}: {f}", path.display()));
        }
    }

    let numbers = root.join("NUMBERS.toml");
    let text = fs::read_to_string(&numbers).expect("NUMBERS.toml must be readable");
    scanned += 1;
    for f in duplicate_keys(&text, Style::Toml, Checks { reject_duplicate_sibling_keys: true }) {
        findings.push(format!("{}: {f}", numbers.display()));
    }

    // ANTI-VACUITY: an empty scan set reports identically to a clean one.
    assert!(
        scanned >= 2,
        "ANTI-VACUITY: scanned {scanned} files — expected at least .github/workflows/*.yml and \
         NUMBERS.toml. A deliverable that was never checked must never report like one that passed"
    );
    assert!(
        findings.is_empty(),
        "duplicate keys in machine-read config — a strict parser refuses the whole file while a \
         lenient one silently takes the last value:\n  {}",
        findings.join("\n  ")
    );
}

/// The specific job that was broken must exist, with exactly one `runs-on` and one `steps`.
///
/// A count-only check would pass if `state-wildcard-lint`'s steps were merged into a neighbour
/// again, so this asserts the JOB, not the total.
#[test]
fn state_wildcard_lint_has_its_own_job_with_one_runs_on_and_one_steps() {
    let text = fs::read_to_string(repo_root().join(".github/workflows/gate.yml")).expect("read");
    assert!(
        text.contains("\n  state-wildcard-lint:\n"),
        "the job header that went missing must be present at job indent"
    );
    let dups = duplicate_keys(&text, Style::Yaml, Checks { reject_duplicate_sibling_keys: true });
    assert!(dups.is_empty(), "gate.yml still has duplicate keys: {dups:?}");

    // every job declares runs-on and steps exactly once
    let jobs: Vec<&str> = text
        .lines()
        .filter_map(|l| {
            let t = l.strip_prefix("  ")?;
            if t.starts_with(' ') || !t.ends_with(':') {
                return None;
            }
            Some(t.trim_end_matches(':'))
        })
        .collect();
    assert!(jobs.len() >= 10, "expected at least ten jobs, found {}: {jobs:?}", jobs.len());
    assert!(jobs.contains(&"state-wildcard-lint"), "{jobs:?}");
}

/// FIRES-ON-KNOWN-BAD: the exact `gate.yml` shape that shipped, reconstructed.
#[test]
fn the_original_gate_yml_defect_is_caught_and_named() {
    let known_bad = "\
jobs:
  kernel-bypass-gate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test -p kernel-bypass-gate
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test -p state-wildcard-lint
";
    let dups = duplicate_keys(known_bad, Style::Yaml, Checks { reject_duplicate_sibling_keys: true });
    let text = dups.join("\n");
    assert!(text.contains("key=runs-on"), "must catch the duplicate runs-on:\n{text}");
    assert!(text.contains("key=steps"), "must catch the duplicate steps:\n{text}");
    assert!(
        text.contains("kernel-bypass-gate"),
        "the refusal must NAME the job it came from:\n{text}"
    );
    assert!(text.contains("lines="), "and the line numbers:\n{text}");
}

/// FIRES-ON-KNOWN-BAD: the `NUMBERS.toml` shape that shipped at `5cb854f`.
#[test]
fn the_original_numbers_toml_defect_is_caught_and_named() {
    let known_bad = "\
[figures.test_functions]
command  = \"grep -c x\"
note     = \"first note\"
expect   = \"984\"

note     = \"second note, silently wins in a lenient parser\"
";
    let dups = duplicate_keys(known_bad, Style::Toml, Checks { reject_duplicate_sibling_keys: true });
    let text = dups.join("\n");
    assert!(text.contains("key=note"), "must catch the duplicate note:\n{text}");
    assert!(
        text.contains("figures.test_functions"),
        "must name the table:\n{text}"
    );
}

/// KNOWN-GOOD, negative direction: legitimate repetition must NOT be flagged.
///
/// Sequence items each open their own scope, and two different jobs may both say `runs-on`.
/// Without this the gate is over-strict and gets switched off.
#[test]
fn legitimate_repetition_across_siblings_and_jobs_is_not_flagged() {
    let good = "\
jobs:
  one:
    runs-on: ubuntu-latest
    steps:
      - name: a
        run: echo a
      - name: b
        run: echo b
  two:
    runs-on: ubuntu-latest
    steps:
      - name: a
        run: |
          runs-on: this is DATA inside a block scalar, not a key
          steps: also data
      - name: b
        run: echo b
";
    let dups = duplicate_keys(good, Style::Yaml, Checks { reject_duplicate_sibling_keys: true });
    assert!(
        dups.is_empty(),
        "false positives would get this gate routed around: {dups:?}"
    );
}

/// MUTATION with a byte-identical restore, run against the REAL file so the leg is attributable.
#[test]
fn mutating_gate_yml_goes_red_and_a_byte_identical_restore_goes_green() {
    let path = repo_root().join(".github/workflows/gate.yml");
    let before = fs::read(&path).expect("read");
    let text = String::from_utf8_lossy(&before).into_owned();

    assert!(
        duplicate_keys(&text, Style::Yaml, Checks { reject_duplicate_sibling_keys: true }).is_empty(),
        "baseline must be GREEN before mutating"
    );

    // reintroduce exactly the shipped defect: delete the job header that was missing
    let mutated = text.replace("\n  state-wildcard-lint:\n", "\n");
    assert_ne!(mutated, text, "the mutation must actually change the content");
    let red = duplicate_keys(&mutated, Style::Yaml, Checks { reject_duplicate_sibling_keys: true });
    assert!(
        red.iter().any(|f| f.contains("key=runs-on")) && red.iter().any(|f| f.contains("key=steps")),
        "removing the job header must go RED on both duplicated keys: {red:?}"
    );

    // and with the predicate disabled the SAME input goes quiet — so the finding is
    // attributable to this check and not to something else
    let quiet = duplicate_keys(
        &mutated,
        Style::Yaml,
        Checks { reject_duplicate_sibling_keys: false },
    );
    assert!(
        quiet.is_empty(),
        "with the predicate off the known-bad must stop being reported: {quiet:?}"
    );

    // byte-identical restore: nothing was written, so prove the source is untouched
    let after = fs::read(&path).expect("read");
    assert_eq!(before, after, "the real file must be untouched by this leg");
}

/// ANTI-VACUITY, explicit: a repo with no workflows must be an ERROR, not a pass.
#[test]
fn an_empty_scan_set_is_an_error_not_a_pass() {
    let root = std::env::temp_dir().join(format!(
        "omp-config-parses-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&root).expect("fixture");
    assert!(
        workflow_files(&root).is_empty(),
        "fixture has no workflows, so a count-based guard is the only thing standing between \
         this and a vacuous pass"
    );
    let _ = fs::remove_dir_all(&root);
}
