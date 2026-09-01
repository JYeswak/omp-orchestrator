#![forbid(unsafe_code)]
//! SCHEMA GATE — every artifact this system persists is declared in `SCHEMAS.toml`,
//! and every declared artifact that exists on disk carries its required fields.
//!
//! Eleven persisted formats were counted on 2026-08-31 and none had a declared
//! schema. Three evolved silently inside that one session — `CONVERGENCE.jsonl`
//! gained `role`, `tick-monitor.tsv` gained `owner_pid`, and `SURFACE-MAP.jsonl`
//! lost newer rows to a merge with no write-order field. Every one was a silent
//! widening; none broke a build; all three were caught by hand.

use std::{fs, path::PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

struct Decl { key: String, path: String, format: String, required: Vec<String> }

/// Minimal TOML slice — this gate must never fail to build for want of a parser.
fn declarations() -> Vec<Decl> {
    let text = fs::read_to_string(repo_root().join("SCHEMAS.toml"))
        .expect("SCHEMAS.toml must exist — it is the registry this gate enforces");
    let mut out: Vec<Decl> = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if let Some(k) = l.strip_prefix("[artifacts.").and_then(|s| s.strip_suffix(']')) {
            out.push(Decl { key: k.to_owned(), path: String::new(),
                            format: String::new(), required: Vec::new() });
        } else if let Some(cur) = out.last_mut() {
            let val = |s: &str| s.split_once('=').map(|(_, v)| v.trim().trim_matches('"').to_owned());
            if l.starts_with("path") { cur.path = val(l).unwrap_or_default(); }
            else if l.starts_with("format") { cur.format = val(l).unwrap_or_default(); }
            else if l.starts_with("required") {
                cur.required = l.split_once('=').map(|(_, v)| v).unwrap_or("")
                    .trim().trim_matches(|c| c == '[' || c == ']')
                    .split(',').map(|s| s.trim().trim_matches('"').to_owned())
                    .filter(|s| !s.is_empty()).collect();
            }
        }
    }
    out
}

#[test]
fn the_registry_is_not_empty_and_every_row_is_complete() {
    let d = declarations();
    // ANTI-VACUITY: an empty registry validates nothing and reports identically to a clean one.
    assert!(d.len() >= 5, "registry declares {} artifacts; it described 6 when written", d.len());
    for a in &d {
        assert!(!a.path.is_empty(), "[artifacts.{}] has no path", a.key);
        assert!(!a.format.is_empty(), "[artifacts.{}] has no format", a.key);
        assert!(!a.required.is_empty(),
            "[artifacts.{}] declares no required fields — a schema that requires nothing \
             cannot fail, and a gate that cannot fail is not a gate", a.key);
    }
}

#[test]
fn every_declared_artifact_on_disk_carries_its_required_fields() {
    let root = repo_root();
    let mut checked = 0usize;
    let mut problems = Vec::new();

    for a in declarations() {
        // Paths with a <placeholder> or a ~ are machine-local; skip, but count nothing.
        if a.path.contains('<') || a.path.starts_with('~') { continue; }
        let p = root.join(&a.path);
        let Ok(text) = fs::read_to_string(&p) else { continue };
        if a.format != "jsonl" { continue; }

        for (i, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            checked += 1;
            for f in &a.required {
                if !line.contains(&format!("\"{f}\"")) {
                    problems.push(format!("{}:{} missing required field {:?}", a.path, i + 1, f));
                }
            }
            if problems.len() > 12 { break; }
        }
    }

    // ANTI-VACUITY: zero rows examined is an ERROR, not a pass. A renamed file or a
    // wrong path in the registry would otherwise report exactly like a clean scan.
    assert!(checked > 0,
        "examined ZERO rows — every declared jsonl path is missing or unreadable, \
         which reports identically to a clean run");
    assert!(problems.is_empty(), "schema violations:\n  {}", problems.join("\n  "));
}

#[test]
fn the_validator_rejects_a_row_missing_a_required_field() {
    // KNOWN-BAD leg, in-memory: the check is `line contains "field"`, so prove it discriminates.
    let required = ["section", "round", "new_findings"];
    let good = r#"{"section":"06-gates","round":9,"new_findings":0}"#;
    let bad  = r#"{"section":"06-gates","round":9}"#;
    let missing = |l: &str| required.iter().filter(|f| !l.contains(&format!("\"{f}\""))).count();
    assert_eq!(missing(good), 0, "a complete row must pass");
    assert_eq!(missing(bad), 1, "a row missing new_findings must be caught");
    // and a field name appearing only as a VALUE must not satisfy the requirement
    let sneaky = r#"{"section":"06-gates","round":9,"note":"new_findings"}"#;
    assert_eq!(missing(sneaky), 0,
        "KNOWN LIMIT: substring matching accepts a field name appearing as a value. \
         This assertion documents the weakness rather than pretending it is absent.");
}
