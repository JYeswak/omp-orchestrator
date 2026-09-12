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

/// One site that forwards a code it did not choose as its own.
///
/// The name says CHILD PROCESS and that is the case it was built for, but the recogniser
/// cannot actually establish it. [`passthrough_chain`] accepts any bare identifier chain,
/// so a local value reached through a field access (`outcome.code`) or a zero-argument
/// call to our OWN function (`selftest()`) is indistinguishable here from `out.code`
/// holding a child's status. Both are recorded, and the registry row is where the
/// distinction is made explicit — §6 already carves out `XC-PT-VERDICT` and
/// `XC-PT-EXITCODE` on exactly that ground. So read a row here as "this site forwards a
/// code chosen elsewhere", and read its registry row for whether "elsewhere" is inside
/// this workspace.
///
/// `file` and `line` are carried because the message is the deliverable. Without them a
/// crate with four sites for one expression printed the same string four times, which
/// tells a reader the count and nothing they can act on.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PassThrough {
    expression: String,
    crate_name: String,
    file: String,
    line: usize,
}

/// One exit-path integer narrowing cast derived by this gate's own scanner.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct NarrowingCast {
    expression: String,
    target: String,
    file: String,
    line: usize,
    /// The enclosing `fn` name, and the occurrence index of this (function, target)
    /// pair within it. THE KEY IS A SHAPE, NOT A POSITION
    /// (`omp-orchestrator-mg9ua`): a line number rots on every insertion ABOVE the
    /// site, and the row it breaks describes a cast that never moved relative to its
    /// own function. Measured twice in two days on ONE row (`:853 -> :1146 -> :1153`).
    /// A shape survives an insertion above it and a rename beside it; a line number
    /// survives neither.
    function: String,
    occurrence: usize,
}

