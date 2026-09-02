#![forbid(unsafe_code)]

//! EXIT CODE REGISTRY GATE — re-derives every emitted exit code from source and refuses
//! any code that `docs/error_codes/exit_code_registry.md` does not document.
//!
//! # Why a gate and not a snapshot
//!
//! Four exit codes were misread by agents on 2026-09-01, and the reason was not
//! carelessness: a refusal and a failure are indistinguishable from outside when the only
//! signal is a small integer. The registry's cure is a `does NOT mean` column. The cure's
//! own failure mode is rot — a table that was true at one commit and silently wrong at the
//! next, which is exactly what `NUMBERS.toml` was written to stop for figures.
//!
//! So this suite derives the emission set from disk on every run. A new
//! `ExitCode::from(213)` anywhere under `crates/*/src` fails the build naming `file:line`.
//!
//! # What it cannot do
//!
//! It proves PRESENCE OF A ROW, never truth of a row. A wrong `does NOT mean` cell passes
//! every leg here. It cannot tell a correct `1` from a wrong `1`, because both are `1` —
//! that is the defect `XC-001` names and this gate does not fix. It also sees only what it
//! scans: a code from a shell wrapper, a build script, or a crate outside `crates/*/src` is
//! invisible to it.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

/// Floors seeded from THIS suite's own scan of the workspace at commit `d48615c`:
/// 123 `.rs` files under `crates/*/src`, 12 distinct emitted codes, 7 pass-through sites.
///
/// Seeded from the scan, NEVER from a neighbouring count. A ceiling seeded from
/// `cargo metadata` while the gate counted structurally let a mutation probe pass on
/// 2026-09-01 when it should have failed — two instruments, one denominator, and the
/// disagreement was invisible. These floors sit below the measurement so ordinary
/// extraction does not trip them, and far enough above zero that a broken walker does.
const FILE_FLOOR: usize = 100;
const CODE_FLOOR: usize = 10;
const PASSTHROUGH_FLOOR: usize = 5;

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate must live beneath the workspace root")
        .to_path_buf()
}

fn registry_path(root: &Path) -> PathBuf {
    root.join("docs/error_codes/exit_code_registry.md")
}

// ---------------------------------------------------------------------------------------
// The scanner
// ---------------------------------------------------------------------------------------

/// One emission site: the code, and where it is written.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Emission {
    code: u16,
    file: String,
    line: usize,
}

/// One pass-through site: a CHILD process's code forwarded as our own, so the code space at
/// that site is 0..=255 of whatever ran underneath.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PassThrough {
    expression: String,
    crate_name: String,
}

/// Every `.rs` file beneath `<root>/crates/*/src`, recursively.
///
/// The scan set must be at least as wide as the patterns run over it. The first hand-built
/// census derived its file list with `grep -l 'ExitCode|process::exit'`, which returned 53
/// files and silently omitted `crates/fleet-monitor/src/lib.rs` — the file that declares
/// `EXIT_CANNOT_OBSERVE = 78` and contains neither token. Hence: no pattern filter here.
fn source_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let crates_dir = root.join("crates");
    let Ok(entries) = fs::read_dir(&crates_dir) else {
        return out;
    };
    let mut crate_dirs: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    crate_dirs.sort();
    for c in crate_dirs {
        walk_rs(&c.join("src"), &mut out);
    }
    out.sort();
    out
}

fn walk_rs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries.flatten().map(|e| e.path()).collect();
    paths.sort();
    for p in paths {
        if p.is_dir() {
            walk_rs(&p, out);
        } else if p.extension().is_some_and(|e| e == "rs") {
            out.push(p);
        }
    }
}

/// Digits immediately following `needle`, when the call is `needle<digits>)`.
fn literal_after(line: &str, needle: &str) -> Vec<u16> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(i) = rest.find(needle) {
        let tail = &rest[i + needle.len()..];
        let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
        if !digits.is_empty() && tail[digits.len()..].starts_with(')') {
            if let Ok(n) = digits.parse::<u16>() {
                out.push(n);
            }
        }
        rest = tail;
    }
    out
}

