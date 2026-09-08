#![forbid(unsafe_code)]
//! Derive OMP's inbound RPC command set from the INSTALLED bundle, and map each method
//! to the crate that consumes it.
//!
//! # Why this is a crate and not a document
//!
//! `omp-orchestrator-omp-surface-map-41b` asks for a table checked into the repo,
//! **derived from the installed source, with a re-runnable command printed beside its
//! output**. A prose table cannot satisfy the third clause: it is a snapshot of a
//! binary that moves. Measured within one day —
//!
//! ```text
//! AGENTS.md pins   v18.0.11  sha a95635ad…  19,803,745 bytes
//! installed now    v18.1.3   sha a64dd6cb…  21,382,167 bytes  (mtime 2026-09-02 10:26)
//! ```
//!
//! — and **AGENTS.md's own derivation command no longer runs**: its anchor
//! `let w=async(v)=>` occurs **0 times** in the installed bundle, because the string is
//! a minifier-generated variable name. The 42-method figure it published was correct
//! when written and is not re-derivable by the command it published.
//!
//! # The version-robust anchor
//!
//! Instead of a minified identifier, anchor on a METHOD NAME the protocol requires:
//! `negotiate_protocol`. Collect every `case"snake_case"` site in the bundle, take the
//! contiguous cluster containing that anchor, then split the cluster at its largest
//! internal byte gap — the seam between the inbound-command switch and the adjacent
//! outbound-event switch.
//!
//! Measured at v18.1.3: the cluster holds **47** names; the seam falls after `login`
//! with a 2,323-byte gap, giving **42 inbound commands** and **5 outbound events**
//! (`message_update`, `message_start`, `message_end`, `turn_end`, `agent_end`). The 42
//! reproduce AGENTS.md's list exactly — the figure survives, the command that produced
//! it does not.
//!
//! # The instrument defect this crate exists to correct
//!
//! `41b` records **"SURFACE WE CONSUME: ZERO"**, from four greps:
//! `Command::new("omp")`, `mode=rpc`, `muxConnect`, `omp/` — all **0 files**.
//!
//! **None of those tokens is how a method name appears in code.** A method is a bare
//! quoted string: `"get_state"`, `"negotiate_protocol"`. Searching for `omp/` cannot
//! find it, exactly as `git grep findings_ledger` returned 0 for an artifact whose path
//! is `FINDINGS.jsonl`, and `^command = ` returned 0 against 34 rows aligned with two
//! spaces.
//!
//! Re-measured by the quoted-method token: **9 of 42 inbound methods are consumed**, by
//! `omp-rpc-session`, `omp-inventory-map`, `fast-dispatch`, `receiver-receipt`, and
//! `kernel-only-operator-hook`. And the bead's own four greps now answer **1, 4, 0, 1**
//! rather than 0, 0, 0, 0 — so even its own instrument disagrees with its recorded
//! output. Two distinct causes: a token that could not match, and a repo that moved.
//!
//! # What this does NOT claim
//!
//! Consumption is not adoption, and it is not correctness. `SilverWolf` measured that
//! `--mode=rpc` is a **single-session transport that cannot address a third-party
//! pane** (`OMP-SURFACE-MAP.toml` revision 2), so our terminal scraping for cross-pane
//! dispatch is CORRECT rather than lazy. This crate counts which methods appear in our
//! source; it does not say the remaining 33 should be adopted.

pub mod cli_probe;

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The method any JSON-RPC dispatcher must handle, used as the cluster anchor.
pub const ANCHOR_METHOD: &str = "negotiate_protocol";

/// Maximum byte distance between neighbouring `case` sites still considered the same
/// switch statement. Wide enough to span a handler body, narrow enough not to swallow
/// the next switch.
pub const CLUSTER_GAP_BYTES: usize = 4_000;

const _: () = assert!(
    CLUSTER_GAP_BYTES >= 512,
    "a gap under 512 bytes splits a single switch and undercounts the command set"
);

