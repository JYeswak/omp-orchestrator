#![no_main]
//! INVARIANTS for the close-reason lattice (beads mcq2, qt9r, igwd — claim-status-lattice-non-upgrading):
//!
//! 1. `classify_close_reason` never panics on any string (reasons are agent-typed prose).
//! 2. `Unread` (the observer did not look) is NEVER `is_verified`; `Empty` is never verified.
//! 3. A `Verified { prefix }` verdict is returned ONLY when the trimmed reason literally starts
//!    with `prefix.as_str()` — and with exactly the FIRST matching prefix in `ClosePrefix::ALL`
//!    (the lattice cannot be upgraded by a later, stronger token appearing in the prose).
//! 4. `PolicyRefused { leading }` carries the first whitespace-delimited token verbatim.
//! 5. TOKEN BOUNDARY: a prefix followed immediately by an alphanumeric (`DONEZO`, `APPROVEDx`,
//!    `WONTFIXED`) is NOT that prefix. This is the contract the policy MEANS; if the fuzzer
//!    finds the kernel accepting such a reason, that is a finding to file, not a harness bug.
//! 6. Classification is idempotent under re-classifying the verdict's own `Display` — a verified
//!    verdict's rendering starts with the same prefix.

use ack_spine::close_reason::{classify_close_reason, ClosePrefix, CloseReasonVerdict};
use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;

const MAX_LEN: usize = 4096;

#[derive(Arbitrary, Debug)]
enum Input {
    Unread,
    Raw(Vec<u8>),
    /// Structure-aware: a sanctioned prefix, an optional glued suffix, then prose.
    Structured { prefix: u8, glue: Option<u8>, sep: u8, prose: Vec<u8>, leading_ws: u8 },
}

fn build(input: &Input) -> Option<String> {
    match input {
        Input::Unread => None,
        Input::Raw(b) => Some(String::from_utf8_lossy(b).chars().take(MAX_LEN).collect()),
        Input::Structured { prefix, glue, sep, prose, leading_ws } => {
            let p = ClosePrefix::ALL[usize::from(*prefix) % ClosePrefix::ALL.len()].as_str();
            let mut s = " ".repeat(usize::from(*leading_ws % 4));
            s.push_str(p);
            if let Some(g) = glue {
                // glued alphanumeric or punctuation — the boundary question
                s.push((b'0' + (g % 75)) as char);
            }
            s.push([' ', ':', '\n', '-', '\t', '.', ',', ';'][usize::from(*sep % 8)]);
            s.push_str(&String::from_utf8_lossy(prose).chars().take(200).collect::<String>());
            Some(s)
        }
    }
}

fn model(reason: Option<&str>) -> CloseReasonVerdict {
    let Some(raw) = reason else { return CloseReasonVerdict::Unread };
    let trimmed = raw.trim_start();
    if trimmed.trim().is_empty() {
        return CloseReasonVerdict::Empty;
    }
    for prefix in ClosePrefix::ALL {
        let tok = prefix.as_str();
        if let Some(rest) = trimmed.strip_prefix(tok) {
            let boundary_ok = rest.chars().next().map_or(true, |c| !c.is_alphanumeric());
            if boundary_ok {
                return CloseReasonVerdict::Verified { prefix: *prefix };
            }
        }
    }
    CloseReasonVerdict::PolicyRefused {
        leading: trimmed.split_whitespace().next().unwrap_or_default().to_owned(),
    }
}

fuzz_target!(|input: Input| {
    let reason = build(&input);
    // Contract 1: never panics.
    let verdict = classify_close_reason(reason.as_deref());
    let _ = verdict.label();
    let rendered = verdict.to_string();

    // Contract 2: absence of a reading is never a verification.
    if reason.is_none() {
        assert_eq!(verdict, CloseReasonVerdict::Unread);
    }
    assert!(!(matches!(verdict, CloseReasonVerdict::Unread | CloseReasonVerdict::Empty) && verdict.is_verified()), "Unread/Empty reported verified");

    // Contract 3: a verified prefix is literally at the start, and is the first in ALL order.
    if let CloseReasonVerdict::Verified { prefix } = &verdict {
        let trimmed = reason.as_deref().unwrap_or("").trim_start();
        assert!(trimmed.starts_with(prefix.as_str()), "verified {prefix} but reason does not start with it: {trimmed:?}");
        for earlier in ClosePrefix::ALL.iter().take_while(|p| *p != prefix) {
            assert!(!trimmed.starts_with(earlier.as_str()), "lattice upgraded past an earlier-matching prefix {earlier}");
        }
        // Contract 6: the rendering re-classifies to the same prefix.
        let again = classify_close_reason(Some(rendered.trim_start_matches("CLOSE_REASON_VERIFIED prefix=")));
        assert_eq!(again, verdict, "Display round-trip changed the verdict");
    }

    // Contract 4: the refused token is the first whitespace token, verbatim.
    if let CloseReasonVerdict::PolicyRefused { leading } = &verdict {
        let trimmed = reason.as_deref().unwrap_or("").trim_start();
        assert_eq!(Some(leading.as_str()), trimmed.split_whitespace().next(), "leading token not verbatim");
        assert!(!leading.is_empty(), "refused with an empty leading token on a non-empty reason");
    }

    // Contract 5: TOKEN BOUNDARY. The model requires it; a divergence here is a FINDING about
    // the kernel (a prefix-shaped word passing as a sanctioned close), and the assertion names it.
    assert_eq!(verdict, model(reason.as_deref()), "TOKEN-BOUNDARY: classify_close_reason accepted or refused differently from the boundary model for {reason:?}");
});
