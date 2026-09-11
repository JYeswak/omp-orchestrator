#![forbid(unsafe_code)]

//! SENDER IDENTITY GATE — a crate that builds an `ntm` send invocation must put a
//! SENDER IDENTITY in the packet, or carry a named allowance with a reason.
//!
//! # The measured defect
//!
//! Every packet dispatched on 2026-09-01/02 went out as
//! `ntm send omp-orchestrator --panes=N` with no `FROM` line. `ntm --robot-send` names a
//! SESSION and an INDEX, and pane indices are not stable handles: they shifted twice in
//! one night, and two panes changed MODEL mid-session while their agent NAMES held.
//! Agent names are the durable handle; indices are not. Three agents could reply only
//! because they happened to know the sender was pane 1 — which is not addressing, it is
//! a guess that happened to be right.
//!
//! A receiving pane cannot answer a packet it cannot attribute. So the packet, not the
//! transport, must carry the sender.
//!
//! # The predicate, stated so it can be attacked
//!
//! For every tracked crate, for every `crates/<name>/src/**/*.rs`:
//!
//! 1. The file is tokenized once — comments removed, STRING LITERALS KEPT (see
//!    [`tokenize`]). Both halves matter: the flags live inside argv literals, so
//!    stripping strings destroys the signal; and prose about the flags is exactly what
//!    six gates in this repo have matched instead of the thing they hunted.
//! 2. A **dispatch site** is an occurrence of `--robot-send`, `--msg-file`, or `--msg`
//!    INSIDE A STRING LITERAL (see [`DISPATCH_FLAGS`]). `--msg-file` is matched before
//!    `--msg`, so one `--msg-file=` argument counts once, not twice.
//! 3. A file with at least one dispatch site must ALSO contain, somewhere in its
//!    comment-stripped source, one of [`SENDER_IDENTITY_MARKERS`] — a quoted packet
//!    field (`"sender"`, `"from"`, `"from_agent"`, `"reply_to"`) or a plaintext header
//!    form (`FROM=`, `FROM:`, `from=`, `reply_to=`).
//! 4. Otherwise the file's every dispatch site is reported as `file:line`, unless the
//!    file appears in [`NO_FROM_LINE_ALLOWANCE`] with a reason.
//!
//! The markers are QUOTED on purpose. A bare `sender` substring passes
//! `crates/omp-idle-dispatch/src/main.rs`, which has `"sender_ok": true` in a LEDGER row
//! and no sender identity in the packet at all — measured 2026-09-02. The quote is the
//! whole difference between reading a packet field and reading a receipt field.
//!
//! # What a green run does NOT establish
//!
//! * **Presence of a marker, never correctness of the value.** A file that renders
//!   `FROM: {}` with an empty string, or names the wrong agent, passes every leg here.
//!   Proving the rendered value is the true sender needs per-site dataflow, which is
//!   unbuilt. This gate raises the floor from *no sender field exists anywhere in the
//!   dispatching file* to *one exists, or a stated reason*.
//! * **File granularity, not site granularity.** A file with two dispatch paths, one
//!   carrying `"sender"` and one not, passes. That is a known hole and the reason the
//!   allowance is keyed by file: the unit of evidence and the unit of exception are the
//!   same, so no row can silently cover more than it names.
//! * **The WORKING TREE, not `HEAD`.** In a shared checkout with concurrent agents a
//!   green result can reflect a sibling's in-flight edit. The crate ROSTER comes from
//!   `git ls-files`, so an untracked scratch crate is not scanned; the CONTENTS are
//!   whatever is on disk.
//! * **`crates/<name>/src/` only — never `tests/`.** This file lives in `tests/` and
//!   names every flag and every marker it hunts, so sweeping `tests/` in would make
//!   `no-shell-gate` the largest dispatcher in the tree.
//!   [`the_scan_does_not_include_its_own_source`] pins that.
//! * A dispatch built by a shell wrapper, a build script, or a crate outside
//!   `crates/*/src` is invisible here.
//!
//! # Precedent
//!
//! Shape follows `oracle_routing.rs` and `exit_codes.rs` in this crate: `repo_root()`
//! from `CARGO_MANIFEST_DIR`, a git-derived roster, a comment-stripping pass that keeps
//! string literals, and a bidirectional allowance that can only shrink.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use subprocess_contract::{bounded_output, BoundedOutcome};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root two levels up")
        .to_path_buf()
}

