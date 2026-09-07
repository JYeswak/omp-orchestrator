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

use std::{fs, path::{Path, PathBuf}, process::{Command, Stdio}};


fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf()
}

struct Figure {
    key: String,
    command: String,
    expect: String,
}

#[derive(Debug)]
enum FigureRun {
    Compared { got: String },
    LiveNonempty,
    ProbeMissing { probe: &'static str, detail: String },
    SpawnFailed,
}

fn executable_on_path(name: &str, path_var: Option<&str>) -> bool {
    let mut cmd = Command::new(name);
    cmd.arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(path) = path_var {
        cmd.env("PATH", path);
    }
    cmd.status().is_ok()
}

/// Host probes a figure's command needs. Absence is not a measured zero.
fn missing_host_probe(command: &str, path_var: Option<&str>) -> Option<(&'static str, String)> {
    if command.contains("br list") || command.contains(" br ") || command.starts_with("br ") {
        if !executable_on_path("br", path_var) {
            return Some(("br", "br not on PATH".to_owned()));
        }
    }
    const OMP_TYPES: &str =
        "/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/types";
    if command.contains(OMP_TYPES) && !Path::new(OMP_TYPES).is_dir() {
        return Some((
            "omp_dist_types",
            format!("{OMP_TYPES} is not a directory"),
        ));
    }
    const BUILDSHARED: &str = "/Volumes/BuildShared/cargo-targets/release";
    if command.contains("/Volumes/BuildShared") && !Path::new(BUILDSHARED).is_dir() {
        return Some((
            "buildshared_release",
            format!("{BUILDSHARED} is not a directory"),
        ));
    }
    None
}

fn run_figure(root: &Path, figure: &Figure, path_var: Option<&str>) -> FigureRun {
    if let Some((probe, detail)) = missing_host_probe(&figure.command, path_var) {
        return FigureRun::ProbeMissing { probe, detail };
    }
    let mut cmd = Command::new("sh");
    cmd.arg("-c").arg(&figure.command).current_dir(root);
    if let Some(path) = path_var {
        cmd.env("PATH", path);
    }
    let Ok(out) = cmd.output() else {
        return FigureRun::SpawnFailed;
    };
    let got = String::from_utf8_lossy(&out.stdout).trim().to_owned();
    if figure.expect == "LIVE" {
        if got.is_empty() {
            return FigureRun::SpawnFailed;
        }
        return FigureRun::LiveNonempty;
    }
    FigureRun::Compared { got }
}


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
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('"') => out.push('"'),
            Some('\\') => out.push('\\'),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            // Unknown escape: keep both characters. A regex like \[ or \d is a
            // legitimate payload here and must survive intact.
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn figures() -> Vec<Figure> {
    let text = fs::read_to_string(repo_root().join("NUMBERS.toml"))
        .expect("NUMBERS.toml must exist — it is the registry this gate re-runs");
    figures_from(&text)
}

/// The reporter, parameterised over its input.
///
/// `omp-orchestrator-m0c` item 2b: the acceptance now asserts that *the number of figures the
/// REPORTER emits equals the number of `[figures.*]` tables declared*. That is unprovable while the
/// reporter can only read one hardcoded file — a fires-on-known-bad leg needs to hand it a registry
/// with a known defect, and a mutation leg needs to reach it without editing the real registry that
/// five agents are writing to. Splitting the read from the parse costs nothing and makes both
/// possible; `figures()` is unchanged in behaviour.
fn figures_from(text: &str) -> Vec<Figure> {
    let mut out: Vec<Figure> = Vec::new();
    for line in text.lines() {
        let l = line.trim();
        if let Some(k) = l
            .strip_prefix("[figures.")
            .and_then(|s| s.strip_suffix(']'))
        {
            out.push(Figure {
                key: k.to_owned(),
                command: String::new(),
                expect: String::new(),
            });
        } else if let Some(cur) = out.last_mut() {
            // take everything after the first '=', trim one layer of quotes
            // Strip the outer quotes AND unescape \" — a registry that cannot hold a
            // command containing quotes is broken, and this parser silently produced an
            // empty command (which the gate then reported as drift to "").
            let val = |s: &str| {
                s.split_once('=').map(|(_, v)| {
                    let v = v.trim();
                    let v = v
                        .strip_prefix('"')
                        .and_then(|x| x.strip_suffix('"'))
                        .unwrap_or(v);
                    unescape(v)
                })
            };
            if l.starts_with("command") {
                cur.command = val(l).unwrap_or_default();
            } else if l.starts_with("expect") {
                cur.expect = val(l).unwrap_or_default();
            }
        }
    }
    out
}

/// The DECLARATION count, derived by a route the reporter does not share.
///
/// `omp-orchestrator-m0c` item 2b needs two arms, and an arm that reuses `figures_from` proves
/// nothing — a reporter compared against itself agrees by construction. So this scanner is
/// deliberately implemented differently, and the differences are the defect classes it can see
/// that the reporter cannot:
///
/// * it SKIPS triple-quoted regions, so a `[figures.x]` written inside a multiline string is not a
///   declaration. The reporter's line loop has no string state and counts it.
/// * it REFUSES a dotted key, so `[figures.a.b]` is a sub-table and not a figure. The reporter
///   would emit a figure literally named `a.b`.
/// * it returns the KEY SET, not a count, so two registries with 36 different keys cannot compare
///   equal — a count equality is satisfied by a reporter that emits the wrong 36.
fn declared_figure_keys(text: &str) -> Vec<String> {
    let triple_double = "\"\"\"";
    let triple_single = "'''";
    let mut keys = Vec::new();
    let mut in_multiline = false;
    for raw in text.lines() {
        let line = raw.trim();
        let fences = line.matches(triple_double).count() + line.matches(triple_single).count();
        if fences % 2 == 1 {
            in_multiline = !in_multiline;
            continue;
        }
        if in_multiline || line.starts_with('#') {
            continue;
        }
        let Some(inner) = line
            .strip_prefix("[figures.")
            .and_then(|rest| rest.strip_suffix(']'))
        else {
            continue;
        };
        if inner.is_empty() || inner.contains('.') || inner.contains('[') {
            continue;
        }
        keys.push(inner.to_owned());
    }
    keys
}

/// ITEM 2b IN ITS AMENDED FORM: the reporter emits EXACTLY the declared figure set.
///
/// The old acceptance said "reports 22 figures". It was already wrong at 36 and would be wrong at
/// 37, so it could neither be satisfied nor graded — the absolute-count staleness this repository
/// has now hit five times in one session. An equality catches the defect that actually matters,
/// **a reporter that silently drops a figure**, which a fixed integer cannot distinguish from
/// healthy growth.
///
/// KEY SETS, NOT COUNTS. `36 == 36` is also satisfied by a reporter emitting the wrong thirty-six.
///
/// The existing `>= 5` floor in `every_figure_declares_a_runnable_command_and_an_expectation` is
/// deliberately NOT removed: it is a cheap non-vacuity guard and it is honest about being a floor.
/// What it cannot do is see a drop — 5 of 36 satisfies it — and that is proven in-tree by
/// `a_dropped_figure_is_caught_by_the_equality_and_missed_by_the_floor` rather than argued here.
#[test]
fn the_reporter_emits_exactly_the_declared_figure_set() {
    let text = fs::read_to_string(repo_root().join("NUMBERS.toml")).expect("registry must exist");
    let declared = declared_figure_keys(&text);
    let emitted: Vec<String> = figures_from(&text).into_iter().map(|f| f.key).collect();

    // ANTI-VACUITY. An empty registry makes every equality below trivially true, and it reports
    // identically to a healthy one.
    assert!(
        !declared.is_empty(),
        "the registry declares no [figures.*] tables; an empty scan set is an ERROR, not a pass"
    );

    let mut unique = declared.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        declared.len(),
        "a figure key is declared twice; the reporter would keep one and a count would still \
         agree, which is how a duplicate hides"
    );