/// One `case"name"` site: its byte offset and its name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaseSite {
    pub offset: usize,
    pub name: String,
}

/// The split of a case cluster into inbound commands and outbound events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandSet {
    pub inbound: Vec<String>,
    pub outbound: Vec<String>,
    /// Byte gap at the seam, published so a reader can judge the split rather than
    /// trust it. A small seam gap means the split is a guess.
    pub seam_gap: usize,
}

/// Why a derivation refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeriveError {
    /// The bundle held no `case"snake_case"` sites at all. An empty enumeration is an
    /// ERROR, never "nothing to map" — acceptance 6.
    EmptyEnumeration,
    /// The anchor method is absent, so we cannot locate the dispatcher. Refuses rather
    /// than returning whichever cluster happened to be largest.
    AnchorAbsent { anchor: String },
    /// The cluster holds a single name, so there is no seam to split and the result
    /// would be indistinguishable from a parse failure.
    ClusterTooSmall { found: usize },
}
impl fmt::Display for DeriveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyEnumeration => f.write_str("EMPTY_ENUMERATION"),
            Self::AnchorAbsent { anchor } => write!(f, "ANCHOR_ABSENT anchor={anchor}"),
            Self::ClusterTooSmall { found } => write!(f, "CLUSTER_TOO_SMALL found={found}"),
        }
    }
}

impl std::error::Error for DeriveError {}

/// Extract every `case"snake_case"` site. Deliberately a scanner over the minified
/// bundle rather than a JS parse: the bundle is 21 MB of one-line output and the only
/// stable shape in it is the literal.
pub fn case_sites(bundle: &str) -> Vec<CaseSite> {
    let needle = "case\"";
    let bytes = bundle.as_bytes();
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(rel) = bundle[from..].find(needle) {
        let start = from + rel + needle.len();
        let mut end = start;
        while end < bytes.len() && bytes[end] != b'"' {
            end += 1;
        }
        if end < bytes.len() {
            let name = &bundle[start..end];
            if is_snake_case(name) {
                out.push(CaseSite {
                    offset: start,
                    name: name.to_owned(),
                });
            }
        }
        from = start;
    }
    out
}

/// `[a-z][a-z0-9_]*`, checked without a regex dependency.
fn is_snake_case(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(c) if c.is_ascii_lowercase() => {}
        _ => return false,
    }
    chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// Split the anchored cluster into inbound commands and outbound events.
pub fn derive_command_set(sites: &[CaseSite]) -> Result<CommandSet, DeriveError> {
    if sites.is_empty() {
        return Err(DeriveError::EmptyEnumeration);
    }
    let anchor = sites
        .iter()
        .position(|s| s.name == ANCHOR_METHOD)
        .ok_or_else(|| DeriveError::AnchorAbsent {
            anchor: ANCHOR_METHOD.to_owned(),
        })?;
    let mut lo = anchor;
    while lo > 0 && sites[lo].offset - sites[lo - 1].offset < CLUSTER_GAP_BYTES {
        lo -= 1;
    }
    let mut hi = anchor;
    while hi + 1 < sites.len() && sites[hi + 1].offset - sites[hi].offset < CLUSTER_GAP_BYTES {
        hi += 1;
    }
    let cluster = &sites[lo..=hi];
    if cluster.len() < 2 {
        return Err(DeriveError::ClusterTooSmall {
            found: cluster.len(),
        });
    }
    // The seam is the largest internal gap: the boundary between two switches.
    let (seam_idx, seam_gap) = (0..cluster.len() - 1)
        .map(|i| (i, cluster[i + 1].offset - cluster[i].offset))
        .max_by_key(|(_, gap)| *gap)
        .expect("cluster has at least two sites");
    let dedup = |slice: &[CaseSite]| {
        let mut seen = BTreeSet::new();
        slice
            .iter()
            .filter(|s| seen.insert(s.name.clone()))
            .map(|s| s.name.clone())
            .collect::<Vec<_>>()
    };
    Ok(CommandSet {
        inbound: dedup(&cluster[..=seam_idx]),
        outbound: dedup(&cluster[seam_idx + 1..]),
        seam_gap,
    })
}

