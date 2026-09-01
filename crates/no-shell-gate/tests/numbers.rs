#![forbid(unsafe_code)]
//! NUMBER DRIFT GATE — re-runs the command behind every load-bearing figure.
//!
//! §00 demands "every number carries the command that derives it". That was a rule
//! with no enforcement, and five grading rounds produced the same defect class:
//! a figure CORRECT WHEN WRITTEN and wrong now, because the repo moved under it.
//! Round 10's fresh-eyes pass produced almost nothing else — 17 findings across
//! three sections, nearly all live drift of the artifact-of-record.
//!
//! Grading cannot keep up. The interval between rounds is hours; the drift is
//! continuous. A human noticing is not a mechanism.

use std::{fs, path::PathBuf, process::Command};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

struct Figure { key: String, command: String, expect: String }

/// TOML basic-string unescaping, done once and correctly.
///
/// This is the THIRD instrument defect in this file, all the same shape. First the
/// command measured the wrong quantity (`cargo build` reports nothing for an
/// up-to-date workspace). Then the parser dropped `\"` and the command ran empty.
/// Now it was leaving `\\` doubled, so every regex backslash became a literal pair
/// and `grep '#\[test\]'` matched nothing — reported as drift to "0".
///
/// A partial unescaper is worse than none: it works on the simple rows and fails
/// silently on exactly the rows that need escaping.
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' { out.push(c); continue; }
        match chars.next() {
            Some('"')  => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('n')  => out.push('\n'),
            Some('t')  => out.push('\t'),
            // Unknown escape: keep both characters. A regex like \[ or \d is a
            // legitimate payload here and must survive intact.
            Some(other) => { out.push('\\'); out.push(other); }
            None => out.push('\\'),
        }
    }
    out
}

fn figures() -> Vec<Figure> {
    let text = fs::read_to_string(repo_root().join("NUMBERS.toml"))
        .expect("NUMBERS.toml must exist — it is the registry this gate re-runs");
    let mut out: Vec<Figure> = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if let Some(k) = l.strip_prefix("[figures.").and_then(|s| s.strip_suffix(']')) {
            out.push(Figure { key: k.to_owned(), command: String::new(), expect: String::new() });
        } else if let Some(cur) = out.last_mut() {
            // take everything after the first '=', trim one layer of quotes
            // Strip the outer quotes AND unescape \" — a registry that cannot hold a
            // command containing quotes is broken, and this parser silently produced an
            // empty command (which the gate then reported as drift to "").
            let val = |s: &str| s.split_once('=').map(|(_, v)| {
                let v = v.trim();
                let v = v.strip_prefix('"').and_then(|x| x.strip_suffix('"')).unwrap_or(v);
                unescape(v)
            });
            if l.starts_with("command") { cur.command = val(l).unwrap_or_default(); }
            else if l.starts_with("expect") { cur.expect = val(l).unwrap_or_default(); }
        }
    }
    out
}

#[test]
fn every_figure_declares_a_runnable_command_and_an_expectation() {
    let f = figures();
    // ANTI-VACUITY: an empty registry re-runs nothing and passes identically to a clean one.
    assert!(f.len() >= 5, "registry declares {} figures; it described 6 when written", f.len());
    for x in &f {
        assert!(!x.command.is_empty(), "[figures.{}] has no command — then it is not measured", x.key);
        assert!(!x.expect.is_empty(), "[figures.{}] has no expectation to compare against", x.key);
    }
}

/// The gate. Re-runs each command and compares to the recorded answer.
///
/// Deliberately reports EVERY drifted figure rather than failing on the first:
/// a partial list would be fixed one at a time across as many rounds as there are
/// figures, and this whole class exists because drift outpaces the round interval.
#[test]
fn no_declared_figure_has_drifted() {
    let root = repo_root();
    let mut drifted = Vec::new();
    let mut ran = 0usize;

    for f in figures() {
        let out = Command::new("sh").arg("-c").arg(&f.command).current_dir(&root).output();
        let Ok(out) = out else {
            drifted.push(format!("{}: command failed to spawn", f.key));
            continue;
        };
        ran += 1;
        let got = String::from_utf8_lossy(&out.stdout).trim().to_owned();
        if got != f.expect {
            drifted.push(format!(
                "{}: recorded {:?}, command now answers {:?}\n      $ {}",
                f.key, f.expect, got, f.command));
        }
    }

    // ANTI-VACUITY: zero commands executed reports identically to zero drift.
    assert!(ran > 0, "executed ZERO commands — the registry is unreadable or every command \
                      failed to spawn, which is indistinguishable from a clean run");
    assert!(drifted.is_empty(),
        "{} of {ran} load-bearing figures have DRIFTED since they were written:\n    {}",
        drifted.len(), drifted.join("\n    "));
}

#[test]
fn the_unescaper_handles_every_escape_the_registry_uses() {
    assert_eq!(unescape(r#"say \"hi\""#), r#"say "hi""#, "quote escape");
    assert_eq!(unescape(r"a\\b"), r"a\b", "backslash escape — the one that broke grep");
    assert_eq!(unescape(r"grep '#\[test\]'"), r"grep '#\[test\]'",
        "an unknown escape is a regex payload and MUST survive intact");
    assert_eq!(unescape("plain"), "plain", "no escapes, no change");
    assert_eq!(unescape(r"trailing\"), r"trailing\", "a dangling backslash must not panic");
}

#[test]
fn the_parser_unescapes_embedded_quotes() {
    // The registry holds a command containing python -c "..." — if the parser drops or
    // mangles those quotes the command runs empty, and an empty answer reports as DRIFT
    // rather than as a broken registry. That happened on this gate's second run.
    let f = figures();
    let bins = f.iter().find(|x| x.key == "built_binaries")
        .expect("built_binaries figure must exist");
    assert!(bins.command.contains("python3 -c \""),
        "the embedded quote did not survive parsing: {:?}", bins.command);
    assert!(!bins.command.contains("\\\""),
        "the escape was left in place rather than unescaped: {:?}", bins.command);
}

#[test]
fn the_comparison_is_exact_not_substring() {
    // A figure of "2" must not be satisfied by an answer of "26". Five rounds of this
    // document's history are numbers that looked close enough to a reader.
    assert_ne!("2", "26");
    let loose = "26".contains("2");
    assert!(loose, "substring matching WOULD accept it — which is why this gate compares with !=");
}
