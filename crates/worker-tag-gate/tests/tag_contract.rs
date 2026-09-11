//! Conformance suite for [`worker_tag_gate`] — the WIRING for a gate that was otherwise a bin
//! subcommand nobody calls.
//!
//! `AGENTS.md`'s third rule: *"no gate may exist without a reachable trigger… A gate that cannot
//! fire is worse than no gate, because the repo READS as protected."* The `--selftest` verb proved
//! the logic but had zero callers, which is BUILT ≠ WIRED at test granularity — the same shape as
//! rule 6's `#[ignore]`d leg. `cargo test -p worker-tag-gate --test tag_contract` is what the fleet
//! actually runs, so the legs live here.
//!
//! **Every known-bad leg asserts the MESSAGE *and* the exit code**, per rule 7 as corrected at
//! `17f3357`: a message-only assertion is defeated when two causes collapse into one code
//! (measured on `2sx1` M1), and a code-only assertion is defeated by any unrelated breakage
//! (measured on the `101` workspace-load outage).

use worker_tag_gate::{check, parse_workers, GateError, WorkerRow};

/// The exact shape measured on `contabo-3` on 2026-09-07, which made our own assigned lane refuse
/// every native build.
const MEASURED_BAD: &str = r#"
[[workers]]
id = "contabo-3"
tags = ["linux", "x86_64", "rust", "os:darwin"]
enabled = true
"#;

/// A real Mac may declare `os:darwin`; that is `HD-0013`'s split and must stay legal.
const MEASURED_GOOD: &str = r#"
[[workers]]
id = "zestdata-local"
tags = ["os:darwin", "macos", "aarch64", "artifact", "zestdata"]
enabled = true

[[workers]]
id = "contabo-3"
tags = ["linux", "x86_64", "rust"]
enabled = true
"#;

#[test]
fn known_good_a_real_mac_may_declare_os_darwin() {
    let rows = check(MEASURED_GOOD, "fixture:good").expect("a Mac declaring os:darwin is legal");
    assert_eq!(rows.len(), 2, "both rows must be scanned");
    assert!(
        rows.iter().any(|r| r.id == "zestdata-local" && r.declares_os_darwin()),
        "the Mac's tag must survive — an over-strict gate gets routed around"
    );
}

#[test]
fn known_bad_a_linux_box_declaring_os_darwin_is_refused_with_message_and_code() {
    let error = check(MEASURED_BAD, "fixture:bad").expect_err("must refuse");

    // The CODE half.
    assert_eq!(
        error.exit_code(),
        2,
        "offender cause must own exit code 2, distinct from unreadable/empty"
    );
    assert_eq!(error.code(), "OS_DARWIN_ON_NON_DARWIN_HOST");

    // The MESSAGE half — three independent substrings, so a reworded prefix cannot hide a
    // silently-narrowed check.
    let text = error.to_string();
    assert!(text.contains("OS_DARWIN_ON_NON_DARWIN_HOST"), "code in message: {text}");
    assert!(text.contains("contabo-3"), "offending id named: {text}");
    assert!(
        text.contains("REFUSE EVERY NATIVE BUILD"),
        "message must state the CONSEQUENCE, not just the rule: {text}"
    );
}

#[test]
fn anti_vacuity_an_empty_scan_set_is_an_error_never_a_pass() {
    let error = check("# only a comment\n", "fixture:empty").expect_err("empty must not pass");
    assert!(matches!(error, GateError::NoWorkers { .. }));
    assert_eq!(error.exit_code(), 4);
    assert!(
        error.to_string().contains("an empty scan set is an ERROR"),
        "the refusal must say WHY zero rows is not a pass"
    );
}

#[test]
fn a_commented_out_tag_is_not_a_live_tag() {
    // `AGENTS.md`: a doc comment warning about a needle contained the needle, and the census went
    // GREEN after its subject was deleted. Over-stripping is the safe direction.
    let commented = r#"
[[workers]]
id = "contabo-2"
tags = ["linux", "x86_64", "rust"]
# tags = ["linux", "x86_64", "rust", "os:darwin"]   <- retired 2026-09-07, must not count
enabled = true
"#;
    let rows = check(commented, "fixture:commented").expect("a retired tag must not refuse");
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].declares_os_darwin(), "commented tag was read as live");
}

#[test]
fn every_cause_owns_a_distinct_exit_code() {
    let codes = [
        GateError::DarwinTagOnNonDarwinHost { offenders: vec![] }.exit_code(),
        GateError::Unreadable {
            path: String::new(),
            detail: String::new(),
        }
        .exit_code(),
        GateError::NoWorkers {
            path: String::new(),
        }
        .exit_code(),
    ];
    let mut sorted = codes;
    sorted.sort_unstable();
    assert!(
        sorted.windows(2).all(|w| w[0] != w[1]),
        "causes share an exit code, so a caller cannot discriminate: {codes:?}"
    );
    assert!(
        !codes.contains(&0),
        "no failure cause may use 0 — that is success"
    );
}

#[test]
fn an_unreadable_file_is_unknown_and_says_so() {
    // `AGENTS.md`: "A DENIED OR ERRORED PROBE IS *UNKNOWN*, NEVER A NEGATIVE RESULT."
    let error = GateError::Unreadable {
        path: "/nonexistent/workers.toml".to_owned(),
        detail: "No such file or directory".to_owned(),
    };
    assert_eq!(error.exit_code(), 3);
    let text = error.to_string();
    assert!(
        text.contains("UNKNOWN, not a pass"),
        "an unreadable subject must not read as a clean one: {text}"
    );
}

