//! THE INSTRUMENT THAT VALIDATES EVERY OTHER INSTRUMENT, FINALLY MEASURED.
//!
//! Every commit in this repo is certified by a shell readback: hash the worktree file, hash
//! `git show HEAD:<path>`, compare. `omp-orchestrator-czrvd` measured 2026-09-12 that the idiom
//! everyone uses cannot distinguish a real match from a comparison of two empty strings.
//!
//! BOTH DIRECTIONS ARE REAL AND ONLY ONE IS SURVIVABLE:
//!   * ONE operand empty  -> `[ "" = "b8fd876…" ]` -> DIVERGED. A FALSE ALARM. Benign, because the
//!     author investigates and finds the tree clean. This is how the defect was caught at all.
//!   * BOTH operands empty -> `[ "" = "" ]` -> IDENTICAL. SILENT. Nobody investigates a green.
//!
//! These legs run the ACTUAL SHELL, not a Rust re-implementation of it. A re-implementation would
//! prove that a function I just wrote behaves as I wrote it; only `sh` can prove what the idiom
//! pasted into a hundred callbacks actually does. That distinction is the same one that made
//! `l6hsl` item A a defect: a leg that constructs its own subject proves nothing about the real
//! one.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The UNGUARDED idiom, verbatim as it appears across this repo's callbacks.
const UNGUARDED: &str = r#"[ "$a" = "$b" ] && echo IDENTICAL || echo DIVERGED"#;

/// The GUARDED idiom, verbatim as `AGENTS.md` rule `8x` prescribes it.
///
/// The empty check comes FIRST and short-circuits: `READBACK_UNMEASURABLE` is a THIRD STATE, not a
/// flavour of pass and not a flavour of failure. Collapsing it into either is the defect.
const GUARDED: &str = r#"if [ -z "$a" ] || [ -z "$b" ]; then echo READBACK_UNMEASURABLE
elif [ "$a" = "$b" ]; then echo IDENTICAL
else echo DIVERGED; fi"#;