/// The flag spellings that make a string literal a DISPATCH SITE.
///
/// Order is load-bearing: `--msg-file` precedes `--msg`, and [`flags_in_literal`]
/// consumes the longest match, so `--msg-file=/tmp/packet` yields ONE site rather than
/// two. A gate that double-counts its own subject cannot report an honest denominator.
const DISPATCH_FLAGS: &[&str] = &["--robot-send", "--msg-file", "--msg"];

/// Evidence that the dispatching file puts a sender identity in the packet.
///
/// Every entry is QUOTED or carries its delimiter. That exactness is the measured
/// difference between a packet field and a neighbouring receipt field: `"sender"` does
/// not match `"sender_ok"`, and `from=` does not match `serde_json::from_str` or
/// `String::from`. A bare `from` / `sender` substring passes all four offenders below.
const SENDER_IDENTITY_MARKERS: &[&str] = &[
    "\"sender\"",
    "\"from\"",
    "\"from_agent\"",
    "\"reply_to\"",
    "FROM=",
    "FROM:",
    "from=",
    "reply_to=",
];

// ---------------------------------------------------------------------------------------
// The tokenizer
// ---------------------------------------------------------------------------------------

/// One string literal, with the 1-based line on which it OPENS.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Literal {
    line: usize,
    body: String,
}

/// Strip `//` and `/* */` comments while KEEPING string literals, and collect those
/// literals with their line numbers — in ONE pass.
///
/// One pass rather than two is not tidiness. A separate literal walker run over
/// already-stripped text has to re-derive where strings start, and a `'"'` char literal
/// or a raw string desyncs it silently: every subsequent literal in the file is then
/// parsed with the quotes inverted, which turns real argv into "not a literal" and prose
/// into "a literal". So char literals and `r#"..."#` are consumed here, by the same
/// cursor that decides what a string is.
///
/// Newlines inside block comments are PRESERVED so reported line numbers survive a
/// `/* ... */` that spans lines.
fn tokenize(source: &str) -> (String, Vec<Literal>) {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut literals = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize;

    while i < chars.len() {
        let c = chars[i];

        if c == '\n' {
            out.push('\n');
            line += 1;
            i += 1;
            continue;
        }

        // Line comment: drop to end of line, leave the newline for the arm above.
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        }

        // Block comment: drop the body, keep every newline.
        if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i < chars.len() && !(chars[i] == '*' && chars.get(i + 1) == Some(&'/')) {
                if chars[i] == '\n' {
                    out.push('\n');
                    line += 1;
                }
                i += 1;
            }
            i = (i + 2).min(chars.len());
            continue;
        }

        // Raw string: `r"..."`, `r#"..."#`, `r##"..."##`.
        if c == 'r' {
            let mut hashes = 0usize;
            while chars.get(i + 1 + hashes) == Some(&'#') {
                hashes += 1;
            }
            if chars.get(i + 1 + hashes) == Some(&'"') {
                let open_line = line;
                let mut j = i + 2 + hashes;
                let mut body = String::new();
                loop {
                    if j >= chars.len() {
                        break;
                    }
                    if chars[j] == '"' && (1..=hashes).all(|k| chars.get(j + k) == Some(&'#')) {
                        j += 1 + hashes;
                        break;
                    }
                    if chars[j] == '\n' {
                        line += 1;
                    }
                    body.push(chars[j]);
                    j += 1;
                }
                out.push('"');
                out.push_str(&body);
                out.push('"');
                literals.push(Literal {
                    line: open_line,
                    body,
                });
                i = j;
                continue;
            }
        }

        // Char literal (`'x'`, `'\n'`) versus a lifetime tick (`'a`). Consumed whole so a
        // quote inside a char literal cannot desync the string arm below.
        if c == '\'' {
            if chars.get(i + 1) == Some(&'\\') {
                let mut j = i + 2;
                while j < chars.len() && j < i + 10 && chars[j] != '\'' {
                    j += 1;
                }
                if chars.get(j) == Some(&'\'') {
                    out.push('\'');
                    out.push('_');
                    out.push('\'');
                    i = j + 1;
                    continue;
                }
            } else if chars.get(i + 2) == Some(&'\'') {
                out.push('\'');
                out.push('_');
                out.push('\'');
                i += 3;
                continue;
            }
            out.push(c);
            i += 1;
            continue;
        }

        // Ordinary string literal.
        if c == '"' {
            let open_line = line;
            out.push('"');
            i += 1;
            let mut body = String::new();
            while i < chars.len() {
                if chars[i] == '\\' {
                    out.push(chars[i]);
                    if let Some(next) = chars.get(i + 1) {
                        out.push(*next);
                        if *next == '\n' {
                            line += 1;
                        }
                    }
                    body.push(' ');
                    i += 2;
                    continue;
                }
                if chars[i] == '"' {
                    out.push('"');
                    i += 1;
                    break;
                }
                if chars[i] == '\n' {
                    line += 1;
                }
                out.push(chars[i]);
                body.push(chars[i]);
                i += 1;
            }
            literals.push(Literal {
                line: open_line,
                body,
            });
            continue;
        }

        out.push(c);
        i += 1;
    }

    (out, literals)
}