/// The balanced-paren argument of `needle`, or None when unbalanced.
fn call_argument(line: &str, needle: &str) -> Option<String> {
    let i = line.find(needle)?;
    let tail = &line[i + needle.len()..];
    let mut depth = 1usize;
    let mut end = None;
    for (k, ch) in tail.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(k);
                    break;
                }
            }
            _ => {}
        }
    }
    Some(tail[..end?].to_owned())
}

/// A bare identifier chain — `code`, `out.code`, `v.exit_code()` — which is what a
/// pass-through looks like, returned in CANONICAL form with any `as uN` cast stripped.
/// Uppercase (a named `EXIT_*` const), `::` paths, `&`, and any call carrying arguments are
/// deliberately excluded: those are our own values, not a forwarded child code.
///
/// # Why this returns the normalized string rather than a bool
///
/// The first version answered `is_identifier_chain(&arg) -> bool` and then recorded the RAW
/// argument, so `ExitCode::from(verdict.exit as u8)` was recorded as
/// `"verdict.exit as u8"` and never matched the registry's `verdict.exit`. The leg failed on
/// all 18 sites — which is the pass-through leg proving on its first run that it is not
/// vacuous, at the cost of proving it against my own defect. Normalizing at the point of
/// recognition makes the mismatch unconstructible: there is one function, and it returns the
/// only spelling any caller can see.
fn passthrough_chain(expr: &str) -> Option<String> {
    let e = expr.trim();
    let e = e
        .strip_suffix(" as u8")
        .or_else(|| e.strip_suffix(" as u16"))
        .or_else(|| e.strip_suffix(" as i32"))
        .or_else(|| e.strip_suffix(" as u32"))
        .unwrap_or(e)
        .trim();
    let bare = e.strip_suffix("()").unwrap_or(e);
    if bare.is_empty() {
        return None;
    }
    let first = bare.chars().next()?;
    if !(first.is_ascii_lowercase() || first == '_') {
        return None;
    }
    let shaped = bare
        .chars()
        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '.');
    if shaped { Some(e.to_owned()) } else { None }
}

/// `const EXIT_NAME: uN = 78;` -> (name, value).
fn const_declaration(line: &str) -> Option<(String, u16)> {
    let i = line.find("const EXIT_")?;
    let tail = &line[i + "const ".len()..];
    let colon = tail.find(':')?;
    let name = tail[..colon].trim().to_owned();
    let eq = tail.find('=')?;
    let digits: String = tail[eq + 1..]
        .trim_start()
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    if digits.is_empty() {
        return None;
    }
    Some((name, digits.parse().ok()?))
}

/// Integers returned from an `fn exit_code(...)` body, brace-matched.
///
/// This is how `XC-070` is reachable at all: `omp-orchestrator` returns 70 from
/// `if evidence.is_empty() { 70 } else { 0 }`, with no `ExitCode::from` anywhere near it.
fn exit_code_body_values(text: &str) -> Vec<u16> {
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(i) = text[from..].find("fn exit_code(") {
        let start = from + i;
        let Some(open) = text[start..].find('{') else {
            break;
        };
        let body_start = start + open;
        let mut depth = 0usize;
        let mut body_end = text.len();
        for (k, ch) in text[body_start..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        body_end = body_start + k;
                        break;
                    }
                }
                _ => {}
            }
        }
        let body = &text[body_start..body_end];
        let bytes = body.as_bytes();
        let mut k = 0usize;
        while k < bytes.len() {
            if bytes[k].is_ascii_digit() {
                let s = k;
                while k < bytes.len() && bytes[k].is_ascii_digit() {
                    k += 1;
                }
                let prev_ok = s == 0 || !(bytes[s - 1] as char).is_alphanumeric();
                let next_ok = k >= bytes.len() || !(bytes[k] as char).is_alphanumeric();
                if prev_ok && next_ok {
                    if let Ok(n) = body[s..k].parse::<u16>() {
                        out.push(n);
                    }
                }
            } else {
                k += 1;
            }
        }
        from = body_end.max(start + 1);
    }
    out
}