#[test]
fn the_parser_reads_id_tags_and_enabled_from_a_realistic_body() {
    let rows = parse_workers(MEASURED_GOOD);
    assert_eq!(rows.len(), 2);
    let mac = rows.iter().find(|r| r.id == "zestdata-local").expect("mac row");
    assert!(mac.enabled, "enabled = true must parse");
    assert!(mac.is_darwin_host(), "macos tag marks a darwin host");
    let linux = rows.iter().find(|r| r.id == "contabo-3").expect("linux row");
    assert!(linux.is_linux_host());
    assert!(!linux.is_darwin_host(), "a linux box is never a darwin host");
}

#[test]
fn a_darwin_host_tag_is_required_not_merely_a_macos_looking_id() {
    // An id is not evidence. Only a host TAG licenses the os:darwin declaration, so a Linux box
    // renamed `joshs-brain-2` cannot smuggle the tag past this gate.
    let by_name_only = r#"
[[workers]]
id = "joshs-brain-2"
tags = ["linux", "x86_64", "rust", "os:darwin"]
enabled = true
"#;
    let error = check(by_name_only, "fixture:name").expect_err("id must not license the tag");
    assert_eq!(error.exit_code(), 2);
    assert!(error.to_string().contains("joshs-brain-2"));
}

#[test]
fn the_allowed_host_tag_set_is_declared_not_inferred() {
    // A named constant, so an exception is a row with a reason rather than a silent widening.
    assert!(WorkerRow::DARWIN_HOST_TAGS.contains(&"macos"));
    assert!(!WorkerRow::DARWIN_HOST_TAGS.contains(&"linux"));
    assert_eq!(WorkerRow::OS_DARWIN, "os:darwin");
}

/// This crate's own source, read as text so the legs below can key on CLAIM TEXT rather than a
/// line number. `:24-26` and `:157` both move the next time anyone edits this file — `AGENTS.md`
/// moved a cited line from `:48` to `:68` inside one night, by the very commit that fixed the
/// thing the cite was about.
const THIS_CRATE_SOURCE: &str = include_str!("../src/lib.rs");

/// Every in-source surface that still asserts darwin artifacts belong on the local Mac. HD-0013's
/// decision stands and its quoted text is never edited; what must never again appear ALONE is the
/// stale consequence, because a reader who meets it without the cross-build form stops.
const STALE_CONSEQUENCE_CLAIMS: &[&str] = &[
    // The HD-0013 ruling, quoted verbatim in the module docs.
    "the LOCAL MAC for",
    // The refusal message a reader actually receives from this gate.
    "darwin artifacts belong on the local Mac",
];

/// The amendment must be reachable from the claim, not merely present somewhere in the file. An
/// amendment a reader only meets AFTER acting on the stale text is not an amendment.
const AMENDMENT_WINDOW_LINES: usize = 40;

/// Needles proving the working form travels with the claim: the target triple, and the trap that
/// `--target` is the wrong flag.
const AMENDMENT_NEEDLES: &[&str] = &["aarch64-apple-darwin", "--target"];

/// Does `pointer` occur within `window` lines at or after every line matching `claim`?
///
/// Returns the count of claim sites checked, so a caller can refuse a vacuous pass: a needle that
/// matches nothing would otherwise satisfy a for-all assertion trivially.
fn claim_sites_carrying(source: &str, claim: &str, pointer: &str, window: usize) -> (usize, usize) {
    let lines: Vec<&str> = source.lines().collect();
    let mut sites = 0usize;
    let mut carried = 0usize;
    for (idx, line) in lines.iter().enumerate() {
        if !line.contains(claim) {
            continue;
        }
        sites += 1;
        let end = (idx + window + 1).min(lines.len());
        if lines[idx..end].iter().any(|l| l.contains(pointer)) {
            carried += 1;
        }
    }
    (sites, carried)
}

#[test]
fn every_stale_local_mac_claim_carries_the_cross_build_form_at_the_claim() {
    for claim in STALE_CONSEQUENCE_CLAIMS {
        for needle in AMENDMENT_NEEDLES {
            let (sites, carried) =
                claim_sites_carrying(THIS_CRATE_SOURCE, claim, needle, AMENDMENT_WINDOW_LINES);
            // Anti-vacuity: a claim needle that stopped matching is UNKNOWN, not absence. If the
            // wording is deliberately changed, this leg must be re-derived, not silently pass.
            assert!(
                sites > 0,
                "claim needle {claim:?} matched ZERO lines — an empty scan set is an ERROR, never a \
                 pass; re-derive this leg against the current wording"
            );
            assert_eq!(
                carried, sites,
                "{sites} site(s) contain {claim:?} but only {carried} carry {needle:?} within \
                 {AMENDMENT_WINDOW_LINES} lines — the stale consequence is unamended AT the claim"
            );
        }
    }
}

#[test]
fn the_refusal_message_itself_names_the_cross_build_and_still_exits_2() {
    // The strongest, position-free form of the leg above: what a reader actually RECEIVES.
    let error = check(MEASURED_BAD, "fixture:bad").expect_err("os:darwin on a linux box must fail");
    let text = error.to_string();
    assert_eq!(error.exit_code(), 2, "cause-specific exit code, per rule 7: {text}");
    assert!(
        text.contains("OS_DARWIN_ON_NON_DARWIN_HOST"),
        "code in message: {text}"
    );
    for needle in ["aarch64-apple-darwin", "--target", "rc=103"] {
        assert!(
            text.contains(needle),
            "refusal must name the cross-build form and its traps; missing {needle:?} in: {text}"
        );
    }
}