/// The comment-stripped source alone — the half [`carries_sender_identity`] reads.
fn strip_comments(source: &str) -> String {
    text_structure::code_only(source).into_owned()
}

/// Dispatch flags inside one literal body, longest match first, non-overlapping.
fn flags_in_literal(body: &str) -> Vec<&'static str> {
    let mut hits = Vec::new();
    let bytes = body.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'-' {
            if let Some(flag) = DISPATCH_FLAGS
                .iter()
                .copied()
                .find(|flag| body[i..].starts_with(flag))
            {
                hits.push(flag);
                i += flag.len();
                continue;
            }
        }
        i += 1;
    }
    hits
}

/// Does this comment-stripped source name a sender identity anywhere?
fn carries_sender_identity(stripped: &str) -> bool {
    SENDER_IDENTITY_MARKERS
        .iter()
        .any(|marker| stripped.contains(marker))
}

// ---------------------------------------------------------------------------------------
// The scan
// ---------------------------------------------------------------------------------------

/// One dispatch site: which file, which line, which flag, and whether the ENCLOSING FILE
/// carries a sender identity.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Site {
    file: String,
    line: usize,
    flag: &'static str,
    identified: bool,
}

#[derive(Debug)]
struct Scan {
    crates: usize,
    files: usize,
    sites: Vec<Site>,
}

impl Scan {
    /// Files that dispatch and carry no sender identity, deduplicated.
    fn offending_files(&self) -> BTreeSet<&str> {
        self.sites
            .iter()
            .filter(|site| !site.identified)
            .map(|site| site.file.as_str())
            .collect()
    }
}