/// The full derived surface: emissions, named constants, and pass-through sites.
struct Scan {
    files: usize,
    emissions: Vec<Emission>,
    consts: BTreeMap<String, BTreeSet<u16>>,
    passthrough: Vec<PassThrough>,
}

impl Scan {
    fn codes(&self) -> BTreeSet<u16> {
        let mut s: BTreeSet<u16> = self.emissions.iter().map(|e| e.code).collect();
        for values in self.consts.values() {
            s.extend(values.iter().copied());
        }
        s
    }
}

fn scan(root: &Path) -> Scan {
    let files = source_files(root);
    let mut emissions = Vec::new();
    let mut consts: BTreeMap<String, BTreeSet<u16>> = BTreeMap::new();
    let mut passthrough = Vec::new();
    for path in &files {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        let crate_name = rel
            .split('/')
            .nth(1)
            .unwrap_or("<unknown>")
            .to_owned();
        for value in exit_code_body_values(&text) {
            emissions.push(Emission { code: value, file: rel.clone(), line: 0 });
        }
        for (n, line) in text.lines().enumerate() {
            let lineno = n + 1;
            for needle in ["ExitCode::from(", "process::exit("] {
                for code in literal_after(line, needle) {
                    emissions.push(Emission { code, file: rel.clone(), line: lineno });
                }
                if let Some(chain) =
                    call_argument(line, needle).as_deref().and_then(passthrough_chain)
                {
                    passthrough.push(PassThrough {
                        expression: chain,
                        crate_name: crate_name.clone(),
                    });
                }
            }
            if let Some(rest) = line.split("exit:").nth(1) {
                let digits: String = rest
                    .trim_start()
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .collect();
                if !digits.is_empty() {
                    if let Ok(code) = digits.parse::<u16>() {
                        emissions.push(Emission { code, file: rel.clone(), line: lineno });
                    }
                }
            }
            if let Some((name, value)) = const_declaration(line) {
                consts.entry(name).or_default().insert(value);
            }
        }
    }
    Scan { files: files.len(), emissions, consts, passthrough }
}

// ---------------------------------------------------------------------------------------
// The registry reader
// ---------------------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct Row {
    id: String,
    cells: Vec<String>,
}

/// Every markdown row whose first cell is a backticked `XC-*` id.
fn registry_rows(doc: &str) -> Vec<Row> {
    let mut out = Vec::new();
    for line in doc.lines() {
        let t = line.trim();
        if !t.starts_with('|') {
            continue;
        }
        let cells: Vec<String> = t
            .trim_matches('|')
            .split('|')
            .map(|c| c.trim().to_owned())
            .collect();
        let Some(first) = cells.first() else { continue };
        let id = first.trim_matches('`').to_owned();
        if id.starts_with("XC-") {
            out.push(Row { id, cells });
        }
    }
    out
}

/// The codes the registry claims THIS WORKSPACE emits — `XC-EXT-*` rows are foreign
/// receipts and `XC-PT-*` rows are pass-through declarations, so neither counts.
fn documented_emitted(doc: &str) -> BTreeMap<u16, Row> {
    let mut out = BTreeMap::new();
    for row in registry_rows(doc) {
        if row.id.starts_with("XC-EXT-") || row.id.starts_with("XC-PT-") {
            continue;
        }
        if let Some(code) = row.cells.get(1).and_then(|c| c.trim().parse::<u16>().ok()) {
            out.insert(code, row);
        }
    }
    out
}

fn documented_passthrough(doc: &str) -> BTreeSet<String> {
    registry_rows(doc)
        .into_iter()
        .filter(|r| r.id.starts_with("XC-PT-"))
        .filter_map(|r| r.cells.get(1).map(|c| c.trim_matches('`').trim().to_owned()))
        .collect()
}