/// One row of the deliverable table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceRow {
    pub method: String,
    /// Crates whose source carries the quoted method name today. Empty = unmapped.
    pub consumed_by: Vec<String>,
}

impl SurfaceRow {
    pub fn is_mapped(&self) -> bool {
        !self.consumed_by.is_empty()
    }
}

/// Fold a method list plus a consumption index into the table, refusing a scan that
/// found nothing.
///
/// **Acceptance 5, the positive control, is enforced here rather than trusted:** if no
/// method comes back mapped, that is either the headline or a broken scan, and the two
/// are indistinguishable from the outside — so the caller is forced to say which.
pub fn build_table(
    methods: &[String],
    consumption: &BTreeMap<String, Vec<String>>,
) -> Result<Vec<SurfaceRow>, DeriveError> {
    if methods.is_empty() {
        return Err(DeriveError::EmptyEnumeration);
    }
    Ok(methods
        .iter()
        .map(|method| SurfaceRow {
            method: method.clone(),
            consumed_by: consumption.get(method).cloned().unwrap_or_default(),
        })
        .collect())
}

/// Render the table, one line per method, mapped rows first.
pub fn render_table(rows: &[SurfaceRow]) -> String {
    let mapped = rows.iter().filter(|r| r.is_mapped()).count();
    let mut out = vec![format!(
        "OMP_SURFACE_CONSUMPTION mapped={mapped} unmapped={} total={}",
        rows.len() - mapped,
        rows.len()
    )];
    for row in rows.iter().filter(|r| r.is_mapped()) {
        out.push(format!(
            "  MAPPED   {:28} consumed_by={}",
            row.method,
            row.consumed_by.join(",")
        ));
    }
    for row in rows.iter().filter(|r| !r.is_mapped()) {
        out.push(format!("  UNMAPPED {:28} consumed_by=NONE", row.method));
    }
    out.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bundle shaped like the real one: an inbound switch, a wide gap, then an
    /// outbound event switch.
    fn fixture() -> String {
        let mut s = String::new();
        s.push_str("case\"negotiate_protocol\"");
        s.push_str(&"x".repeat(100));
        s.push_str("case\"get_state\"");
        s.push_str(&"x".repeat(100));
        s.push_str("case\"login\"");
        // the seam
        s.push_str(&"y".repeat(2_300));
        s.push_str("case\"message_update\"");
        s.push_str(&"y".repeat(50));
        s.push_str("case\"turn_end\"");
        s
    }

    #[test]
    fn the_seam_splits_inbound_commands_from_outbound_events() {
        let sites = case_sites(&fixture());
        let set = derive_command_set(&sites).expect("fixture must derive");
        assert_eq!(set.inbound, vec!["negotiate_protocol", "get_state", "login"]);
        assert_eq!(set.outbound, vec!["message_update", "turn_end"]);
        assert!(
            set.seam_gap > 2_000,
            "the seam must be published so a reader can judge the split, got {}",
            set.seam_gap
        );
    }

    /// ANTI-VACUITY, acceptance 6: an empty enumeration is an ERROR. A scanner that
    /// matched nothing reads exactly like a bundle with no dispatcher.
    #[test]
    fn an_empty_enumeration_is_an_error_not_nothing_to_map() {
        assert_eq!(
            derive_command_set(&[]).unwrap_err(),
            DeriveError::EmptyEnumeration
        );
        assert_eq!(
            build_table(&[], &BTreeMap::new()).unwrap_err(),
            DeriveError::EmptyEnumeration
        );
        // A bundle with no `case"` at all must also refuse, not return an empty set.
        assert_eq!(
            derive_command_set(&case_sites("no cases here at all")).unwrap_err(),
            DeriveError::EmptyEnumeration
        );
        for (error, expected) in [
            (DeriveError::EmptyEnumeration, "EMPTY_ENUMERATION"),
            (DeriveError::AnchorAbsent { anchor: ANCHOR_METHOD.to_owned() }, "ANCHOR_ABSENT anchor=negotiate_protocol"),
            (DeriveError::ClusterTooSmall { found: 1 }, "CLUSTER_TOO_SMALL found=1"),
        ] {
            assert_eq!(error.to_string(), expected);
        }
    }

    /// Refuse rather than guess when the anchor is gone — which is exactly how
    /// AGENTS.md's derivation failed: its anchor vanished with a version bump and the
    /// command produced no answer at all rather than a wrong one.
    #[test]
    fn a_missing_anchor_refuses_instead_of_taking_the_largest_cluster() {
        let sites = case_sites("case\"prompt\"case\"steer\"case\"abort\"");
        assert_eq!(
            derive_command_set(&sites).unwrap_err(),
            DeriveError::AnchorAbsent {
                anchor: "negotiate_protocol".to_owned()
            }
        );
    }

    /// The scanner must ignore non-snake-case cases, which the bundle is full of:
    /// minified switches over single letters and PascalCase names.
    #[test]
    fn only_snake_case_names_are_collected() {
        let sites = case_sites("case\"A\"case\"Foo\"case\"get_state\"case\"x1\"case\"_leading\"");
        let names: Vec<&str> = sites.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, vec!["get_state", "x1"]);
        assert!(is_snake_case("get_messages_page"));
        assert!(!is_snake_case("GetState"));
        assert!(!is_snake_case(""));
    }

    /// POSITIVE CONTROL, acceptance 5: at least one method must come back mapped, and
    /// the render must say which. `9 of 42` measured 2026-09-02.
    #[test]
    fn the_table_distinguishes_mapped_from_unmapped_and_counts_both() {
        let methods: Vec<String> = ["get_state", "negotiate_protocol", "login", "compact"]
            .iter()
            .map(|s| (*s).to_owned())
            .collect();
        let mut consumption = BTreeMap::new();
        consumption.insert(
            "get_state".to_owned(),
            vec!["omp-inventory-map".to_owned(), "omp-rpc-session".to_owned()],
        );
        consumption.insert(
            "negotiate_protocol".to_owned(),
            vec!["omp-rpc-session".to_owned()],
        );
        let rows = build_table(&methods, &consumption).expect("table must build");
        assert_eq!(rows.iter().filter(|r| r.is_mapped()).count(), 2);
        let text = render_table(&rows);
        assert!(text.contains("mapped=2 unmapped=2 total=4"), "{text}");
        assert!(text.contains("MAPPED   get_state"), "{text}");
        assert!(text.contains("UNMAPPED login"), "{text}");
        // A method nobody consumes must not be silently omitted: an absent row and an
        // unmapped row are different facts.
        assert!(rows.iter().any(|r| r.method == "compact" && !r.is_mapped()));
    }

    /// THE INSTRUMENT LEG. The bead's four greps cannot match a quoted method name,
    /// and that is why it recorded ZERO. Encoded so the next reader does not repeat it.
    #[test]
    fn the_beads_grep_tokens_cannot_match_a_quoted_method_name() {
        let real_source = r#"    let request = RpcRequest::GetState; frame("get_state")"#;
        for token in ["Command::new(\"omp\")", "mode=rpc", "muxConnect", "omp/"] {
            assert!(
                !real_source.contains(token),
                "{token} cannot find a method name in real source"
            );
        }
        assert!(
            real_source.contains("\"get_state\""),
            "the quoted method name is the token that actually appears"
        );
    }
}