/// Run `git` under the workspace's bounded-subprocess kernel.
///
/// `TimedOut` and `Unspawned` are RESTRICTIVE outcomes and are never read as success:
/// both collapse to `None`, the roster comes back EMPTY, and
/// [`an_empty_dispatch_site_set_is_an_error`] turns that into a named error rather than a
/// clean bill. A hand-rolled `Command::output()` here would block forever on a wedged
/// index lock, which is the exact defect `subprocess-contract` exists to forbid.
fn git(root: &Path, args: &[&str]) -> Option<String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(root).args(args);
    match bounded_output(&mut command, Duration::from_secs(60)) {
        BoundedOutcome::Completed(output) if output.status.success() => {
            Some(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        _ => None,
    }
}

/// Crate roster from `git ls-files`, never a hand list.
///
/// A hand list is how a census went stale at 27 crates while the tree held 51. Deriving
/// it also means an untracked scratch crate is not scanned, and that a `git` failure
/// yields an EMPTY roster — which the anti-vacuity leg converts into a hard error.
fn tracked_crates(root: &Path) -> Vec<String> {
    let Some(stdout) = git(root, &["ls-files", "--", "crates/*/Cargo.toml"]) else {
        return Vec::new();
    };
    let mut names: Vec<String> = stdout
        .lines()
        .filter_map(|line| line.strip_prefix("crates/"))
        .filter_map(|rest| rest.strip_suffix("/Cargo.toml"))
        .map(str::to_string)
        .collect();
    names.sort();
    names.dedup();
    names
}

/// Every `.rs` file beneath `crates/<name>/src`, recursively. Deliberately NOT `tests/`.
fn crate_source_files(root: &Path, name: &str) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut stack = vec![root.join("crates").join(name).join("src")];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_some_and(|ext| ext == "rs") {
                found.push(path);
            }
        }
    }
    found.sort();
    found
}

/// The whole scan for an explicit roster. [`scan`] is this with the git roster; the
/// fixtures reach it through [`scan`] too, because their temp trees are real git repos.
fn scan_roster(root: &Path, names: &[String]) -> Scan {
    let mut sites = Vec::new();
    let mut files = 0usize;
    for name in names {
        for path in crate_source_files(root, name) {
            let Ok(text) = fs::read_to_string(&path) else {
                continue;
            };
            files += 1;
            let (stripped, literals) = tokenize(&text);
            let identified = carries_sender_identity(&stripped);
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            for literal in &literals {
                for flag in flags_in_literal(&literal.body) {
                    sites.push(Site {
                        file: relative.clone(),
                        line: literal.line,
                        flag,
                        identified,
                    });
                }
            }
        }
    }
    sites.sort_by(|a, b| (&a.file, a.line, a.flag).cmp(&(&b.file, b.line, b.flag)));
    Scan {
        crates: names.len(),
        files,
        sites,
    }
}

fn scan(root: &Path) -> Scan {
    scan_roster(root, &tracked_crates(root))
}

// ---------------------------------------------------------------------------------------
// The allowance
// ---------------------------------------------------------------------------------------

/// Dispatch-site files permitted to carry no sender identity, each with the reason.
///
/// CURRENT ALLOWANCE after mad1: packet renderers now add a verified sender header before staging.
/// The sole remaining row is dispatch-saga/m2, which returns a downstream send argv but stages no
/// packet; this is a named non-packet boundary, not an unidentified packet exception. The row is
/// checked by [`scan`] in both directions and must be deleted if that boundary gains packet data.
///
/// Checked in BOTH directions. A dispatching file with no sender identity and no row
/// fails; a row naming a file that has SINCE grown one also fails, with a message telling
/// the reader to delete the row. An allowance that outlives its defect is how a repaired
/// gap keeps reading as broken, and it is what stops this list from shrinking.
const NO_FROM_LINE_ALLOWANCE: &[(&str, &str)] = &[
    (
        "crates/dispatch-saga/src/m2.rs",
        "Decision-only route: it constructs an ntm send argv for a downstream consumer but has no \
         --msg or --msg-file and stages no packet. A FROM line has no packet boundary to live in; \
         the downstream renderer remains responsible for sender identity.",
    ),
];

fn allowed() -> BTreeSet<&'static str> {
    NO_FROM_LINE_ALLOWANCE
        .iter()
        .map(|(file, _)| *file)
        .collect()
}