/// The check itself, so every leg exercises the same code path.
fn undocumented(scan: &Scan, doc: &str) -> Result<Vec<String>, String> {
    if scan.files == 0 {
        return Err("EXIT_SCAN_EMPTY: zero source files scanned. An empty scan set is an \
                    ERROR, never a pass — a code that was never looked for reports \
                    identically to one that has a row."
            .to_owned());
    }
    let known = documented_emitted(doc);
    if known.is_empty() {
        return Err("EXIT_REGISTRY_EMPTY: the registry parsed to zero emitted rows".to_owned());
    }
    let mut complaints = Vec::new();
    let mut seen = BTreeSet::new();
    for e in &scan.emissions {
        if !known.contains_key(&e.code) && seen.insert(e.code) {
            complaints.push(format!(
                "EXIT_CODE_UNDOCUMENTED code={} at {}:{} — add an XC-* row",
                e.code, e.file, e.line
            ));
        }
    }
    for (name, values) in &scan.consts {
        for v in values {
            if !known.contains_key(v) && seen.insert(*v) {
                complaints.push(format!(
                    "EXIT_CODE_UNDOCUMENTED code={v} declared as {name} — add an XC-* row"
                ));
            }
        }
    }
    Ok(complaints)
}

// ---------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------

/// A temporary workspace shaped like ours, carrying exactly one emission.
fn fixture(tag: &str, body: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "omp-exit-codes-{}-{}-{tag}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let src = root.join("crates").join("planted").join("src");
    fs::create_dir_all(&src).expect("fixture dirs");
    fs::write(src.join("main.rs"), body).expect("fixture source");
    root
}

// ---------------------------------------------------------------------------------------
// Legs
// ---------------------------------------------------------------------------------------

#[test]
fn every_emitted_exit_code_is_documented() {
    let root = repo_root();
    let doc = fs::read_to_string(registry_path(&root))
        .expect("docs/error_codes/exit_code_registry.md must exist — it is what this gate reads");
    let s = scan(&root);
    let complaints = undocumented(&s, &doc).expect("the real scan must not be vacuous");
    assert!(
        complaints.is_empty(),
        "{} undocumented exit code(s):\n{}",
        complaints.len(),
        complaints.join("\n")
    );
}

#[test]
fn an_empty_scan_set_is_an_error_not_a_pass() {
    let empty = fixture("empty-set", "// no emissions here\n");
    // Point the scan at a root with no `crates/*/src/*.rs` at all.
    let barren = empty.join("nothing");
    fs::create_dir_all(&barren).expect("barren dir");
    let s = scan(&barren);
    assert_eq!(s.files, 0, "the barren root must scan zero files");
    let doc = fs::read_to_string(registry_path(&repo_root())).expect("registry");
    let verdict = undocumented(&s, &doc);
    assert!(
        verdict.is_err_and(|e| e.starts_with("EXIT_SCAN_EMPTY")),
        "an empty scan set must be a NAMED error, not an empty complaint list"
    );
    let _ = fs::remove_dir_all(&empty);
}