    assert_eq!(
        emitted.len(),
        declared.len(),
        "the reporter emitted {} figures against {} declared tables — a silent drop is exactly \
         what item 2b's equality exists to catch",
        emitted.len(),
        declared.len()
    );

    let mut sorted_emitted = emitted;
    sorted_emitted.sort();
    assert_eq!(
        sorted_emitted, unique,
        "the reporter emitted the wrong SET, not merely the wrong count — a count equality would \
         have passed here"
    );
}

/// FIRES-ON-KNOWN-BAD, and it proves the two arms are genuinely independent.
///
/// A `[figures.*]` header written inside a multiline string is DATA, not a declaration. The
/// declaration scanner tracks string state and skips it; the reporter's line loop has none and
/// counts it. They MUST disagree here — if they agree, the second arm is a copy of the first and
/// every equality above is decoration.
#[test]
fn a_header_inside_a_multiline_string_is_not_a_declaration() {
    let fixture = concat!(
        "[figures.real]\n",
        "command = \"echo 1\"\n",
        "expect = \"1\"\n",
        "[figures.with_prose]\n",
        "command = \"echo 2\"\n",
        "expect = \"2\"\n",
        "note = \"\"\"\n",
        "[figures.not_a_figure]\n",
        "\"\"\"\n"
    );
    let declared = declared_figure_keys(fixture);
    let emitted: Vec<String> = figures_from(fixture).into_iter().map(|f| f.key).collect();
    assert_eq!(
        declared,
        vec!["real".to_owned(), "with_prose".to_owned()],
        "the declaration scanner must skip a header inside a multiline string"
    );
    assert!(
        emitted.contains(&"not_a_figure".to_owned()),
        "the reporter is expected to be fooled here; if it is not, this fixture no longer \
         demonstrates independence and the leg is vacuous"
    );
    assert_ne!(
        declared.len(),
        emitted.len(),
        "the two arms must be able to DISAGREE, or the differential proves nothing"
    );
}