/// The check itself, so every leg exercises one code path.
///
/// `Err` is reserved for VACUITY: zero dispatch sites means the scan is broken, most
/// often because `git ls-files` returned an empty roster. A deliberate NAMED error, so a
/// deliverable that was never checked cannot report identically to one that passed.
fn unidentified_sites(scan: &Scan) -> Result<Vec<String>, String> {
    if scan.sites.is_empty() {
        return Err(format!(
            "SENDER_SCAN_EMPTY crates={} files={} sites=0 — zero dispatch sites found. \
             An empty scan set is an ERROR, never a pass: the most likely cause is an \
             empty crate roster from `git ls-files -- crates/*/Cargo.toml`.",
            scan.crates, scan.files
        ));
    }
    let allow = allowed();
    Ok(scan
        .sites
        .iter()
        .filter(|site| !site.identified && !allow.contains(site.file.as_str()))
        .map(|site| {
            format!(
                "{}:{} flag={} — builds an ntm send with no sender identity in the packet",
                site.file, site.line, site.flag
            )
        })
        .collect())
}

/// Allowance rows that no longer describe a defect: the file is gone, holds no dispatch
/// site any more, or has grown a sender identity.
fn stale_allowance_rows(root: &Path, scan: &Scan) -> Vec<String> {
    let offending = scan.offending_files();
    let dispatching: BTreeSet<&str> = scan.sites.iter().map(|s| s.file.as_str()).collect();
    NO_FROM_LINE_ALLOWANCE
        .iter()
        .filter(|(file, _)| !offending.contains(*file))
        .map(|(file, _)| {
            let why = if !root.join(file).exists() {
                "the file does not exist"
            } else if !dispatching.contains(*file) {
                "the file holds no dispatch site"
            } else {
                "the file now carries a sender identity"
            };
            format!("{file} — {why}; DELETE THIS ROW")
        })
        .collect()
}

// ---------------------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------------------

fn temp_root(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "omp-sender-identity-{}-{}-{tag}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ))
}

/// A temp tree shaped like ours AND tracked by git, so the fixture legs run [`scan`]
/// verbatim — roster derivation included — rather than a parallel copy of it.
fn fixture(tag: &str, body: &str) -> PathBuf {
    let root = temp_root(tag);
    let src = root.join("crates").join("planted").join("src");
    fs::create_dir_all(&src).expect("fixture dirs");
    fs::write(
        root.join("crates").join("planted").join("Cargo.toml"),
        "[package]\nname = \"planted\"\nversion = \"0.0.0\"\nedition = \"2021\"\n",
    )
    .expect("fixture manifest");
    fs::write(src.join("main.rs"), body).expect("fixture source");
    git(&root, &["init", "-q"]).expect("fixture `git init` must succeed");
    git(&root, &["add", "-f", "--", "crates"]).expect("fixture `git add` must succeed");
    root
}

/// A git repo with no `crates/` at all — the vacuity case.
fn barren_fixture(tag: &str) -> PathBuf {
    let root = temp_root(tag);
    fs::create_dir_all(&root).expect("barren dir");
    git(&root, &["init", "-q"]).expect("barren `git init` must succeed");
    root
}

/// A planted dispatch with NO sender identity — the known-bad body.
const PLANTED_BAD: &str = r#"
fn dispatch(session: &str, pane: &str, packet: &str) -> Vec<String> {
    vec![
        format!("--robot-send={session}"),
        format!("--panes={pane}"),
        format!("--msg={packet}"),
    ]
}
"#;

/// The same dispatch WITH a sender identity — the known-good body.
const PLANTED_GOOD: &str = r#"
fn dispatch(session: &str, pane: &str, packet: &str) -> Vec<String> {
    let body = format!("FROM: refill-idle-panes (pane %1413)\n{packet}");
    vec![
        format!("--robot-send={session}"),
        format!("--panes={pane}"),
        format!("--msg={body}"),
    ]
}
"#;

// ---------------------------------------------------------------------------------------
// Legs
// ---------------------------------------------------------------------------------------