fn run_idiom(idiom: &str, a: &str, b: &str) -> String {
    let output = Command::new("sh")
        .arg("-c")
        .arg(format!("a={a}\nb={b}\n{idiom}"))
        .output()
        .expect("sh must spawn — without it this whole file measures nothing");
    assert!(
        output.status.success(),
        "the idiom must not error; it must RETURN A VERDICT: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// POSITIVE CONTROL FOR THE WHOLE FILE, AND THE KNOWN-BAD `czrvd` DEMANDED.
///
/// If this leg ever goes green by returning something other than `IDENTICAL`, the defect has
/// stopped reproducing and EVERY OTHER LEG HERE IS MEASURING NOTHING. That is why it is pinned as
/// an assertion rather than left as a comment: an anti-vacuity control that is not executed is a
/// claim, and this repo has paid for those.
#[test]
fn the_unguarded_idiom_reports_agreement_between_two_empty_strings() {
    assert_eq!(
        run_idiom(UNGUARDED, "", ""),
        "IDENTICAL",
        "THE SILENT FORM MUST STILL REPRODUCE. If `sh` no longer prints IDENTICAL for two empty \
         operands, rule 8x's premise is dead and the guarded legs below prove nothing."
    );
}

/// The benign direction, pinned so the two are never conflated in a report.
#[test]
fn the_unguarded_idiom_reports_a_false_divergence_when_only_the_pin_is_empty() {
    assert_eq!(
        run_idiom(UNGUARDED, "", "b8fd876"),
        "DIVERGED",
        "one empty operand must raise a FALSE ALARM, not a false green — this is the direction \
         that is survivable, and the reason the defect was findable at all"
    );
}

/// THE REMEDY, ON THE EXACT INPUT THAT DEFEATS THE UNGUARDED FORM.
#[test]
fn the_guarded_idiom_refuses_two_empty_strings_instead_of_agreeing() {
    assert_eq!(
        run_idiom(GUARDED, "", ""),
        "READBACK_UNMEASURABLE",
        "the guarded form must REFUSE where the unguarded form AGREES — same input, opposite verdict"
    );
}

#[test]
fn the_guarded_idiom_refuses_when_either_operand_alone_is_empty() {
    assert_eq!(
        run_idiom(GUARDED, "", "b8fd876"),
        "READBACK_UNMEASURABLE",
        "an absent pin is UNMEASURABLE, not DIVERGED: reporting divergence against nothing sends \
         the reader to look for a tree difference that does not exist"
    );
    assert_eq!(
        run_idiom(GUARDED, "b8fd876", ""),
        "READBACK_UNMEASURABLE",
        "and it must be symmetric — the empty side is not always the pin"
    );
}

/// The guard must not have bought refusal by destroying the verdicts it exists to protect.
///
/// A remedy that refuses EVERYTHING is a gate that proves nothing, which is the failure mode this
/// repo calls self-weakening. Both live answers must survive intact.
#[test]
fn the_guarded_idiom_still_answers_identical_and_diverged_on_real_operands() {
    assert_eq!(run_idiom(GUARDED, "b8fd876", "b8fd876"), "IDENTICAL");
    assert_eq!(run_idiom(GUARDED, "b8fd876", "fe892b6"), "DIVERGED");
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root two levels up")
        .to_path_buf()
}

/// THE RULE MUST BE WHERE READBACKS ARE PRESCRIBED, NOT ONLY WHERE THEY ARE TESTED.
///
/// A green suite proving the guarded idiom works is worthless if the doctrine every agent copies
/// from still shows the unguarded one. This leg reads `AGENTS.md` and requires the canonical
/// guarded form to be present there verbatim.
///
/// ANTI-VACUITY: an unreadable or rule-less `AGENTS.md` is an ERROR, never a pass. A scan that
/// finds nothing to check must say so rather than reporting success over an empty set — the same
/// rule the readback itself was violating.
#[test]
fn the_canonical_guarded_readback_is_prescribed_in_agents_md() {
    let path = repo_root().join("AGENTS.md");
    let doctrine = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("AGENTS.md must be readable: {error}"));
    assert!(
        !doctrine.trim().is_empty(),
        "ANTI-VACUITY: AGENTS.md is empty, so every needle below would be trivially absent and \
         this leg would report a meaningless red"
    );
    assert!(
        doctrine.contains("READBACK_UNMEASURABLE"),
        "rule 8x's third state must be NAMED in the doctrine agents copy from, not only in tests"
    );
    for clause in [
        r#"if [ -z "$a" ] || [ -z "$b" ]; then echo READBACK_UNMEASURABLE"#,
        r#"elif [ "$a" = "$b" ]; then echo IDENTICAL"#,
    ] {
        assert!(
            doctrine.contains(clause),
            "the canonical guarded readback must appear VERBATIM in AGENTS.md so it is copied \
             rather than re-derived; missing: {clause}"
        );
    }
}

/// THE FOURTH CLAUSE IS THE ONE THE GUARDED IDIOM CANNOT CATCH, SO IT NEEDS ITS OWN ENFORCEMENT.
///
/// Clauses 1-3 are all defeated by an EMPTY operand and all rescued by a non-empty guard. The
/// attribution failure has NO empty operand: the worktree blob is right, the `HEAD:` blob is right,
/// they genuinely match, and the conclusion "therefore MY commit landed" is still false because a
/// peer's path-scoped commit carried the content. Measured from both sides on 2026-09-12 --
/// `250db103` (an agent's hunks landed under a peer's sha) and `990c8ea` (an agent's `git push`
/// pushed a peer's commit while its own never landed).
///
/// There is nothing to assert non-empty here, so the only defence is a DIFFERENT QUESTION:
/// identity against HEAD instead of presence in the tree. This leg requires the doctrine to ask it.
#[test]
fn the_attribution_clause_and_its_two_commands_are_prescribed_in_agents_md() {
    let doctrine = std::fs::read_to_string(repo_root().join("AGENTS.md"))
        .unwrap_or_else(|error| panic!("AGENTS.md must be readable: {error}"));
    assert!(
        !doctrine.trim().is_empty(),
        "ANTI-VACUITY: an empty AGENTS.md would make every needle below trivially absent"
    );
    for needle in [
        // The claim itself, so a reader meets it before the commands.
        "IT SAYS NOTHING",
        // The two commands, verbatim, because a clause without its procedure gets re-derived
        // and the re-derivation is what produced "needle present" in the first place.
        "git rev-parse HEAD",
        "git show --stat --format= HEAD",
    ] {
        assert!(
            doctrine.contains(needle),
            "the attribution clause must carry its own procedure VERBATIM; missing: {needle}"
        );
    }
}

/// A COUNT OR AN ABSENCE TAKEN FROM A TRUNCATED LISTING IS A PROPERTY OF THE TRUNCATION.
///
/// Three instances on 2026-09-12, two of them by agents who then had to retract a published figure.
/// This is NOT a readback failure -- no comparison is involved -- which is exactly why it must be
/// pinned separately rather than folded into `8x`'s clauses, where it would be looked for only by
/// someone already debugging a readback.
#[test]
fn the_truncated_listing_hazard_is_recorded_beside_the_clause() {
    let doctrine = std::fs::read_to_string(repo_root().join("AGENTS.md"))
        .unwrap_or_else(|error| panic!("AGENTS.md must be readable: {error}"));
    assert!(
        doctrine.contains("A head ON A LISTING IS A COUNT OF THE HEAD."),
        "the truncated-listing hazard must be stated in the doctrine, not only in a callback"
    );
}
