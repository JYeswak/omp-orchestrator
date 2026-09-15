#![forbid(unsafe_code)]

//! Proptest floor for the close-prefix lattice.
//! Restates `fuzz/fuzz_targets/ack_spine_close_reason_lattice.rs`.

use ack_spine::close_reason::{classify_close_reason, ClosePrefix, CloseReasonVerdict};
use proptest::prelude::*;

fn model(reason: Option<&str>) -> CloseReasonVerdict {
    let Some(raw) = reason else {
        return CloseReasonVerdict::Unread;
    };
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
        leading: trimmed
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_owned(),
    }
}

proptest! {
    #[test]
    fn close_prefix_lattice_and_token_boundary(
        unread in proptest::bool::ANY,
        prefix_idx in 0usize..ClosePrefix::ALL.len(),
        glue in proptest::option::of(0u8..75),
        sep in 0u8..8,
        prose in "[A-Za-z0-9 .:_-]{0,40}",
        leading_ws in 0u8..4,
        raw in proptest::option::of("[A-Za-z0-9 .:_-]{0,64}"),
        structured in proptest::bool::ANY,
    ) {
        let reason = if unread {
            None
        } else if structured {
            let p = ClosePrefix::ALL[prefix_idx].as_str();
            let mut s = " ".repeat(usize::from(leading_ws));
            s.push_str(p);
            if let Some(g) = glue {
                s.push((b'0' + (g % 75)) as char);
            }
            s.push([' ', ':', '\n', '-', '\t', '.', ',', ';'][usize::from(sep % 8)]);
            s.push_str(&prose);
            Some(s)
        } else {
            raw
        };
        let verdict = classify_close_reason(reason.as_deref());
        let _ = verdict.label();
        let rendered = verdict.to_string();
        if reason.is_none() {
            prop_assert_eq!(verdict.clone(), CloseReasonVerdict::Unread);
        }
        prop_assert!(
            !(matches!(
                verdict,
                CloseReasonVerdict::Unread | CloseReasonVerdict::Empty
            ) && verdict.is_verified())
        );
        if let CloseReasonVerdict::Verified { prefix } = &verdict {
            let trimmed = reason.as_deref().unwrap_or("").trim_start();
            prop_assert!(trimmed.starts_with(prefix.as_str()));
            for earlier in ClosePrefix::ALL.iter().take_while(|p| *p != prefix) {
                prop_assert!(!trimmed.starts_with(earlier.as_str()));
            }
            let again = classify_close_reason(Some(
                rendered.trim_start_matches("CLOSE_REASON_VERIFIED prefix="),
            ));
            prop_assert_eq!(again, verdict.clone());
        }
        if let CloseReasonVerdict::PolicyRefused { leading } = &verdict {
            let trimmed = reason.as_deref().unwrap_or("").trim_start();
            prop_assert_eq!(Some(leading.as_str()), trimmed.split_whitespace().next());
            prop_assert!(!leading.is_empty());
        }
        prop_assert_eq!(verdict, model(reason.as_deref()));
    }
}

#[test]
fn planted_glued_prefix_is_not_verified() {
    let verdict = classify_close_reason(Some("DONEZO: glued"));
    assert!(!verdict.is_verified(), "{verdict:?}");
}