/// THE INVARIANT. Every file that builds an `ntm` send names its sender, or is allowed.
///
/// Predicate: for each `crates/<name>/src/**/*.rs` over the git-derived roster, if any
/// string literal contains `--robot-send`, `--msg-file` or `--msg`, then the file's
/// comment-stripped source must contain one of [`SENDER_IDENTITY_MARKERS`]. Failures are
/// reported one per dispatch site as `file:line flag=...`. Files listed in
/// [`NO_FROM_LINE_ALLOWANCE`] are exempt and are checked for staleness by
/// [`every_allowance_row_carries_a_reason_and_names_a_real_site`].
#[test]
fn every_dispatch_site_renders_a_from_line() {
    let root = repo_root();
    let scanned = scan(&root);
    let complaints = unidentified_sites(&scanned).expect("the real scan must not be vacuous");
    assert!(
        complaints.is_empty(),
        "{} dispatch site(s) with no sender identity and no allowance row.\n\
         A pane index is not a durable handle — it shifted twice in one night — so a \
         packet without a FROM line cannot be answered.\n{}",
        complaints.len(),
        complaints.join("\n")
    );
}

/// ANTI-VACUITY. Zero dispatch sites is a NAMED error, never a clean bill — with a
/// positive control first, proving the reader can see what IS present.
#[test]
fn an_empty_dispatch_site_set_is_an_error() {
    let root = repo_root();

    // POSITIVE CONTROL 1: the roster is real. Floor seeded from this suite's own
    // measurement (56 tracked manifests on 2026-09-02) and set far enough below it that
    // ordinary growth does not trip it, far enough above zero that a broken git does.
    let roster = tracked_crates(&root);
    assert!(
        roster.len() > 40,
        "only {} tracked crate manifest(s) from `git ls-files` — if git is failing here \
         the roster is a lie and every clean bill below it is vacuous",
        roster.len()
    );

    // POSITIVE CONTROL 2: the tokenizer finds literals that are known to be present.
    let scanned = scan(&root);
    assert!(
        scanned.files >= 100,
        "only {} source file(s) scanned across {} crates",
        scanned.files,
        scanned.crates
    );
    assert!(
        scanned.sites.len() >= 10,
        "only {} dispatch site(s) found; the current tree measured 17; a drop below this \
         floor means the literal walker stopped seeing argv",
        scanned.sites.len()
    );
    // omp-orchestrator-nar5l: this named `crates/omp-orchestrator/src/main.rs`, a path absent
    // from TREE, INDEX and WORKTREE -- the crate is lib-plus-`src/bin/`, so the "literal that is
    // definitely there" was in a file that is definitely not. `resident.rs` is the dispatcher
    // now and carries the `send-keys` argv the walker keys on. The CLAIM was true; the address
    // was dead. Not repointed at `lib.rs`, which would resolve without carrying the behaviour.
    assert!(
        scanned
            .sites
            .iter()
            .any(|site| site.file == "crates/omp-orchestrator/src/resident.rs"),
        "the known dispatcher crates/omp-orchestrator/src/resident.rs was not found — the \
         reader cannot see a literal that is definitely there"
    );
    assert!(
        scanned
            .sites
            .iter()
            .any(|site| site.identified && site.file == "crates/tick-dispatch/src/main.rs"),
        "crates/tick-dispatch/src/main.rs carries `\"sender\"` and must be seen as \
         identified — the marker reader is blind"
    );

    // THE VACUITY CASE: a git repo with no crates at all.
    let barren = barren_fixture("empty-set");
    let empty = scan(&barren);
    assert_eq!(
        empty.crates, 0,
        "the barren root must yield an empty roster"
    );
    assert_eq!(empty.sites.len(), 0, "the barren root must find no sites");
    let verdict = unidentified_sites(&empty);
    assert!(
        verdict
            .as_ref()
            .is_err_and(|message| message.starts_with("SENDER_SCAN_EMPTY")),
        "an empty dispatch-site set must be a NAMED error, not an empty complaint list; \
         got {verdict:?}"
    );
    let _ = fs::remove_dir_all(&barren);
}