#[test]
fn a_planted_undocumented_code_is_caught() {
    let root = fixture(
        "known-bad",
        "fn main() -> std::process::ExitCode { std::process::ExitCode::from(213) }\n",
    );
    let s = scan(&root);
    assert_eq!(s.files, 1, "the fixture must be seen: positive control on the walker");
    let doc = fs::read_to_string(registry_path(&repo_root())).expect("registry");
    let complaints = undocumented(&s, &doc).expect("fixture scan is not vacuous");
    assert!(
        complaints.iter().any(|c| c.contains("code=213")),
        "the gate did not fire on a planted undocumented code; complaints were {complaints:?}"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_planted_documented_code_passes() {
    // The known-GOOD leg. Without it this gate could be over-strict and still look green,
    // and an over-strict gate gets routed around — a slower death than no gate.
    let root = fixture(
        "known-good",
        "fn main() -> std::process::ExitCode { std::process::ExitCode::from(2) }\n",
    );
    let s = scan(&root);
    assert_eq!(s.files, 1);
    let doc = fs::read_to_string(registry_path(&repo_root())).expect("registry");
    let complaints = undocumented(&s, &doc).expect("fixture scan is not vacuous");
    assert!(
        complaints.is_empty(),
        "a documented code must pass; got {complaints:?}"
    );
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn every_row_carries_a_does_not_mean() {
    let doc = fs::read_to_string(registry_path(&repo_root())).expect("registry");
    let rows: Vec<Row> = registry_rows(&doc)
        .into_iter()
        .filter(|r| !r.id.starts_with("XC-PT-"))
        .collect();
    assert!(
        rows.len() >= 12,
        "the registry must carry at least the 12 emitted codes; found {}",
        rows.len()
    );
    for row in &rows {
        assert_eq!(
            row.cells.len(),
            6,
            "{} has {} cells, expected 6 (id, code, emitters, means, does-not-mean, action)",
            row.id,
            row.cells.len()
        );
        // A LENGTH GATE IS A PROXY, AND SAYING SO IS THE POINT. This cannot check that a
        // `does NOT mean` cell is TRUE — only that someone wrote a sentence rather than a
        // dash. The threshold was 20 and refused `XC-EXT-126`'s honest "that it is missing"
        // (18 chars), which is the over-strict failure the known-good leg exists to expose:
        // an over-strict gate gets routed around, a slower death than no gate. Floor is now
        // 10 plus an explicit placeholder list, because a placeholder is the thing a real
        // sentence never is.
        const PLACEHOLDERS: &[&str] = &["", "-", "—", "n/a", "na", "tbd", "todo", "none", "?"];
        let dnm = &row.cells[4];
        let folded = dnm.trim().to_ascii_lowercase();
        assert!(
            !PLACEHOLDERS.contains(&folded.as_str()) && dnm.trim().len() >= 10,
            "{} has an empty or placeholder `does NOT mean` cell: {dnm:?}. That column is the \
             load-bearing one — it is what would have stopped all four misreads.",
            row.id
        );
        assert!(
            row.cells[5].len() >= 8,
            "{} names no operator action",
            row.id
        );
    }
}

#[test]
fn the_scan_floors_seeded_from_this_suites_own_measurement_hold() {
    let s = scan(&repo_root());
    assert!(
        s.files >= FILE_FLOOR,
        "scanned {} files, floor {FILE_FLOOR}: the walker is broken or the workspace shrank \
         drastically",
        s.files
    );
    let codes = s.codes();
    assert!(
        codes.len() >= CODE_FLOOR,
        "derived {} distinct codes {:?}, floor {CODE_FLOOR}: a pattern stopped matching",
        codes.len(),
        codes
    );
    assert!(
        s.passthrough.len() >= PASSTHROUGH_FLOOR,
        "derived {} pass-through sites, floor {PASSTHROUGH_FLOOR}",
        s.passthrough.len()
    );
}

#[test]
fn every_pass_through_site_is_declared() {
    let root = repo_root();
    let doc = fs::read_to_string(registry_path(&root)).expect("registry");
    let declared = documented_passthrough(&doc);
    assert!(
        !declared.is_empty(),
        "the registry declares no pass-through sites; §6 must not be empty"
    );
    let s = scan(&root);
    let mut missing: Vec<String> = Vec::new();
    for pt in &s.passthrough {
        if !declared.contains(&pt.expression) && !missing.contains(&pt.expression) {
            missing.push(format!("{} (in {})", pt.expression, pt.crate_name));
        }
    }
    assert!(
        missing.is_empty(),
        "undeclared pass-through site(s) — a foreign code can arrive wearing our name here: \
         {missing:?}"
    );
}