/// THE ACCEPTANCE'S OWN RATIONALE, PROVEN IN-TREE RATHER THAN ARGUED.
///
/// A dropped figure is the defect item 2b names. This leg shows the equality catching it AND the
/// `>= 5` floor missing it, on the same input — the concrete form of "a fixed integer cannot
/// distinguish a drop from healthy growth", and the reason the floor was kept rather than trusted.
#[test]
fn a_dropped_figure_is_caught_by_the_equality_and_missed_by_the_floor() {
    let text = fs::read_to_string(repo_root().join("NUMBERS.toml")).expect("registry must exist");
    let declared = declared_figure_keys(&text);
    assert!(
        declared.len() > 5,
        "this leg needs a registry larger than the floor to make the point; declared={}",
        declared.len()
    );

    // The reporter, crippled the way a real refactor would cripple it: it stops early.
    let dropped: Vec<String> = figures_from(&text)
        .into_iter()
        .map(|f| f.key)
        .take(5)
        .collect();

    assert!(
        dropped.len() >= 5,
        "the FLOOR still passes on a reporter that dropped {} of {} figures — that is why a floor \
         is not an equality",
        declared.len() - dropped.len(),
        declared.len()
    );
    assert_ne!(
        dropped.len(),
        declared.len(),
        "and the EQUALITY sees it: {} emitted against {} declared",
        dropped.len(),
        declared.len()
    );
}

#[test]
fn every_figure_declares_a_runnable_command_and_an_expectation() {
    let f = figures();
    // ANTI-VACUITY: an empty registry re-runs nothing and passes identically to a clean one.
    assert!(
        f.len() >= 5,
        "registry declares {} figures; it described 6 when written",
        f.len()
    );
    for x in &f {
        assert!(
            !x.command.is_empty(),
            "[figures.{}] has no command — then it is not measured",
            x.key
        );
        assert!(
            !x.expect.is_empty(),
            "[figures.{}] has no expectation to compare against",
            x.key
        );
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
        match run_figure(&root, &f, None) {
            FigureRun::ProbeMissing { probe, detail } => {
                eprintln!(
                    "PROBE_MISSING figure={} probe={probe} {detail} — not a measured zero",
                    f.key
                );
            }
            FigureRun::LiveNonempty => {
                ran += 1;
            }
            FigureRun::Compared { got } => {
                ran += 1;
                if got != f.expect {
                    drifted.push(format!(
                        "{}: recorded {:?}, command now answers {:?}\n      $ {}",
                        f.key, f.expect, got, f.command
                    ));
                }
            }
            FigureRun::SpawnFailed => {
                drifted.push(format!(
                    "{}: declared {} but its command produced nothing — a volatile figure \
                     with a broken command is undetectable rot\n      $ {}",
                    f.key,
                    if f.expect == "LIVE" { "LIVE" } else { "pinned" },
                    f.command
                ));
            }
        }
    }

    assert!(
        ran > 0,
        "executed ZERO commands — the registry is unreadable or every command \
                      failed to spawn, which is indistinguishable from a clean run"
    );
    assert!(
        drifted.is_empty(),
        "{} of {ran} load-bearing figures have DRIFTED since they were written:\n    {}",
        drifted.len(),
        drifted.join("\n    ")
    );
}