/// FIRES ON KNOWN-BAD. Runs [`scan`] and [`unidentified_sites`] — the identical path leg
/// one takes — over a planted crate whose send carries no sender field.
#[test]
fn a_planted_dispatch_site_without_a_from_line_is_caught() {
    let root = fixture("known-bad", PLANTED_BAD);
    let scanned = scan(&root);
    assert_eq!(
        scanned.crates, 1,
        "the fixture roster must come back from git: positive control on the roster path"
    );
    assert_eq!(scanned.files, 1, "the fixture source must be seen");
    let complaints = unidentified_sites(&scanned).expect("the fixture scan is not vacuous");
    assert!(
        complaints
            .iter()
            .any(|c| c.starts_with("crates/planted/src/main.rs:")),
        "the gate did not fire on a planted send with no sender identity; complaints \
         were {complaints:?}"
    );
    assert!(
        complaints.iter().any(|c| c.contains("flag=--robot-send")),
        "the flag must be named in the complaint; got {complaints:?}"
    );
    let _ = fs::remove_dir_all(&root);
}

/// KNOWN-GOOD. The same fixture WITH a `FROM:` header must pass. Without this leg the
/// gate could be over-strict and still look green, and an over-strict gate gets routed
/// around — a slower death than no gate.
#[test]
fn a_planted_dispatch_site_with_a_from_line_passes() {
    let root = fixture("known-good", PLANTED_GOOD);
    let scanned = scan(&root);
    assert_eq!(scanned.files, 1, "the fixture source must be seen");
    assert!(
        !scanned.sites.is_empty(),
        "the good fixture must still register as a dispatch site — otherwise this leg \
         proves nothing about the marker reader"
    );
    assert!(
        scanned.sites.iter().all(|site| site.identified),
        "a send carrying `FROM:` must be recognised as identified"
    );
    let complaints = unidentified_sites(&scanned).expect("the fixture scan is not vacuous");
    assert!(
        complaints.is_empty(),
        "a dispatch site with a sender identity must pass; got {complaints:?}"
    );
    let _ = fs::remove_dir_all(&root);
}

/// The allowance can only shrink: every row must carry a real reason and still describe a
/// real, still-broken dispatch site.
#[test]
fn every_allowance_row_carries_a_reason_and_names_a_real_site() {
    let root = repo_root();
    let scanned = scan(&root);
    assert!(
        !scanned.sites.is_empty(),
        "vacuous scan: staleness cannot be judged with no sites"
    );

    let mut thin: Vec<String> = Vec::new();
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    for (file, reason) in NO_FROM_LINE_ALLOWANCE {
        *seen.entry(*file).or_default() += 1;
        if reason.trim().len() < 20 {
            thin.push(format!(
                "{file} — reason is {} chars; a row without a stated reason is an \
                 exemption nobody can audit",
                reason.trim().len()
            ));
        }
    }
    assert!(thin.is_empty(), "{}", thin.join("\n"));

    let duplicated: Vec<&&str> = seen
        .iter()
        .filter(|(_, count)| **count > 1)
        .map(|(file, _)| file)
        .collect();
    assert!(
        duplicated.is_empty(),
        "duplicate allowance row(s) {duplicated:?} — two reasons for one file means one \
         of them is unread"
    );

    let stale = stale_allowance_rows(&root, &scanned);
    assert!(
        stale.is_empty(),
        "{} stale allowance row(s). An allowance that outlives its defect is how a \
         repaired gap keeps reading as broken:\n{}",
        stale.len(),
        stale.join("\n")
    );
}

/// The gate must not match itself. This file names every flag and every marker it hunts,
/// so if `tests/` were ever swept in, `no-shell-gate` would become the largest dispatcher
/// in the tree and the allowance would have to grow a row for the gate itself.
#[test]
fn the_scan_does_not_include_its_own_source() {
    let root = repo_root();
    let this_file = root
        .join("crates")
        .join("no-shell-gate")
        .join("tests")
        .join("sender_identity.rs");
    let own = fs::read_to_string(&this_file).expect("the gate must be able to find itself");

    // Positive control: exclusion is only meaningful if this file WOULD match.
    let (_, literals) = tokenize(&own);
    let own_sites: usize = literals
        .iter()
        .map(|literal| flags_in_literal(&literal.body).len())
        .sum();
    assert!(
        own_sites > 0,
        "this file must itself contain dispatch flags in string literals, or proving \
         exclusion proves nothing"
    );

    let scanned = scan(&root);
    assert!(
        !scanned
            .sites
            .iter()
            .any(|site| site.file.starts_with("crates/no-shell-gate/tests/")),
        "the scan swept crates/no-shell-gate/tests/ — it must cover src/ only"
    );
    assert!(
        !scanned
            .sites
            .iter()
            .any(|site| site.file.starts_with("crates/no-shell-gate/")),
        "no-shell-gate must not appear as a dispatcher; if it does, the scan is reading \
         its own gate sources"
    );
    assert!(
        crate_source_files(&root, "no-shell-gate")
            .iter()
            .all(|path| !path.ends_with("sender_identity.rs")),
        "the source walker reached tests/sender_identity.rs"
    );
}

