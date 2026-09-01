#![forbid(unsafe_code)]
//! CITATION INTEGRITY GATE — every external type path this plan cites must exist,
//! and every symbol it names must be findable in the file it names.
//!
//! # Why this exists
//!
//! Round 13, `GradeActions` filed a BLOCKER against `05-actions.md`:
//!
//! > The document cites TypeScript types from `dist/types/` paths as external
//! > validation of design, but provides no evidence that these files exist or that
//! > the quoted signatures are current. […] a buyer relying on alignment with
//! > external substrate types has no way to verify the claims.
//!
//! Checked by hand, all 8 cited paths existed and all 8 named symbols were
//! present. The grader was still right, because **the plan shipped no way to
//! check**, and this session had already produced two fabricated citations that a
//! reader could not have caught either:
//!
//! - a verbatim quote attributed to `frankenterm` that does not appear in the
//!   cited file (retracted in-tree)
//! - `SessionStopEvent.settle`, cited as an API, which is not a member of that
//!   interface at all — the word `settle` appears only in a doc comment on a
//!   different field (retracted in-tree)
//!
//! Both were caught by a human re-reading, which does not scale and did not
//! generalise. This gate is the mechanism.
//!
//! # What it enforces, mechanically
//!
//! 1. Every `dist/types/**/*.d.ts` path appearing anywhere in `docs/plan/` must
//!    exist under the installed OMP package.
//! 2. Every symbol cited in the form `` `Symbol` (`dist/types/x/y.d.ts`) `` must
//!    appear in that file.
//!
//! # What still passes
//!
//! A symbol that exists but whose *shape* we describe wrongly. `AgentEndEvent`
//! having a `willContinue` field does not prove our sentence about what it means
//! is true. This gate makes a **nonexistent type** citation impossible and leaves a
//! **misread** one possible.
//!
//! **It also does NOT catch a fabricated MEMBER, and that is measured rather than
//! assumed.** I built the member leg, ran it against the real retraction from this
//! session — `SessionStopEvent.settle` — and it passed: `settle` occurs in that
//! file inside a doc comment on a different field, so substring containment
//! returns true. The leg was removed rather than shipped green, because a gate that
//! passes on the precise defect it advertises is a fooled certificate. Catching it
//! requires parsing the interface body, which is unbuilt.
//!
//! It also cannot fire when OMP is not installed. That case is an explicit SKIP
//! with a printed reason, never a silent pass, because a gate that reports green
//! on a missing subject is the vacuous-pass defect this repo refuses.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root two levels up")
        .to_path_buf()
}

fn omp_root() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let p = PathBuf::from(home).join(".local/lib/node_modules/@oh-my-pi/pi-coding-agent");
    if p.is_dir() { Some(p) } else { None }
}

fn plan_text() -> String {
    let dir = repo_root().join("docs/plan");
    let mut all = String::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            if p.extension().is_some_and(|x| x == "md") {
                if let Ok(t) = std::fs::read_to_string(&p) {
                    all.push_str(&t);
                    all.push('\n');
                }
            }
        }
    }
    all
}