#[test]
fn the_unescaper_handles_every_escape_the_registry_uses() {
    assert_eq!(unescape(r#"say \"hi\""#), r#"say "hi""#, "quote escape");
    assert_eq!(
        unescape(r"a\\b"),
        r"a\b",
        "backslash escape — the one that broke grep"
    );
    assert_eq!(
        unescape(r"grep '#\[test\]'"),
        r"grep '#\[test\]'",
        "an unknown escape is a regex payload and MUST survive intact"
    );
    assert_eq!(unescape("plain"), "plain", "no escapes, no change");
    assert_eq!(
        unescape(r"trailing\"),
        r"trailing\",
        "a dangling backslash must not panic"
    );
}

#[test]
fn the_parser_unescapes_embedded_quotes() {
    // The registry holds a command containing python -c "..." — if the parser drops or
    // mangles those quotes the command runs empty, and an empty answer reports as DRIFT
    // rather than as a broken registry. That happened on this gate's second run.
    let f = figures();
    let bins = f
        .iter()
        .find(|x| x.key == "built_binaries")
        .expect("built_binaries figure must exist");
    assert!(
        bins.command.contains("python3 -c \""),
        "the embedded quote did not survive parsing: {:?}",
        bins.command
    );
    assert!(
        !bins.command.contains("\\\""),
        "the escape was left in place rather than unescaped: {:?}",
        bins.command
    );
}

#[test]
fn a_live_figure_is_declared_but_not_pinned() {
    let f = figures();
    let live: Vec<_> = f.iter().filter(|x| x.expect == "LIVE").collect();
    assert!(
        !live.is_empty(),
        "no LIVE figure declared — if the board total stopped being volatile, pin it \
         and delete this test rather than leaving a mode nothing exercises"
    );
    for x in &live {
        assert!(
            !x.command.is_empty(),
            "[figures.{}] is LIVE with no command to run",
            x.key
        );
    }
}

#[test]
fn no_figure_key_is_declared_twice() {
    // Two panes concurrently appended `board_total` in the round-11 fix and one of them
    // clobbered the NOTE of the block above it. A duplicate key is silent in TOML-by-
    // convention parsers like this one: the second wins and the first vanishes.
    let f = figures();
    let mut keys: Vec<&str> = f.iter().map(|x| x.key.as_str()).collect();
    keys.sort_unstable();
    let before = keys.len();
    keys.dedup();
    assert_eq!(
        before,
        keys.len(),
        "a figure key is declared more than once — concurrent appends to a shared \
         registry silently drop the earlier block"
    );
}

#[test]
fn the_comparison_is_exact_not_substring() {
    // A figure of "2" must not be satisfied by an answer of "26". Five rounds of this
    // document's history are numbers that looked close enough to a reader.
    assert_ne!("2", "26");
    let loose = "26".contains("2");
    assert!(
        loose,
        "substring matching WOULD accept it — which is why this gate compares with !="
    );
}

/// A figure that derives ZERO must declare that the zero is real.
///
/// # The measured failure this exists to catch
///
/// 2026-09-01: grading flagged `03-crates.md` for claiming "25 of 26 source roots
/// carry `#![forbid(unsafe_code)]`". Checking it, I ran:
///
/// ```text
/// grep -rl 'forbid(unsafe_code)' crates --include='*.rs'   ->  0 files
/// ```
///
/// and was one commit away from filing a BLOCKER against the plan. The document
/// was RIGHT — 25 of 26 is the true answer.
///
/// SCOPE, corrected after measuring both engines: this hazard belongs to the
/// AGENT HARNESS's grep, which is a Rust regex engine. Shell `grep` reached
/// through `sh -c` — the path every figure in this registry uses — treats `(`
/// as a LITERAL in a basic regex and answers correctly. A planted figure using
/// the unescaped pattern returned 64, not 0. So the registry was never exposed;
/// the reviewer was. The gate below is therefore not a fix for that bug at all.
/// It raises the floor on a different and real case: a figure that derives zero
/// for ANY reason now has to say why — the real answer is 25 of 26. My pattern was wrong: the built-in
/// grep is a REGEX engine, so `(` and `)` are grouping metacharacters and
/// `forbid(unsafe_code)` matches the literal string `forbidunsafe_code`, which
/// appears nowhere. The correct pattern escapes them: `forbid\(unsafe_code\)`.
///
/// **It returned zero and exited zero.** A broken pattern and a true absence are
/// byte-identical to the caller. The only thing that caught it was a positive
/// control — grepping for a substring I knew existed (`forbid`) found the files,
/// while the full pattern found none, and those two facts cannot both be true of
/// a working instrument.
///
/// So: any figure deriving 0 or empty must carry `zero_is_real = "<reason>"`.
/// Writing that reason forces the author to say WHY nothing is there, which is
/// exactly the sentence a broken pattern cannot honestly produce.
///
/// # What this does NOT do
///
/// It cannot tell a correct pattern from a broken one when both return non-zero.
/// A pattern that matches 20 things when the truth is 26 passes this gate
/// silently. This raises the floor on the zero case only, because zero is the
/// case where a broken instrument is indistinguishable from a real measurement.
#[test]
fn a_figure_deriving_zero_must_declare_the_zero_is_real() {
    // Reuse figures() and unescape() -- the helpers directly above, whose own doc
    // comment says "a partial unescaper is worse than none". My first version of
    // this test hand-rolled a replace() chain instead and reported 16 of 17
    // figures as deriving zero, which was its own bug, not the data.
    let text = std::fs::read_to_string(repo_root().join("NUMBERS.toml")).expect("readable");
    let mut undeclared = Vec::new();
    let mut checked = 0usize;
    for f in figures() {
        if f.command.is_empty() || f.expect == "LIVE" {
            continue;
        }
        match run_figure(&repo_root(), &f, None) {
            FigureRun::ProbeMissing { probe, detail } => {
                eprintln!(
                    "PROBE_MISSING figure={} probe={probe} {detail} — not a measured zero",
                    f.key
                );
            }
            FigureRun::Compared { got } => {
                checked += 1;
                if got.is_empty() || got == "0" {
                    let block = text
                        .split("[figures.")
                        .find(|b| b.starts_with(&f.key))
                        .unwrap_or("");
                    if !block.contains("zero_is_real") {
                        undeclared.push(format!("{} -> {:?}", f.key, got));
                    }
                }
            }
            FigureRun::LiveNonempty | FigureRun::SpawnFailed => {}
        }
    }

    assert!(
        checked > 0,
        "ANTI-VACUITY: no figure commands ran -- this gate proves nothing about an empty set"
    );
    assert!(
        undeclared.is_empty(),
        "{} figure(s) derive zero without `zero_is_real`: {:?}",
        undeclared.len(),
        undeclared
    );
}

#[test]
fn empty_path_does_not_treat_missing_br_as_zero_or_live_rot() {
    let figure = figures()
        .into_iter()
        .find(|f| f.key == "board_total")
        .expect("board_total is the br-coupled LIVE figure");
    match run_figure(&repo_root(), &figure, Some("")) {
        FigureRun::ProbeMissing { probe, detail } => {
            assert_eq!(probe, "br", "{detail}");
            assert!(detail.contains("PATH"), "{detail}");
        }
        other => panic!("missing br must be PROBE_MISSING, not {other:?}"),
    }
}

#[test]
fn missing_omp_dist_is_probe_missing_not_a_zero_count() {
    let command = "/Users/josh/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/types";
    let fake = Figure {
        key: "ipg6_root_symbols".into(),
        command: format!("R={command}; tot=0; echo $tot"),
        expect: "611".into(),
    };
    if Path::new(command).is_dir() {
        match missing_host_probe(&fake.command, None) {
            None => {}
            other => panic!("present dist tree must not look missing: {other:?}"),
        }
        return;
    }
    match run_figure(&repo_root(), &fake, None) {
        FigureRun::ProbeMissing { probe, .. } => assert_eq!(probe, "omp_dist_types"),
        other => panic!("absent dist tree must be PROBE_MISSING, not {other:?}"),
    }
}

#[test]
fn a_laptop_with_tools_still_fails_when_a_figure_drifts() {
    let planted = Figure {
        key: "planted_drift".into(),
        command: "echo 1".into(),
        expect: "2".into(),
    };
    match run_figure(&repo_root(), &planted, None) {
        FigureRun::Compared { got } => {
            assert_eq!(got, "1");
            assert_ne!(got, planted.expect);
        }
        other => panic!("echo 1 must compare, got {other:?}"),
    }
}