/// The tokenizer must tell an argv literal from prose about one, and must not be desynced
/// by a quote inside a char literal.
#[test]
fn tokenizing_removes_prose_but_keeps_argv_literals() {
    let source = concat!(
        "// ntm --robot-send is the dispatch kernel and --msg carries the packet\n",
        "/* --msg-file=/tmp/packet\n   still a comment */\n",
        "fn f() { let quote = '\"'; let a = \"--robot-send=demo\"; }\n"
    );
    let (stripped, literals) = tokenize(source);
    assert!(
        !stripped.contains("dispatch kernel"),
        "line-comment prose survived stripping: {stripped:?}"
    );
    assert!(
        !stripped.contains("still a comment"),
        "block-comment prose survived stripping: {stripped:?}"
    );
    assert!(
        stripped.contains("\"--robot-send=demo\""),
        "the argv literal was destroyed: {stripped:?}"
    );

    let bodies: Vec<&str> = literals.iter().map(|l| l.body.as_str()).collect();
    assert_eq!(
        bodies,
        vec!["--robot-send=demo"],
        "exactly one string literal must be seen; a quote inside a char literal must not \
         desync the walker"
    );
    assert_eq!(
        literals[0].line, 4,
        "the literal is on line 4; newlines inside a block comment must be preserved so \
         reported line numbers are usable"
    );

    // Longest match first: `--msg-file=` is one site, not two.
    assert_eq!(flags_in_literal("--msg-file=/tmp/p"), vec!["--msg-file"]);
    assert_eq!(flags_in_literal("--msg=body"), vec!["--msg"]);
    assert_eq!(
        flags_in_literal("--robot-send=s --panes=1 --msg-file=/tmp/p"),
        vec!["--robot-send", "--msg-file"]
    );
    assert!(flags_in_literal("--panes=1 --all").is_empty());
}

/// The marker reader must tell a packet field from a neighbouring receipt field. This is
/// the exact false pass measured on `omp-idle-dispatch`, which has `"sender_ok"` in a
/// ledger row and no sender identity in its packet.
#[test]
fn the_marker_reader_distinguishes_a_packet_field_from_a_receipt_field() {
    assert!(
        !carries_sender_identity("let row = json!({ \"sender_ok\": true });"),
        "`\"sender_ok\"` is a receipt about the send, not an identity in it"
    );
    assert!(
        !carries_sender_identity("let v: T = serde_json::from_str(s)?; String::from(x);"),
        "`from_str` / `String::from` are not sender identities"
    );
    assert!(carries_sender_identity("json!({ \"sender\": sender })"));
    assert!(carries_sender_identity(
        "format!(\"FROM: omp-orchestrator\\n\")"
    ));
    assert!(carries_sender_identity(
        "format!(\"FROM=omp-orchestrator\\n\")"
    ));
    assert!(carries_sender_identity("json!({ \"from_agent\": me })"));
    assert!(carries_sender_identity("json!({ \"reply_to\": pane })"));
    // Prose about a FROM line is stripped before the marker reader sees it.
    assert!(
        !carries_sender_identity(&strip_comments("// the packet must carry FROM: <agent>\n")),
        "a comment promising a FROM line must not satisfy the gate"
    );
}