/// Pull every `dist/types/...d.ts` substring out of the plan.
fn cited_paths(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (i, _) in text.match_indices("dist/types/") {
        let rest = &text[i..];
        let end = rest.find(".d.ts").map(|e| e + 5);
        if let Some(end) = end {
            let cand = &rest[..end];
            // reject anything with whitespace or markdown punctuation inside
            if !cand.contains(char::is_whitespace) && !cand.contains('`') && !cand.contains(')') {
                out.push(cand.to_owned());
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

#[test]
fn every_cited_type_path_exists() {
    let Some(root) = omp_root() else {
        eprintln!("SKIP every_cited_type_path_exists: OMP not installed at \
                   ~/.local/lib/node_modules/@oh-my-pi/pi-coding-agent — \
                   the subject is absent, so this proves nothing and says so");
        return;
    };
    let text = plan_text();
    let paths = cited_paths(&text);
    assert!(
        !paths.is_empty(),
        "ANTI-VACUITY: the plan cites zero dist/types paths — either the scan is \
         broken or the external-validation claims are gone. Both are findings."
    );
    let missing: Vec<_> = paths.iter().filter(|p| !root.join(p).is_file()).collect();
    assert!(
        missing.is_empty(),
        "{} cited type path(s) do not exist under the installed OMP package: {:#?}\n\
         A citation to a file that is not there is indistinguishable from a fabrication.",
        missing.len(),
        missing
    );
}

#[test]
fn every_cited_symbol_appears_in_the_file_it_names() {
    let Some(root) = omp_root() else {
        eprintln!("SKIP every_cited_symbol_appears_in_the_file_it_names: OMP not installed");
        return;
    };
    let text = plan_text();
    // ANCHOR ON THE PATH, NOT ON BACKTICKS.
    //
    // The first version matched only `` `Symbol` (`dist/types/x.d.ts`) `` — symbol in
    // backticks, open paren immediately followed by a backtick. The plan does not write
    // that. It writes, in §4 of 00-brief and 31 other places:
    //
    //     | idle | GuestIdleReconcilerCtx (dist/types/collab/guest.d.ts:9-30) | **DE...
    //
    // No backticks anywhere. So the parser matched ZERO of 32 real citations and the
    // anti-vacuity leg fired — correctly, and it is the only reason this was caught:
    // without it the gate would have reported a serene green over a plan it never read.
    // That is the third reader-scoped-to-a-guess defect in this session (a `^??` grep
    // where `?` is a quantifier so it matched every line; a sweep restricted to files
    // NAMED dispatch_cli_contract.rs while the same code sat in main.rs).
    //
    // The original comment's caution is RIGHT and is preserved: "a loose scan would
    // sweep prose words into symbol position and produce false failures, which is worse
    // than a smaller gate." So rather than loosening the symbol side, this anchors on
    // the one unambiguous token — `dist/types/` inside parentheses — and walks BACKWARD
    // for the symbol, keeping the same plausibility filter. Both the backticked and the
    // bare form are accepted because both appear in the plan and both are legitimate.
    let mut pairs: Vec<(String, String)> = Vec::new();
    for (idx, _) in text.match_indices("(dist/types/").chain(text.match_indices("(`dist/types/")) {
        let after = &text[idx..];
        let Some(e) = after.find(".d.ts") else { continue };
        let path = after[1..e + 5].trim_start_matches('`').to_owned();

        // Walk back over the space(s) before '(' and take the preceding token.
        let before = &text[..idx];
        let trimmed = before.trim_end_matches([' ', '\t', '`']);
        let sym_raw = trimmed
            .rsplit(|c: char| c.is_whitespace() || c == '|' || c == '`')
            .next()
            .unwrap_or("");
        let clean_sym = sym_raw.split(['<', '(']).next().unwrap_or("").trim().to_owned();

        // Unchanged from the original: an uppercase-initial identifier-shaped token.
        // This is what stops a prose word landing in symbol position.
        let plausible = !clean_sym.is_empty()
            && clean_sym.chars().next().is_some_and(|c| c.is_ascii_uppercase())
            && clean_sym.chars().all(|c| c.is_alphanumeric() || c == '_' || c == '.');
        if plausible {
            pairs.push((clean_sym, path));
        }
    }
    pairs.sort();
    pairs.dedup();
    assert!(
        !pairs.is_empty(),
        "ANTI-VACUITY: no `Symbol` (`dist/types/...`) citations parsed. The plan \
         contains such citations, so a zero here means this parser is broken — \
         which is the silent-false-zero defect, not a pass."
    );
    let mut bad = Vec::new();
    for (sym, path) in &pairs {
        let f = root.join(path);
        match std::fs::read_to_string(&f) {
            Ok(body) => {
                // TYPE EXISTENCE ONLY. A dotted citation names two claims — the type
                // exists, and the member exists on it — and this gate enforces only
                // the first. That is not laziness; it is measured.
                //
                // I extended this check to members, ran it against the real
                // fabrication we retracted tonight (`SessionStopEvent.settle`), and
                // IT PASSED. The word `settle` does occur in that file: in a doc
                // comment, on a different field. `body.contains("settle")` is true
                // and says nothing about membership.
                //
                // Substring containment cannot distinguish an API member from a word
                // in a comment, so a member leg built on it is a gate that reports
                // green on the exact citation class it claims to catch — worse than
                // no gate, because it invites a reader to stop looking. Proving
                // membership needs the interface body parsed; that is unbuilt.
                let ty = sym.split('.').next().unwrap_or("");
                if !ty.is_empty() && !body.contains(ty) {
                    bad.push(format!("type `{ty}` not found in {path}"));
                }
            }
            Err(_) => bad.push(format!("{path} unreadable (cited for `{sym}`)")),
        }
    }
    assert!(
        bad.is_empty(),
        "{} citation(s) name a symbol absent from the file cited: {:#?}\n\
         This is the exact shape of the two fabrications retracted this session \
         (a frankenterm quote, and SessionStopEvent.settle).",
        bad.len(),
        bad
    );
}