/// The enclosing `fn` name for a line, from the nearest preceding `fn` at any depth.
///
/// Comments are stripped BEFORE the scan (`text_structure::code_only`) because this
/// repository has measured a doc comment containing the needle it documents and
/// defeating the mitigation it described. A doc comment naming `fn whatever` is not a
/// function definition.
fn enclosing_functions(code: &str) -> Vec<String> {
    let mut current = String::new();
    let mut out = Vec::new();
    for line in code.lines() {
        let trimmed = line.trim_start();
        for prefix in ["fn ", "pub fn ", "pub(crate) fn ", "async fn ", "pub async fn "] {
            if let Some(rest) = trimmed.strip_prefix(prefix) {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() {
                    current = name;
                }
                break;
            }
        }
        out.push(current.clone());
    }
    out
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

/// One line with `//` comments removed and string-literal CONTENTS blanked.
///
/// # Why this exists, measured 2026-09-02
///
/// The scanner ran the emission patterns over raw source, so it counted its own
/// documentation. Adding a doc comment reading
/// `` `ExitCode::from(selftest() as u8)` `` to `cargo-lane-budget` and an assertion
/// message quoting the same call created TWO new pass-through "sites" out of prose, and
/// the registry was then edited to cite `src/lib.rs:943,967` — documenting comments as
/// emission sites. Tree-wide there were exactly two such hits and both were introduced
/// by that one edit, which is why the blind spot survived until something wrote about
/// the gate inside a file the gate reads.
///
/// The sharper case is worse than an inflated count: a comment reading
/// `ExitCode::from(213)` would make `every_emitted_exit_code_is_documented` demand a
/// registry row for a code nothing emits, and the only way to satisfy it would be to
/// document a fiction. `a_code_that_appears_only_in_prose_is_not_an_emission` pins both
/// directions.
///
/// # Which shared helper, measured 2026-09-11
///
/// This wrapper called `text_structure::code_only`, which masks comments and DELIBERATELY
/// KEEPS string literals — its own doc says to use it "when string literals are themselves
/// the evidence being measured", which is the `oracle_routing` case (argv literals ARE the
/// surface signal) and the exact opposite of this one. So the half of the defect above that
/// lives in a STRING survived the fix that named it: `crates/cargo-lane-budget/src/lib.rs:989`
/// quotes `ExitCode::from(selftest() as u8)` inside an assertion message and was scored as a
/// live narrowing cast, and `crates/ompo-doctor/src/health_repair.rs:418-423` cites
/// `adapter_exec.rs:138-142` and `:306-307` inside a `fn exit_code` body, which made
/// `every_emitted_exit_code_is_documented` demand registry rows for codes 138, 142, 306 and
/// 307 — four fictions, and 306/307 are not even representable in the `u8` that body returns.
/// `text_structure::code_and_literals` is the helper for a scanner whose evidence is code and
/// never prose; it blanks comment and literal bodies while preserving bytes and lines, so
/// `call_argument`'s paren balancing still sees a well-formed line. No second parser is
/// written here: both helpers already exist and this picks the correct one.
///
/// Multi-line `/* */` blocks ARE handled by the shared helper when it is given whole-file
/// text; given one line at a time, as the emission loop does, an unterminated block comment
/// masks only to end of line. No such block in `crates/*/src` contains an exit pattern.
fn code_without_literals(line: &str) -> String {
    text_structure::code_and_literals(line)
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
    if shaped {
        Some(e.to_owned())
    } else {
        None
    }
}

/// Return an integer cast in an exit argument, with source expression and target type.
fn narrowing_cast(expr: &str) -> Option<(String, String)> {
    let expression = expr.trim();
    for target in [
        "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128",
        "isize",
    ] {
        let suffix = format!(" as {target}");
        if let Some(source) = expression.strip_suffix(&suffix) {
            let source = source.trim();
            if !source.is_empty() {
                return Some((source.to_owned(), target.to_owned()));
            }
        }
    }
    None
}

/// const EXIT_NAME: uN = 78; -> (name, value).
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
    narrowing: Vec<NarrowingCast>,
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
    let mut narrowing = Vec::new();
    for path in &files {
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();
        let crate_name = rel.split('/').nth(1).unwrap_or("<unknown>").to_owned();
        // Whole-file masking, not per-line: an `fn exit_code` body is brace-matched across
        // lines, and the prose inside one is where 138, 142, 306 and 307 came from.
        let masked = code_without_literals(&text);
        for value in exit_code_body_values(&masked) {
            emissions.push(Emission {
                code: value,
                file: rel.clone(),
                line: 0,
            });
        }
        // THE SHAPE KEY'S INPUTS, computed once per file: the enclosing function per line
        // (comments stripped first) and a per-(function,target) occurrence counter, so a row
        // survives an insertion above its site (`omp-orchestrator-mg9ua`).
        let functions = enclosing_functions(&code_without_literals(&text));
        let mut seen_in_function: BTreeMap<(String, String), usize> = BTreeMap::new();
        for (n, raw_line) in text.lines().enumerate() {
            let lineno = n + 1;
            // Prose is not an emission. See `code_without_literals`.
            let stripped = code_without_literals(raw_line);
            let line = stripped.as_str();
            for needle in ["ExitCode::from(", "process::exit("] {
                for code in literal_after(line, needle) {
                    emissions.push(Emission {
                        code,
                        file: rel.clone(),
                        line: lineno,
                    });
                }
                let argument = call_argument(line, needle);
                if let Some((expression, target)) = argument.as_deref().and_then(narrowing_cast) {
                    let function = functions.get(n).cloned().unwrap_or_default();
                    let counter = seen_in_function
                        .entry((function.clone(), target.clone()))
                        .or_insert(0);
                    let occurrence = *counter;
                    *counter += 1;
                    narrowing.push(NarrowingCast {
                        expression,
                        target,
                        file: rel.clone(),
                        line: lineno,
                        function,
                        occurrence,
                    });
                }
                if let Some(chain) = argument.as_deref().and_then(passthrough_chain) {
                    passthrough.push(PassThrough {
                        expression: chain,
                        crate_name: crate_name.clone(),
                        file: rel.clone(),
                        line: lineno,
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
                        emissions.push(Emission {
                            code,
                            file: rel.clone(),
                            line: lineno,
                        });
                    }
                }
            }
            if let Some((name, value)) = const_declaration(line) {
                consts.entry(name).or_default().insert(value);
            }
        }
    }
    Scan {
        files: files.len(),
        emissions,
        consts,
        passthrough,
        narrowing,
    }
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
        .filter_map(|r| {
            r.cells
                .get(1)
                .map(|c| c.trim_matches('`').trim().to_owned())
        })
        .collect()
}

/// The check itself, so every leg exercises the same code path.
fn undocumented(scan: &Scan, doc: &str) -> Result<Vec<String>, String> {
    if scan.files == 0 {
        return Err(
            "EXIT_SCAN_EMPTY: zero source files scanned. An empty scan set is an \
                    ERROR, never a pass — a code that was never looked for reports \
                    identically to one that has a row."
                .to_owned(),
        );
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
    assert_eq!(
        s.files, 1,
        "the fixture must be seen: positive control on the walker"
    );
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

/// FIRES ON KNOWN-BAD, both directions: prose is not an emission, and code still is.
///
/// The pair is the point. A leg that only asserted the comment is ignored would also pass
/// if `code_without_literals` deleted the whole line, and the gate would then see nothing —
/// which is the vacuity this suite exists to refuse. So the SAME undocumented code is
/// planted twice, once in a comment and once as a statement, and the verdicts must differ.
///
/// `213` is chosen because no `XC-*` row documents it, so a false positive is visible as a
/// complaint rather than being absorbed by an existing row.
#[test]
fn a_code_that_appears_only_in_prose_is_not_an_emission() {
    let doc = fs::read_to_string(registry_path(&repo_root())).expect("registry");

    let prose = fixture(
        "prose-only",
        "/// Historically this returned `std::process::ExitCode::from(213)`.\n\
         // and `process::exit(213)` before that\n\
         fn main() -> std::process::ExitCode {\n    \
             let note = \"we used to std::process::ExitCode::from(213) here\";\n    \
             let _ = note;\n    \
             std::process::ExitCode::from(2)\n\
         }\n",
    );
    let s = scan(&prose);
    assert_eq!(s.files, 1, "the fixture must actually be scanned");
    let complaints = undocumented(&s, &doc).expect("fixture scan is not vacuous");
    assert!(
        complaints.is_empty(),
        "a code named only in a comment or a string literal is NOT an emission, yet the \
         scanner complained: {complaints:?}. A gate that reads its own documentation as \
         source demands a registry row for a code nothing emits, and the only way to \
         satisfy it is to document a fiction."
    );
    assert!(
        s.passthrough.is_empty(),
        "and prose must not manufacture pass-through sites either; got {:?}",
        s.passthrough
    );
    let _ = fs::remove_dir_all(&prose);

    let real = fixture(
        "prose-control",
        "fn main() -> std::process::ExitCode { std::process::ExitCode::from(213) }\n",
    );
    let s = scan(&real);
    let complaints = undocumented(&s, &doc).expect("fixture scan is not vacuous");
    assert!(
        complaints.iter().any(|c| c.contains("code=213")),
        "CONTROL FAILED: the same code as a STATEMENT must still be caught, or the \
         stripper has blinded the scanner rather than narrowed it. Got {complaints:?}"
    );
    let _ = fs::remove_dir_all(&real);
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

/// Every site that forwards a code it did not choose must carry a registry row.
///
/// # The anti-vacuity clause is load-bearing, and it was ABSENT
///
/// MEASURED 2026-09-02 05:53Z: with [`passthrough_chain`] short-circuited to `None` and
/// `PASSTHROUGH_FLOOR` relaxed to 0, this suite reported **7 passed, 0 failed**. The gate
/// that exists to catch an undeclared pass-through went fully green over ZERO of them,
/// because the loop below simply had nothing to iterate. A recogniser that stops matching
/// reported identically to a workspace with every site declared.
///
/// `the_scan_floors_seeded_from_this_suites_own_measurement_hold` did cover the count, but
/// a floor in a SIBLING leg is not this leg's guard: weaken or delete that leg and this
/// one returns to reporting green on nothing. So the floor is asserted HERE, against the
/// same scan this leg decides on. This is `calr`'s shape — the flagship pre-commit hook
/// exiting 0 on an empty index — one gate over.
///
/// # Why the message carries `file:line`
///
/// The previous version deduplicated with `!missing.contains(&pt.expression)` while
/// pushing the formatted `"{expr} (in {crate})"`, so the guard compared two different
/// shapes and never fired. `refill-idle-panes` therefore printed `outcome.code (in
/// refill-idle-panes)` four times: the count was right and the reader still had to go
/// find the sites by hand.
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
    assert!(
        s.passthrough.len() >= PASSTHROUGH_FLOOR,
        "PASS_THROUGH_SCAN_EMPTY: derived {} pass-through site(s), floor {PASSTHROUGH_FLOOR}. \
         An empty scan set is an ERROR, never a pass — this workspace forwards codes from \
         children and from typed verdicts in at least {PASSTHROUGH_FLOOR} places, so a \
         count below the floor means the recogniser stopped matching. Measured: blinding \
         it made this whole suite report 7 passed.",
        s.passthrough.len()
    );
    let missing: Vec<String> = s
        .passthrough
        .iter()
        .filter(|pt| !declared.contains(&pt.expression))
        .map(|pt| {
            format!(
                "{} at {}:{} (crate {})",
                pt.expression, pt.file, pt.line, pt.crate_name
            )
        })
        .collect();
    assert!(
        missing.is_empty(),
        "{} undeclared pass-through site(s) — a code chosen elsewhere can arrive wearing \
         our name here:\n{:#?}\n\n\
         Add an `XC-PT-*` row to §6 of docs/error_codes/exit_code_registry.md. The gate \
         keys on the EXPRESSION cell (cell index 1), not the crates cell: adding a crate \
         name to an existing row changes nothing and the edit will look accepted.",
        missing.len(),
        missing
    );
}

/// Every `XC-PT-*` row must actually declare an expression.
///
/// The expression cell is the only cell [`documented_passthrough`] reads, so a row with an
/// empty or unbackticked cell 1 is a row that declares nothing while looking like a
/// declaration. That is the failure mode a reader cannot see, and it is why a crate added
/// to the CRATES cell of `XC-PT-EXITCODE` on 2026-09-02 changed no verdict.
#[test]
fn every_pass_through_row_declares_an_expression() {
    let root = repo_root();
    let doc = fs::read_to_string(registry_path(&root)).expect("registry");
    let rows: Vec<_> = registry_rows(&doc)
        .into_iter()
        .filter(|r| r.id.starts_with("XC-PT-"))
        .collect();
    assert!(
        !rows.is_empty(),
        "ANTI-VACUITY: zero XC-PT-* rows parsed out of the registry"
    );
    let bad: Vec<String> = rows
        .iter()
        .filter(|r| {
            let raw = r.cells.get(1).map_or("", String::as_str).trim();
            let quoted = raw.starts_with('`') && raw.ends_with('`') && raw.len() > 2;
            !quoted || raw.trim_matches('`').trim().is_empty()
        })
        .map(|r| format!("{}: expression cell = {:?}", r.id, r.cells.get(1)))
        .collect();
    assert!(
        bad.is_empty(),
        "{} XC-PT-* row(s) declare no usable expression in cell 1, the ONLY cell the gate \
         reads:\n{:#?}",
        bad.len(),
        bad
    );
}

/// Rows deliberately kept although no scanned site matches their expression, each with
/// the reason. EMPTY BY DESIGN.
///
/// A row belongs here only when the site genuinely exists and the RECOGNISER cannot see
/// it — not when the site is gone. If the site is gone the row must be deleted, which is
/// what `no_declared_pass_through_row_outlives_its_site` enforces.
/// Allowances for reviewed narrowing casts, keyed by `file::function#occurrence:source as target`.
/// Category-three sites must either disappear or carry a reason containing DEBT.
///
/// ⛔ RE-KEYED 2026-09-12 (`omp-orchestrator-mg9ua`), FROM `path:LINE` TO A SHAPE. The old key
/// rotted on edits that never touched its subject: one row was re-anchored `:853 -> :1146`
/// (2026-09-11) and `:1146 -> :1153` (2026-09-12) while the cast never moved relative to its own
/// function, and each shift billed TWO complaints -- one ORPHANED row and one UNDECLARED site --
/// for one event.
///
/// ⛔ THE SECOND INSTANCE ORIGINALLY CITED HERE WAS READ FIVE WAYS BY FOUR AGENTS IN ONE
/// SESSION, AND THE ANSWER IS THAT THE ROW IS EARLY, NOT WRONG. The `XC-PT-RESPONSE` row in §6
/// of the registry was not renamed (wrong direction), is not false from birth, and is not
/// misplaced. The measured state, every operand printed:
///
/// ```text
/// COMMITTED  git log -S'response.exit_code' --all -- '*.rs' \
///              ':(exclude)crates/no-shell-gate/tests/exit_codes.rs'   -> 0 commits
/// WORKTREE   grep -c 'response\.exit_code' crates/contabo-reclaim/src/{lib,main}.rs
///                                                                    -> lib.rs 5 · main.rs 1
/// THE SITE   crates/contabo-reclaim/src/main.rs:52
///              `return ExitCode::from(response.exit_code);`   <- THE ROW'S OWN CITED LINE
/// HEAD-side  git grep 'report.outcome.exit_code()' HEAD       -> 1, in that crate's main.rs
///              ... and 0 in the worktree; model.rs and probe.rs DELETED, build.rs NEW
/// ```
///
/// THE ROW DECLARED `response.exit_code` AT `main.rs:52` AND IT IS AT `main.rs:52` RIGHT NOW, in
/// an uncommitted restructure. Its subject is MID-MIGRATION: correct about the code, ahead of
/// the commit. AND AN EXPRESSION-KEYED REGISTRY CANNOT REPRESENT THAT STATE EVEN WHEN THE ROW IS
/// RIGHT -- which is this bead's thesis arriving as a live specimen instead of an argument.
///
/// ⛔ FOUR INSTRUMENT RULES, ALL PAID FOR HERE, EACH BY A DIFFERENT AGENT:
/// 1. A `git log -S` claim REQUIRES A WORKTREE CONTROL and a worktree claim requires a committed
///    control. `-S` searches COMMITTED history: "never existed in history" is not "does not
///    exist", and an export-pinned run -- the discipline that makes every other figure here
///    trustworthy -- CANNOT SEE an uncommitted restructure.
/// 2. ⭐ NAME THE TREE. The `EMPTY` and the `1 commit` readings of the SAME query were both
///    correct: HEAD moved six commits between them, and the one commit is 473e62a ITSELF. The
///    query form was never the variable -- the TREE was -- so a count without its tree is not a
///    measurement. Attributing that difference to the observer was the error, and it is the
///    `cargo`-reads-the-worktree trap in pure form.
/// 3. A HISTORY CLAIM PUBLISHED AS A COMMENT MUST EXCLUDE ITS OWN FILE. The first version of
///    this comment said "is EMPTY", and committing it made the query match -- one commit, two
///    hits, both of them that sentence. The publication enters the corpus; the remedy
///    contaminated the detector. Hence the `:(exclude)` pathspec above, which is load-bearing
///    and not decoration.
/// 4. AND A `head` ON A LISTING IS A COUNT OF THE HEAD. "Five sites, none in main.rs" was
///    published from a `| head -5` over output sorted with lib.rs first. Six sites, and the
///    sixth is the one that mattered. Pipe a count, not a window, before asserting absence.
///
/// The fictional-row question is a DIFFERENT defect from either, and the shape key answers it
/// for free because a key derived from code shape has nothing to match when the code is not
/// there. Pinned by `a_row_for_an_expression_that_exists_nowhere_is_refused`.
///
/// A shape survives an insertion above it and a rename beside it; a line number survives neither.
///
/// WHAT THE RE-KEY COSTS AND BUYS is enumerated on `narrowing_key` and pinned by
/// `the_shape_key_survives_an_insertion_above_the_site`,
/// `a_rename_beside_the_site_refuses_once_and_names_both_spellings` and
/// `the_shape_key_cannot_see_a_cast_reordered_within_its_own_function`.
///
/// The `omp-idle-dispatch` row's DISPOSITION is untouched here and belongs to
/// `omp-orchestrator-t3d2c` (disk-vs-member population): that crate is excluded by
/// `cargo metadata` and cargo never builds it, so whether the row should exist at all is that
/// bead's question. Carried across unchanged, re-keyed only, exactly as its line-keyed
/// predecessor stood.
const NARROWING_ALLOWANCE: &[(&str, &str)] = &[
    ("crates/crate-soundness-verify/src/main.rs::main#0:EXIT_RED as u8", "ALLOWANCE CATEGORY-1: EXIT_RED is a closed literal status code; its documented value is 1 and cannot wrap."),
    ("crates/crate-soundness-verify/src/main.rs::main#1:EXIT_RED as u8", "ALLOWANCE CATEGORY-1: EXIT_RED is a closed literal status code; its documented value is 1 and cannot wrap."),
    ("crates/crate-soundness-verify/src/main.rs::main#2:EXIT_RED as u8", "ALLOWANCE CATEGORY-1: EXIT_RED is a closed literal status code; its documented value is 1 and cannot wrap."),
    ("crates/crate-soundness-verify/src/main.rs::main#3:EXIT_RED as u8", "ALLOWANCE CATEGORY-1: EXIT_RED is a closed literal status code; its documented value is 1 and cannot wrap."),
    ("crates/crate-soundness-verify/src/main.rs::main#4:EXIT_RED as u8", "ALLOWANCE CATEGORY-1: EXIT_RED is a closed literal status code; its documented value is 1 and cannot wrap."),
    ("crates/dispatcher-deadman/src/main.rs::main#0:verdict.exit as u8", "ALLOWANCE CATEGORY-2: verdict.exit forwards the child/status contract under XC-PT-VERDICT; range validation remains outside this gate."),
    ("crates/fleet-monitor/src/main.rs::main#0:EXIT_CANNOT_OBSERVE as u8", "ALLOWANCE CATEGORY-1: EXIT_CANNOT_OBSERVE is a named closed literal status code; its documented value is 69 and cannot wrap."),
    ("crates/fleet-monitor/src/main.rs::main#1:rc as u8", "ALLOWANCE CATEGORY-2: rc forwards the child/status contract under XC-PT-RC; range validation remains outside this gate."),
    ("crates/loop-driver/src/main.rs::acquire_for_probe#0:output.code as u8", "ALLOWANCE CATEGORY-2: output.code forwards the child status under XC-PT-OUTPUT; range validation remains outside this gate."),
    ("crates/loop-driver/src/main.rs::emit#0:output.code as u8", "ALLOWANCE CATEGORY-2: output.code forwards the child status under XC-PT-OUTPUT; range validation remains outside this gate."),
    ("crates/loop-queue-filter/src/main.rs::main#0:output.code as u8", "ALLOWANCE CATEGORY-2: output.code forwards the child status under XC-PT-OUTPUT; range validation remains outside this gate."),
    ("crates/omp-idle-dispatch/src/main.rs::main#0:exit as i32", "ALLOWANCE CATEGORY-2: exit forwards the child status into process::exit; range validation remains outside this gate. Disposition belongs to `omp-orchestrator-t3d2c`; carried across the mg9ua re-key unchanged."),
    ("crates/pane-oracle-diff/src/main.rs::main#0:v.exit_code() as u8", "ALLOWANCE CATEGORY-2: v.exit_code() forwards the typed verdict under XC-PT-EXITCODE; range validation remains outside this gate."),
    ("crates/pane-oracle-diff/src/main.rs::run_live#0:v.exit_code() as u8", "ALLOWANCE CATEGORY-2: v.exit_code() forwards the typed verdict under XC-PT-EXITCODE; range validation remains outside this gate."),
    ("crates/tick-dispatch/src/main.rs::eval_adm#0:exit as u8", "ALLOWANCE CATEGORY-2: exit forwards the child/status contract under XC-PT-EXIT; range validation remains outside this gate."),
    ("crates/tick-dispatch/src/main.rs::eval_adm#1:exit as u8", "ALLOWANCE CATEGORY-2: exit forwards the child/status contract under XC-PT-EXIT; range validation remains outside this gate."),
    ("crates/tick-dispatch/src/main.rs::pane_step#0:exit as u8", "ALLOWANCE CATEGORY-2: exit forwards the child/status contract under XC-PT-EXIT; range validation remains outside this gate."),
    ("crates/tick-dispatch/src/main.rs::run_live#0:exit as u8", "ALLOWANCE CATEGORY-2: exit forwards the child/status contract under XC-PT-EXIT; range validation remains outside this gate."),
    ("crates/tick-dispatch/src/main.rs::run_live#1:exit as u8", "ALLOWANCE CATEGORY-2: exit forwards the child/status contract under XC-PT-EXIT; range validation remains outside this gate."),
    ("crates/tick-dispatch/src/main.rs::run_live#2:exit as u8", "ALLOWANCE CATEGORY-2: exit forwards the child/status contract under XC-PT-EXIT; range validation remains outside this gate."),
    ("crates/tick-dispatch/src/main.rs::run_live#3:exit as u8", "ALLOWANCE CATEGORY-2: exit forwards the child/status contract under XC-PT-EXIT; range validation remains outside this gate."),
    ("crates/tick-dispatch/src/main.rs::run_live#4:exit as u8", "ALLOWANCE CATEGORY-2: exit forwards the child/status contract under XC-PT-EXIT; range validation remains outside this gate."),
    ("crates/verify-dispatch/src/main.rs::main#0:out.code as u8", "ALLOWANCE CATEGORY-2: out.code forwards the child/status contract under XC-PT-OUT; range validation remains outside this gate."),
];

/// THE ROW KEY: a SHAPE, not a POSITION (`omp-orchestrator-mg9ua`).
///
/// `file::function#occurrence:expr as target`. The LINE IS GONE from the key, because the
/// reviewed fact in every allowance reason is the EXPRESSION and the contract it forwards --
/// never the line it sits on. A line-keyed row rotted on any insertion ABOVE its site and
/// rotted BIDIRECTIONALLY (one ORPHANED row plus one UNDECLARED site) for a cast that had not
/// moved relative to its own function. Measured twice in two days on ONE row: `:853 -> :1146`
/// (2026-09-11) and `:1146 -> :1153` (2026-09-12).
///
/// # WHAT THE RE-KEY COSTS THE GATE, AND WHAT IT BUYS (acceptance item 3)
///
/// A re-key that silently narrows coverage is gate self-weakening, so both directions are
/// enumerated rather than asserted:
///
/// LOST -- what `path:LINE` could prove and `file::function#occurrence` cannot:
/// 1. THE EXACT POSITION. A cast MOVED WITHIN its own function -- above or below a sibling
///    statement, into or out of an `if` arm in the same body -- was previously a key change and
///    is now invisible. Pinned deliberately by
///    `the_shape_key_cannot_see_a_cast_reordered_within_its_own_function`, which asserts the
///    blindness rather than leaving it undocumented.
/// 2. THE ORDER OF TWO IDENTICAL CASTS in one function. `run_live#0..#4` are distinguished only
///    by occurrence index, so SWAPPING two of them is invisible where the line key saw it. What
///    the line key actually reported for such a swap was two spurious complaints about casts
///    nothing had touched, so this loss is a loss of NOISE in every case measured -- but it is
///    a real loss of resolution and it is stated, not waved away.
/// 3. Nothing else. The line was never named by any allowance reason: every row reviews the
///    EXPRESSION and the contract it forwards, and `:1153` appears in no reason text.
///
/// GAINED -- what the shape key proves that the line key could not:
/// 1. A row SURVIVES AN INSERTION ABOVE ITS SITE, so an honest edit elsewhere in the file is no
///    longer billed as two gate failures. That is the measured defect: twice in two days.
/// 2. A CAST THAT MOVES TO A DIFFERENT FUNCTION is now a key change BY CONSTRUCTION. The line
///    key caught that only by accident -- and would have MISSED it entirely had the move landed
///    on the same line number.
/// 3. A RENAME IS ONE ATTRIBUTABLE EVENT (`EXIT_NARROWING_RESPELLED`) naming both spellings,
///    instead of an ORPHANED row and an UNDECLARED site that never mention each other. The
///    refusal is NOT weakened: the row still fails, because the reviewed expression really did
///    change. Only the diagnosis improves.
/// 4. `EXIT_NARROWING_UNDECLARED` now prints `key=` -- the exact string a reviewer must paste --
///    so the remedy stops being a hand-derived line number.
fn narrowing_key(site: &NarrowingCast) -> String {
    format!(
        "{}::{}#{}:{} as {}",
        site.file, site.function, site.occurrence, site.expression, site.target
    )
}

/// The same shape with the EXPRESSION removed: the identity of the SITE rather than of the
/// spelling at it. Used only to turn a rename into ONE attributable complaint instead of an
/// ORPHANED/UNDECLARED pair -- the `XC-PT-RESPONSE` shape, where a rename orphaned a
/// declaration AND manufactured its own undeclared successor.
fn narrowing_site_key(site: &NarrowingCast) -> String {
    format!("{}::{}#{} as {}", site.file, site.function, site.occurrence, site.target)
}

/// `file::function#occurrence as target` for a DECLARED row's key, or `None` if the row is not
/// in shape-key form. Parsing the row rather than storing two columns keeps the table one
/// string per row, which is the form every reviewer already reads.
fn declared_site_key(key: &str) -> Option<String> {
    let (site, target) = key.rsplit_once(" as ")?;
    let (prefix, _expression) = site.rsplit_once(':')?;
    // The shape form, and nothing else. A legacy `path:LINE:expr` row parses to a prefix with
    // no `#`, and must NOT be paired as a respelling -- a line-keyed row carries no site
    // identity, so pairing it would invent one.
    if !prefix.contains('#') || !prefix.contains("::") {
        return None;
    }
    Some(format!("{prefix} as {target}"))
}

fn narrowing_complaints(scan: &Scan) -> Result<Vec<String>, String> {
    narrowing_complaints_with_allowance(scan, NARROWING_ALLOWANCE)
}

fn narrowing_complaints_with_allowance(
    scan: &Scan,
    allowances: &[(&str, &str)],
) -> Result<Vec<String>, String> {
    if scan.files == 0 {
        return Err(
            "EXIT_NARROWING_SCAN_EMPTY: zero source files scanned. An empty scan set is an \
             ERROR, never a pass — no narrowing cast was checked."
                .to_owned(),
        );
    }
    if scan.narrowing.is_empty() {
        return Err(
            "EXIT_NARROWING_SCAN_EMPTY: zero exit-path narrowing casts derived. The deciding \
             leg must not pass while its recogniser sees nothing."
                .to_owned(),
        );
    }
    let occupied: BTreeSet<String> = scan.narrowing.iter().map(narrowing_key).collect();
    let declared: BTreeSet<&str> = allowances.iter().map(|(key, _)| *key).collect();

    // The two unmatched sets, before pairing.
    let stray_sites: Vec<&NarrowingCast> = scan
        .narrowing
        .iter()
        .filter(|site| !declared.contains(narrowing_key(site).as_str()))
        .collect();
    let stray_rows: Vec<&str> = allowances
        .iter()
        .map(|(key, _)| *key)
        .filter(|key| !occupied.contains(*key))
        .collect();

    // THE RENAME ARM (acceptance 2b). A row and a site that agree on
    // `file::function#occurrence as target` and disagree only on the EXPRESSION are ONE event: a
    // respelling. The line-keyed table reported that event as an ORPHANED row PLUS an UNDECLARED
    // site -- two complaints, neither naming the other, which is exactly how `XC-PT-RESPONSE`
    // survived a rename as two unrelated-looking rows. It refuses CLEANLY: the reviewed fact (the
    // expression and the contract it forwards) really did change, so the row must be re-reasoned,
    // not silently carried -- but the operator is told WHAT changed and where, once.
    let mut paired_rows: BTreeSet<&str> = BTreeSet::new();
    let mut paired_sites: BTreeSet<String> = BTreeSet::new();
    let mut complaints: Vec<String> = Vec::new();
    for row in &stray_rows {
        let Some(row_site) = declared_site_key(row) else {
            continue;
        };
        let Some(site) = stray_sites
            .iter()
            .find(|site| narrowing_site_key(site) == row_site && !paired_sites.contains(&narrowing_key(site)))
        else {
            continue;
        };
        paired_rows.insert(row);
        paired_sites.insert(narrowing_key(site));
        complaints.push(format!(
            "EXIT_NARROWING_RESPELLED at {}:{} — {} declares `{}` and the site now reads `{} as {}`; \
             the allowance's reviewed expression changed and must be re-reasoned, not re-anchored",
            site.file, site.line, row, site.expression, site.expression, site.target
        ));
    }

    complaints.extend(
        stray_sites
            .iter()
            .filter(|site| !paired_sites.contains(&narrowing_key(site)))
            .map(|site| {
                format!(
                    "EXIT_NARROWING_UNDECLARED at {}:{} — {} as {} has no reasoned allowance row; \
                     key={}",
                    site.file,
                    site.line,
                    site.expression,
                    site.target,
                    narrowing_key(site)
                )
            }),
    );
    complaints.extend(
        stray_rows
            .iter()
            .filter(|key| !paired_rows.contains(*key))
            .map(|key| format!("EXIT_NARROWING_ORPHANED allowance names no scanned site: {key}")),
    );
    complaints.extend(
        allowances
            .iter()
            .filter(|(_, reason)| reason.trim().len() < 40)
            .map(|(key, _)| format!("EXIT_NARROWING_UNREASONED allowance is too short: {key}")),
    );
    Ok(complaints)
}

const UNMATCHED_ROW_ALLOWANCE: &[(&str, &str)] = &[];

/// Rows deliberately kept although no scanned site matches their expression, each with

/// A declared row must still describe a site. Checked in the direction nothing checked.
///
/// # The mirror defect, measured 2026-09-02
///
/// `every_pass_through_site_is_declared` asserts scanned ⊆ declared. Nothing asserted
/// declared ⊆ scanned, so a row could outlive its site in silence — and one immediately
/// did. `dc617e2` replaced `ExitCode::from(selftest() as u8)` with
/// `ExitCode::from(selftest_exit_code(selftest()))`, which [`passthrough_chain`] rejects
/// because the stripped argument still contains `(`. The site left the scan set,
/// `XC-PT-SELFTEST`'s expression cell matched nothing, `PASSTHROUGH_FLOOR` was still met
/// by the other rows, and the suite reported **9 passed, 0 failed** through a change that
/// emptied a row. `grep -rn 'ExitCode::from(selftest()' crates/` returned five hits, all
/// of them prose.
///
/// This is the exact mirror of the defect [`code_without_literals`] fixed. There, the SCANNER
/// invented a code nothing emits; here, the REGISTRY keeps a site nothing occupies. Same
/// failure, opposite direction — and this direction had no leg at all.
///
/// The message names BOTH remedies because the gate cannot tell them apart: either the
/// site is gone and the row must be retired, or the site still exists and the recogniser
/// stopped seeing it. `crates/pane-truth/src/main.rs:49` is a live instance of the second
/// case — `ExitCode::from(selftest(&rules) as u8)` is rejected on the `(` and the `&`, so
/// it is undeclared AND unrecognised, and this gate never demanded a row for it.
#[test]
fn no_declared_pass_through_row_outlives_its_site() {
    let root = repo_root();
    let doc = fs::read_to_string(registry_path(&root)).expect("registry");
    let declared = documented_passthrough(&doc);
    assert_eq!(
        declared.is_empty(),
        false,
        "ANTI-VACUITY: the registry declares no pass-through expressions, so this leg \
         would compare an empty set against anything and pass"
    );
    let s = scan(&root);
    assert!(
        s.passthrough.len() >= PASSTHROUGH_FLOOR,
        "PASS_THROUGH_SCAN_EMPTY: derived {} site(s), floor {PASSTHROUGH_FLOOR}. Without \
         this the leg would report every row as orphaned the moment the walker broke.",
        s.passthrough.len()
    );
    let occupied: BTreeSet<&str> = s
        .passthrough
        .iter()
        .map(|pt| pt.expression.as_str())
        .collect();
    let allowed: BTreeSet<&str> = UNMATCHED_ROW_ALLOWANCE.iter().map(|(e, _)| *e).collect();
    let orphaned: Vec<&String> = declared
        .iter()
        .filter(|e| !occupied.contains(e.as_str()) && !allowed.contains(e.as_str()))
        .collect();
    assert!(
        orphaned.is_empty(),
        "{} declared pass-through expression(s) match NO scanned site: {:?}\n\n\
         Two different things look like this and the gate cannot tell them apart:\n\
         (a) THE SITE IS GONE — retire the row. A row that outlives its site is how the \
         registry starts describing a workspace that no longer exists, and it is what \
         stops §6 from shrinking.\n\
         (b) THE SITE EXISTS AND THE RECOGNISER STOPPED SEEING IT — fix passthrough_chain, \
         not the registry. It rejects any argument that still contains `(` or `&` after \
         the trailing-`()` strip, so wrapping a call (`f(g())`) or passing a reference \
         (`f(&x)`) drops a real site out of the scan silently.\n\n\
         If a row must be kept because of (b), add it to UNMATCHED_ROW_ALLOWANCE with the \
         reason rather than leaving it silent.",
        orphaned.len(),
        orphaned
    );
    let stale: Vec<&str> = UNMATCHED_ROW_ALLOWANCE
        .iter()
        .map(|(e, _)| *e)
        .filter(|e| occupied.contains(e))
        .collect();
    assert!(
        stale.is_empty(),
        "{} allowance row(s) name an expression the scanner NOW matches: {:?}\n\
         Delete them — an allowance that outlives its defect is how a repaired gap keeps \
         reading as broken, and it is what stops this list from shrinking.",
        stale.len(),
        stale
    );
    let unreasoned: Vec<&str> = UNMATCHED_ROW_ALLOWANCE
        .iter()
        .filter(|(_, why)| why.trim().len() < 40)
        .map(|(e, _)| *e)
        .collect();
    assert!(
        unreasoned.is_empty(),
        "{unreasoned:?} carry no usable reason; a row without a reason is silence with \
         extra steps"
    );
}

#[test]
fn every_narrowing_exit_cast_has_a_reasoned_allowance() {
    let barren = fixture("narrowing-empty", "fn main() {}\n").join("nothing");
    fs::create_dir_all(&barren).expect("barren fixture");
    let empty_scan = scan(&barren);
    let empty_error = narrowing_complaints(&empty_scan).expect_err("empty narrowing scan");
    assert!(
        empty_error.starts_with("EXIT_NARROWING_SCAN_EMPTY"),
        "anti-vacuity must be in the deciding leg: {empty_error}"
    );

    let root = repo_root();
    let derived = scan(&root);
    println!(
        "EXIT_NARROWING_SCAN sites={} allowance_rows={}",
        derived.narrowing.len(),
        NARROWING_ALLOWANCE.len()
    );
    let complaints = narrowing_complaints(&derived).expect("real narrowing scan");
    assert!(
        complaints.is_empty(),
        "{} exit-path narrowing cast(s) lack a reasoned allowance or have an orphaned row:\n{}",
        complaints.len(),
        complaints.join("\n")
    );
    let _ = fs::remove_dir_all(barren);
}

#[test]
fn a_planted_narrowing_cast_is_red_and_the_known_good_shape_is_clean() {
    let bad = fixture(
        "narrowing-known-bad",
        "fn main() { let code = 300i32; let _ = std::process::ExitCode::from(code as u8); }\n",
    );
    let bad_scan = scan(&bad);
    let complaints = narrowing_complaints(&bad_scan).expect("bad fixture scan");
    assert!(
        complaints
            .iter()
            .any(|message| message.contains("crates/planted/src/main.rs:1")),
        "known-bad narrowing cast did not name file:line: {complaints:?}"
    );
    let stale_allowance = [("crates/planted/src/main.rs:99:missing as u8", "A deliberately long reason proves an allowance row must not outlive its scanned site.")];
    let stale_messages = narrowing_complaints_with_allowance(&bad_scan, &stale_allowance)
        .expect("stale allowance fixture scan");
    assert!(
        stale_messages
            .iter()
            .any(|message| message.contains("EXIT_NARROWING_ORPHANED")),
        "a stale allowance must fail bidirectionally: {stale_messages:?}"
    );
    let _ = fs::remove_dir_all(&bad);

    let good = fixture(
        "narrowing-known-good",
        "fn main() { let _ = std::process::ExitCode::from(1); }\n",
    );
    let good_scan = scan(&good);
    assert!(
        good_scan.narrowing.is_empty(),
        "known-good literal exit must not be classified as narrowing: {:?}",
        good_scan.narrowing
    );
    let _ = fs::remove_dir_all(&good);
}

/// The RETIRED key, reconstructed here and NOWHERE ELSE, so the defect can be CONSTRUCTED
/// rather than cited. This is not the old code path -- that code is deleted -- it is the old
/// KEY FUNCTION applied by today's matcher, which is sufficient because the rot was a property
/// of the key, not of the matcher.
fn legacy_line_key(site: &NarrowingCast) -> String {
    format!("{}:{}:{} as {}", site.file, site.line, site.expression, site.target)
}

fn legacy_complaints(scan: &Scan, allowances: &[(&str, &str)]) -> Vec<String> {
    let occupied: BTreeSet<String> = scan.narrowing.iter().map(legacy_line_key).collect();
    let declared: BTreeSet<&str> = allowances.iter().map(|(key, _)| *key).collect();
    let mut out: Vec<String> = scan
        .narrowing
        .iter()
        .filter(|site| !declared.contains(legacy_line_key(site).as_str()))
        .map(|site| format!("EXIT_NARROWING_UNDECLARED at {}:{}", site.file, site.line))
        .collect();
    out.extend(
        allowances
            .iter()
            .filter(|(key, _)| !occupied.contains(*key))
            .map(|(key, _)| format!("EXIT_NARROWING_ORPHANED allowance names no scanned site: {key}")),
    );
    out
}

/// ACCEPTANCE 1 AND 2(a), CONSTRUCTED: ONE INSERTED LINE, TWO FAILING ROWS under the retired
/// key -- and ZERO under the shape key.
///
/// The bidirectional claim was attributed, not measured, when this bead was filed. It is
/// measured here: the SAME single edit (one line inserted ABOVE the cast) must produce BOTH
/// `EXIT_NARROWING_UNDECLARED` and `EXIT_NARROWING_ORPHANED` from the line key. If only one
/// fired the claim would be PREMISE-FALSE and this leg would say so by failing.
#[test]
fn the_shape_key_survives_an_insertion_above_the_site() {
    let before_src = "fn main() {\n    let exit = 2i32;\n    std::process::exit(exit as i32);\n}\n";
    // ONE inserted line, ABOVE the cast, touching nothing the allowance reviews.
    let after_src =
        "fn main() {\n    let unrelated = 1;\n    let exit = 2i32;\n    std::process::exit(exit as i32);\n}\n";

    let before = fixture("mg9ua-insert-before", before_src);
    let before_scan = scan(&before);
    assert_eq!(
        before_scan.narrowing.len(),
        1,
        "ANTI-VACUITY: the fixture must present exactly one cast, or nothing below measures \
         anything: {:?}",
        before_scan.narrowing
    );
    let site = &before_scan.narrowing[0];
    let line_row = legacy_line_key(site);
    let shape_row = narrowing_key(site);
    assert_eq!(site.line, 3, "the fixture's cast must start where the key claims");
    let reason = "A deliberately long allowance reason, because a row without one is silence.";

    // KNOWN-GOOD, both keys: before the edit each row matches its own site.
    assert!(
        legacy_complaints(&before_scan, &[(line_row.as_str(), reason)]).is_empty(),
        "the line-keyed row must be clean BEFORE the insertion, or the rot below has no premise"
    );
    assert!(
        narrowing_complaints_with_allowance(&before_scan, &[(shape_row.as_str(), reason)])
            .expect("shape scan")
            .is_empty(),
        "the shape-keyed row must be clean before the insertion"
    );
    let _ = fs::remove_dir_all(&before);

    let after = fixture("mg9ua-insert-after", after_src);
    let after_scan = scan(&after);
    assert_eq!(after_scan.narrowing.len(), 1, "one cast after the insertion too");
    assert_eq!(
        after_scan.narrowing[0].line, 4,
        "the insertion must actually have moved the line, or this measures nothing"
    );

    // THE DEFECT, CONSTRUCTED. Both halves from the one edit.
    let rot = legacy_complaints(&after_scan, &[(line_row.as_str(), reason)]);
    assert!(
        rot.iter().any(|m| m.contains("EXIT_NARROWING_UNDECLARED"))
            && rot.iter().any(|m| m.contains("EXIT_NARROWING_ORPHANED")),
        "BIDIRECTIONAL ROT IS THE CLAIM: one inserted line above a declared cast must break the \
         row in BOTH directions under the retired line key. If only one fired the bead's premise \
         is FALSE. Got: {rot:?}"
    );
    assert_eq!(
        rot.len(),
        2,
        "two complaints for one event is the whole finding: {rot:?}"
    );

    // THE REMEDY. The same edit, the shape key, nothing to report.
    let survived = narrowing_complaints_with_allowance(&after_scan, &[(shape_row.as_str(), reason)])
        .expect("shape scan after insertion");
    assert!(
        survived.is_empty(),
        "the shape key must survive an insertion above its site: {survived:?}"
    );
    assert_eq!(
        narrowing_key(&after_scan.narrowing[0]),
        shape_row,
        "the key itself must be byte-identical across the edit, not merely tolerated"
    );
    println!("MG9UA_INSERTION line_key_rot={} shape_key_complaints=0 key={shape_row}", rot.len());
    let _ = fs::remove_dir_all(&after);
}

/// ACCEPTANCE 2(b): a RENAME BESIDE the site refuses ONCE and names both spellings.
///
/// The row must NOT silently carry -- the reviewed expression changed, and the reason text
/// reviews the expression. But it must refuse CLEANLY: ONE `EXIT_NARROWING_RESPELLED` naming
/// the old row and the new spelling, never the ORPHANED/UNDECLARED pair that made
/// `XC-PT-RESPONSE` look like two unrelated defects.
#[test]
fn a_rename_beside_the_site_refuses_once_and_names_both_spellings() {
    let renamed = fixture(
        "mg9ua-rename",
        "fn main() {\n    let status = 2i32;\n    std::process::exit(status as i32);\n}\n",
    );
    let scanned = scan(&renamed);
    assert_eq!(scanned.narrowing.len(), 1, "one cast, or nothing is measured");
    let reason = "A deliberately long allowance reason, because a row without one is silence.";
    // The row as it stood BEFORE the rename: same file, same function, same occurrence, same
    // target -- only the expression differs.
    let old_row = "crates/planted/src/main.rs::main#0:exit as i32";
    let messages = narrowing_complaints_with_allowance(&scanned, &[(old_row, reason)])
        .expect("rename scan");
    assert_eq!(
        messages.len(),
        1,
        "a rename is ONE event and must cost ONE complaint: {messages:?}"
    );
    let only = &messages[0];
    assert!(
        only.contains("EXIT_NARROWING_RESPELLED")
            && only.contains("exit as i32")
            && only.contains("status"),
        "the single complaint must name BOTH spellings: {only}"
    );
    assert!(
        !messages.iter().any(|m| m.contains("EXIT_NARROWING_ORPHANED"))
            && !messages.iter().any(|m| m.contains("EXIT_NARROWING_UNDECLARED")),
        "the retired pair must not reappear beside the typed verdict: {messages:?}"
    );
    // AND IT IS STILL A REFUSAL. Renaming does not buy a pass.
    assert!(
        !messages.is_empty(),
        "a respelled allowance must still fail; only the diagnosis improved"
    );
    let _ = fs::remove_dir_all(&renamed);
}

/// ACCEPTANCE 3, ASSERTED RATHER THAN CONCEDED: the cost of the re-key is that a cast moved
/// WITHIN its own function is invisible. Pinned so a future reader cannot mistake the blindness
/// for coverage, and so removing it becomes a visible change.
#[test]
fn the_shape_key_cannot_see_a_cast_reordered_within_its_own_function() {
    let moved = fixture(
        "mg9ua-reorder",
        "fn main() {\n    let exit = 2i32;\n    let _pad = 0;\n    let _pad2 = 0;\n    std::process::exit(exit as i32);\n}\n",
    );
    let scanned = scan(&moved);
    assert_eq!(scanned.narrowing.len(), 1, "one cast, or nothing is measured");
    assert_eq!(
        scanned.narrowing[0].line, 5,
        "the cast must sit at a DIFFERENT line than the other fixtures, or this proves nothing"
    );
    assert_eq!(
        narrowing_key(&scanned.narrowing[0]),
        "crates/planted/src/main.rs::main#0:exit as i32",
        "THIS IS THE COST: three different line numbers across this suite's fixtures all produce \
         ONE key. The gate can no longer tell them apart, and that is the documented trade."
    );
    let _ = fs::remove_dir_all(&moved);
}

/// THE NEW ACCEPTANCE ITEM, MEASURED: can a row be written for an expression that exists
/// NOWHERE? For this table the answer is NO, and this leg pins it in all three shapes a
/// fabricated row can take, so the answer cannot quietly become yes.
///
/// Context: the EXPRESSION-keyed instance originally attributed to this bead turned out to be a
/// row whose subject is MID-MIGRATION in an uncommitted restructure -- see the settled
/// measurement on `NARROWING_ALLOWANCE`, which prints both controls and the pathspec exclusion
/// this comment's own predecessor made necessary. It was neither renamed nor fictional, so the
/// FICTIONAL-ROW question below is a THIRD, separate defect: can a row be written for an
/// expression that exists nowhere at all? The shape key answers it for free -- a key derived
/// from code shape has nothing to match when the code is not there -- but "for free" is not
/// "for granted", which is why it is measured here rather than argued. No row can hide behind a
/// plausible-looking string.
#[test]
fn a_row_for_an_expression_that_exists_nowhere_is_refused() {
    let real = fixture(
        "mg9ua-fiction",
        "fn main() {\n    let exit = 2i32;\n    std::process::exit(exit as i32);\n}\n",
    );
    let scanned = scan(&real);
    assert_eq!(scanned.narrowing.len(), 1, "one real cast, or nothing is measured");
    let reason = "A deliberately long allowance reason, because a row without one is silence.";
    let truth = narrowing_key(&scanned.narrowing[0]);

    for (label, fabricated) in [
        // A function that does not exist in the file.
        ("absent function", "crates/planted/src/main.rs::no_such_fn#0:exit as i32"),
        // The right function, an occurrence index nothing reaches.
        ("absent occurrence", "crates/planted/src/main.rs::main#7:exit as i32"),
        // The retired line-keyed form, which carries no site identity at all.
        ("legacy line key", "crates/planted/src/main.rs:2:exit as i32"),
    ] {
        let messages =
            narrowing_complaints_with_allowance(&scanned, &[(truth.as_str(), reason), (fabricated, reason)])
                .expect("fiction scan");
        assert!(
            messages.iter().any(|m| m.contains("EXIT_NARROWING_ORPHANED") && m.contains(fabricated)),
            "a row with no possible subject ({label}) must be REFUSED and NAMED: {messages:?}"
        );
        assert!(
            !messages.iter().any(|m| m.contains("EXIT_NARROWING_RESPELLED")),
            "a fabricated row must not be laundered into a respelling of the real one \
             ({label}): {messages:?}"
        );
        assert!(
            !messages.iter().any(|m| m.contains("EXIT_NARROWING_UNDECLARED")),
            "the REAL site is declared and must stay declared beside the fiction ({label}): \
             {messages:?}"
        );
    }
    let _ = fs::remove_dir_all(&real);
}

/// A name bound to two values means a reader who learned one crate is confidently
/// wrong in the next. Shown RED against today's tree (`EXIT_CONFIG = {64, 78}`)
/// before cas lands; GREEN after fleet-composite stops using that name for 64.
#[test]
fn no_exit_constant_name_is_bound_to_two_values() {
    let scan = scan(&repo_root());
    let collisions: Vec<String> = scan
        .consts
        .iter()
        .filter(|(_, values)| values.len() != 1)
        .map(|(name, values)| format!("{name} = {values:?}"))
        .collect();
    assert!(
        collisions.is_empty(),
        "exit constant name bound to two values: {}",
        collisions.join("; ")
    );
}

