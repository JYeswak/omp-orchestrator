//! COVERAGE ROW GATE — every census row must state a disposition, and `null` must
//! never be one of the answers.
//!
//! # The measured failure
//!
//! `Lens02Census`, the held-out operator-at-3am lens, filed BLOCKED:
//!
//! > 54 type_root rows readable but **disposition not stated**.
//!
//! Checked in the durable artifacts: **6 of 14 rows** carried `disposition: null`.
//! Every one of the six had `classification: "a"` — the taxonomy's *NOT OURS* — so
//! the null did not mean "nobody decided". It meant "not applicable".
//!
//! **A reader cannot tell those apart, and that is the whole defect.** `null` was
//! doing two jobs: *undecided* and *inapplicable*. One of them is a gap that needs
//! closing and the other is a settled answer, and they rendered identically.
//!
//! This is the third instance of the same shape in one session:
//!
//! | field | the two meanings it carried |
//! |---|---|
//! | `gates_green` | "the workspace passes" vs "19 of 119 suites pass" |
//! | cited SHAs | a BROKEN citation vs a VALID citation of someone else's work |
//! | `disposition: null` | undecided vs not-applicable |
//!
//! Each was fixed the same way: make the second meaning explicit so the first
//! becomes detectable. `N/A-NOT-OURS` is not more informative than `null` about the
//! surface — it is more informative about **whether a human still owes a decision**.
//!
//! # What this cannot do
//!
//! It checks that a disposition is STATED, not that it is CORRECT. A row marked
//! `RETIRE` on a surface we should adopt passes here. `session` — 78 files, 499
//! symbols, zero imported — is the standing reminder that a confident disposition
//! can be wrong.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root two levels up")
        .to_path_buf()
}

/// Crude field extraction. Deliberately no serde: this gate must never fail to
/// build, and a gate that drags a dependency in to read four fields has bought
/// fragility for convenience.
fn rows_of(text: &str) -> Vec<String> {
    // BRACE-DEPTH SCAN, not a split.
    //
    // The first version split on `{` and took everything up to the first `}`. That
    // TRUNCATES long rows, and it produced a false finding within a minute of being
    // written: `ipg11-coverage.json:vibe` was reported as having "no disposition
    // FIELD" while the row plainly carries `disposition: "N/A-NOT-OURS"` — as its
    // NINETEENTH key, past the truncation point.
    //
    // A gate whose parser silently drops trailing fields manufactures exactly the
    // defect class it exists to detect. Caught by checking the data with a second
    // reader instead of believing the gate.
    let Some(start) = text.find("\"rows\"") else { return Vec::new() };
    let body = &text[start..];
    let mut out = Vec::new();
    let mut depth = 0usize;
    let mut cur = String::new();
    let mut in_str = false;
    let mut esc = false;
    for ch in body.chars() {
        if in_str {
            cur.push(ch);
            if esc { esc = false; }
            else if ch == '\\' { esc = true; }
            else if ch == '"' { in_str = false; }
            continue;
        }
        match ch {
            '"' => { in_str = true; cur.push(ch); }
            '{' => { depth += 1; if depth == 1 { cur.clear(); } else { cur.push(ch); } }
            '}' => {
                if depth == 1 {
                    if cur.contains("\"surface\"") { out.push(cur.clone()); }
                    cur.clear();
                } else if depth > 1 {
                    cur.push(ch);
                }
                depth = depth.saturating_sub(1);
            }
            _ => { if depth >= 1 { cur.push(ch); } }
        }
    }
    out
}

fn field(row: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\"");
    let i = row.find(&pat)? + pat.len();
    let rest = row[i..].trim_start().strip_prefix(':')?.trim_start();
    if let Some(r) = rest.strip_prefix('"') {
        Some(r.split('"').next().unwrap_or("").to_owned())
    } else {
        Some(
            rest.split(|c: char| c == ',' || c == '\n')
                .next()
                .unwrap_or("")
                .trim()
                .to_owned(),
        )
    }
}

fn coverage_files(root: &Path) -> Vec<PathBuf> {
    let dir = root.join("docs/plan");
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let p = e.path();
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with("ipg") && name.ends_with("-coverage.json") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

#[test]
fn every_coverage_row_states_a_disposition() {
    let root = repo_root();
    let files = coverage_files(&root);
    assert!(
        !files.is_empty(),
        "ANTI-VACUITY: no ipg*-coverage.json found under docs/plan. Either the census \
         artifacts were never made durable — the /tmp defect this gate exists beside — \
         or this scan is broken. Both are findings; neither is a pass."
    );
    let mut unstated = Vec::new();
    let mut checked = 0usize;
    for f in &files {
        let text = std::fs::read_to_string(f).unwrap_or_default();
        for row in rows_of(&text) {
            checked += 1;
            let surface = field(&row, "surface").unwrap_or_else(|| "?".into());
            match field(&row, "disposition") {
                None => unstated.push(format!(
                    "{}:{surface} — no disposition FIELD",
                    f.file_name().and_then(|n| n.to_str()).unwrap_or("?")
                )),
                Some(v) if v == "null" || v.is_empty() => unstated.push(format!(
                    "{}:{surface} — disposition is null; null means BOTH \"undecided\" \
                     and \"not applicable\", so a reader cannot tell whether a human \
                     still owes a decision",
                    f.file_name().and_then(|n| n.to_str()).unwrap_or("?")
                )),
                Some(_) => {}
            }
        }
    }
    assert!(
        checked > 0,
        "ANTI-VACUITY: coverage files exist but zero rows parsed — the row extractor \
         is broken, which reports identically to a clean census"
    );
    assert!(
        unstated.is_empty(),
        "{} coverage row(s) do not state a disposition:\n{:#?}\n\n\
         Use N/A-NOT-OURS when classification is (a), or UNDECIDED when a human still \
         owes the call. Never null — it collapses those two into one.",
        unstated.len(),
        unstated
    );
}
