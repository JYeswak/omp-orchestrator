//! Surface alignment — the repeatable process that makes cell N+1 mechanical.
//!
//! Bead: omp-orchestrator-ablcf
//!
//! ## What this adds, and what it deliberately does not
//!
//! This crate already DERIVES the OMP surface from the installed artifact through direct
//! process probes, fail-closed, with `UNKNOWN` never upgraded to a healthy result (see the
//! crate docs). **That half is not rewritten here.** What did not exist is the other half:
//! deciding, for each derived surface entry, whether this workspace *consumes* it — and
//! **refusing** when nobody has said either way.
//!
//! ## Three verdicts, and the third is the point
//!
//! ```text
//! CONSUMED          a named crate reaches it, with a cited call site
//! DELIBERATELY_NOT  we do not consume it, WITH an owner, a reason, and a dies_when
//! UNCLASSIFIED      neither -> THE GATE FAILS
//! ```
//!
//! Per `fh C69` these are three verdicts with three different remedies and must never be
//! merged. The classifier this replaces had three arms too, but they were
//! `owner.is_some()` / `name in a hand-typed list` / **everything else**, and that last one
//! emitted `CAPABILITY_NOT_USED` — a positive-sounding claim produced by the *default
//! branch*. That is the defect `AGENTS.md` records in our own
//! `kernel-only-operator-hook`, where a session that never ran classified as `COVERED`
//! because compliance was the fallthrough. **Here the residual is the failing state**, and
//! every arm asserts its own precondition.
//!
//! ## Consumption is DECLARED BY THE CONSUMER, then derived from metadata
//!
//! The previous `owner_for` could name exactly one owner — this crate itself — so a real
//! consumer elsewhere in the workspace was invisible to it. Rather than maintain a central
//! list that must be edited whenever a consumer appears, **each consuming crate declares
//! what it consumes in its own manifest**:
//!
//! ```toml
//! [package.metadata.omp_surface]
//! consumes = [
//!   { kind = "rpc_handler", name = "get_state", call_site = "src/omp_state.rs" },
//! ]
//! ```
//!
//! That is the same architecture `[package.metadata.gate]` already uses in this repo, for
//! the same stated reason: *"this crate's OWN check invocation, so `gate-runner` needs no
//! edit when a gate is added."* The map needs no edit when a consumer is added, and the
//! declaration is read from `cargo metadata` — derived, not transcribed.
//!
//! ## No typed counts
//!
//! There are no expected-count constants here, on purpose. Measured 2026-09-08: the RPC
//! surface figure has been published as `14`, as `3`, and as `39` on a different axis,
//! while one handshake advertised `590` commands — and `--mode=rpc` turns out to read
//! `{id,type}` frames rather than JSON-RPC, so those were not even the same channel.
//! **Any number typed into a source file is a transcribed value that reports the surface as
//! it was when someone typed it.** Ratchets here are per-surface or ratios, and every
//! report carries the install path and version it measured.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The manifest key a consuming crate uses to declare what it reaches.
pub const CONSUMER_METADATA_KEY: &str = "omp_surface";

/// A surface entry we do not consume, with the reason recorded.
///
/// **The list below is empty by design.** Following the `UNWIRED_LANE_ALLOWANCE` shape this
/// repo already uses, an exception is a NAMED ROW carrying an owner and a death condition,
/// never silence. A row whose `dies_when` has come true is a row to delete, and
/// [`AlignmentReport::stale_allowances`] names those rather than letting them accumulate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliberateNonConsumption {
    pub kind: &'static str,
    pub name: &'static str,
    /// Who decided. Not a crate — a person or pane accountable for the decision.
    pub owner: &'static str,
    /// Why not consuming it is correct, not merely current.
    pub reason: &'static str,
    /// The observable condition under which this row must be deleted.
    pub dies_when: &'static str,
}

/// Deliberate non-consumption rows.
///
/// EMPTY BY DESIGN. Adding a row is a decision that must survive review; leaving a surface
/// out of both this list and a consumer declaration makes the gate fail, which is the
/// intended pressure.
pub const DELIBERATELY_NOT: &[DeliberateNonConsumption] = &[];

/// One consumer declaration, read from a crate's manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumerDeclaration {
    pub crate_name: String,
    pub kind: String,
    pub name: String,
    /// Path within the declaring crate. Cited so a reader can check the claim.
    pub call_site: String,
}

/// Why a declaration was rejected. A malformed declaration is NOT a missing one: silently
/// dropping it would turn a typo into an invisible unclassified surface.
// Serialize only: `field` is a `&'static str` because it names a compile-time constant, and
// deriving `Deserialize` on a borrowed-static type would force `'de: 'static`. These
// defects are EMITTED for diagnosis and never read back, so the asymmetry is honest rather
// than a workaround -- turning the field into a `String` to satisfy a trait nobody uses
// would be the tail wagging the dog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE", tag = "kind")]
pub enum DeclarationDefect {
    NotAnArray { crate_name: String },
    NotAnObject { crate_name: String, index: usize },
    MissingField { crate_name: String, index: usize, field: &'static str },
    EmptyField { crate_name: String, index: usize, field: &'static str },
}

impl std::fmt::Display for DeclarationDefect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAnArray { crate_name } => write!(
                formatter,
                "ALIGN_DECL_NOT_AN_ARRAY crate={crate_name} detail=\
                 metadata.{CONSUMER_METADATA_KEY}.consumes must be an array"
            ),
            Self::NotAnObject { crate_name, index } => write!(
                formatter,
                "ALIGN_DECL_NOT_AN_OBJECT crate={crate_name} index={index}"
            ),
            Self::MissingField { crate_name, index, field } => write!(
                formatter,
                "ALIGN_DECL_MISSING_FIELD crate={crate_name} index={index} field={field}"
            ),
            Self::EmptyField { crate_name, index, field } => write!(
                formatter,
                "ALIGN_DECL_EMPTY_FIELD crate={crate_name} index={index} field={field} \
                 detail=an empty value is not a declaration"
            ),
        }
    }
}

/// Every consumer declaration in the workspace, plus every defect found reading them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ConsumerIndex {
    pub declarations: Vec<ConsumerDeclaration>,
    pub defects: Vec<DeclarationDefect>,
    /// Packages examined. A ZERO here is what separates "nobody declares anything" from
    /// "we failed to read the metadata", and [`align`] refuses on the second.
    pub packages_scanned: usize,
}

impl ConsumerIndex {
    #[must_use]
    pub fn find(&self, kind: &str, name: &str) -> Option<&ConsumerDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.kind == kind && declaration.name == name)
    }
}

/// The verdict for one derived surface entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE", tag = "verdict")]
pub enum Alignment {
    Consumed { crate_name: String, call_site: String },
    DeliberatelyNot { owner: String, reason: String, dies_when: String },
    /// The residual, and the failing state. Never produced by a default branch: it is
    /// returned only after both positive arms have been tested and rejected.
    Unclassified,
}

impl Alignment {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Consumed { .. } => "CONSUMED",
            Self::DeliberatelyNot { .. } => "DELIBERATELY_NOT",
            Self::Unclassified => "UNCLASSIFIED",
        }
    }

    #[must_use]
    pub const fn is_classified(&self) -> bool {
        !matches!(self, Self::Unclassified)
    }
}

/// A derived surface entry awaiting classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceEntry {
    pub kind: String,
    pub name: String,
}

/// One classified row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignmentRow {
    pub kind: String,
    pub name: String,
    pub alignment: Alignment,
}

/// Typed refusals. Each names a state with its own remedy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlignmentError {
    /// ANTI-VACUITY. An empty derived surface set is an ERROR: it reports identically to a
    /// fully-classified one, and the likeliest cause is that the installed artifact could
    /// not be read — which is UNMEASURED, never green.
    EmptySurfaceSet,
    /// The metadata itself could not be read. Distinct from "nobody declared anything".
    NoPackagesScanned,
    /// A declaration was present and malformed. Refused rather than dropped.
    MalformedDeclarations(Vec<DeclarationDefect>),
    /// The point of the gate.
    Unclassified(Vec<SurfaceEntry>),
}

impl std::fmt::Display for AlignmentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySurfaceSet => write!(
                formatter,
                "ALIGN_EMPTY_SURFACE_SET detail=zero derived surface entries; a scan that \
                 covered nothing reports identically to one that passed, so this is \
                 UNMEASURED and never a pass"
            ),
            Self::NoPackagesScanned => write!(
                formatter,
                "ALIGN_NO_PACKAGES_SCANNED detail=cargo metadata yielded no packages; \
                 'nobody declares consumption' and 'the metadata was unreadable' are \
                 different states and this is the second"
            ),
            Self::MalformedDeclarations(defects) => {
                writeln!(
                    formatter,
                    "ALIGN_MALFORMED_DECLARATIONS count={} detail=a malformed declaration \
                     is refused, not dropped: dropping it would turn a typo into an \
                     invisible unclassified surface",
                    defects.len()
                )?;
                for defect in defects {
                    writeln!(formatter, "  {defect}")?;
                }
                Ok(())
            }
            Self::Unclassified(entries) => {
                writeln!(
                    formatter,
                    "ALIGN_UNCLASSIFIED count={} detail=every derived OMP surface must be \
                     CONSUMED by a named crate or DELIBERATELY_NOT with an owner, a reason \
                     and a dies_when. Silence is not an exception.",
                    entries.len()
                )?;
                for entry in entries {
                    writeln!(
                        formatter,
                        "  ALIGN_UNCLASSIFIED_SURFACE kind={} name={}",
                        entry.kind, entry.name
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl AlignmentError {
    /// `2` for usage/declaration refusals, `3` for an instrument failure. **Never `4`** —
    /// `4` is reserved for upstream-unreachable, which `adapter_exec.rs:138-142` already
    /// spends that way, and a shared vocabulary is what lets an agent branch on `$?`.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::MalformedDeclarations(_) | Self::Unclassified(_) => 2,
            Self::EmptySurfaceSet | Self::NoPackagesScanned => 3,
        }
    }
}

/// The full alignment result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignmentReport {
    /// Which OMP was measured. Carried so no reader has to guess, and so a stale report is
    /// visibly stale rather than silently wrong.
    pub measured_install: String,
    pub measured_version: String,
    pub rows: Vec<AlignmentRow>,
    pub packages_scanned: usize,
    /// Per-surface-kind coverage as a RATIO, never a workspace-wide absolute. A ratchet on
    /// an absolute count is red-by-construction the next time the subject legitimately
    /// grows, and a gate that is red by construction gets routed around.
    pub coverage_by_kind: BTreeMap<String, KindCoverage>,
    /// Allowance rows whose `dies_when` condition is now observably true.
    pub stale_allowances: Vec<String>,
}

/// Coverage for one surface kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindCoverage {
    pub total: usize,
    pub consumed: usize,
    pub deliberately_not: usize,
    pub unclassified: usize,
}

impl KindCoverage {
    /// Classified fraction in basis points, so the ratio needs no float comparison.
    #[must_use]
    pub const fn classified_bps(&self) -> u32 {
        if self.total == 0 {
            return 0;
        }
        let classified = self.consumed + self.deliberately_not;
        #[allow(clippy::cast_possible_truncation)]
        {
            ((classified * 10_000) / self.total) as u32
        }
    }
}

/// Classify ONE entry. Every arm asserts its own precondition; the residual is the failure.
#[must_use]
pub fn classify(entry: &SurfaceEntry, consumers: &ConsumerIndex) -> Alignment {
    if let Some(declaration) = consumers.find(&entry.kind, &entry.name) {
        return Alignment::Consumed {
            crate_name: declaration.crate_name.clone(),
            call_site: declaration.call_site.clone(),
        };
    }
    if let Some(row) = DELIBERATELY_NOT
        .iter()
        .find(|row| row.kind == entry.kind && row.name == entry.name)
    {
        return Alignment::DeliberatelyNot {
            owner: row.owner.to_owned(),
            reason: row.reason.to_owned(),
            dies_when: row.dies_when.to_owned(),
        };
    }
    Alignment::Unclassified
}

/// Read every `[package.metadata.omp_surface]` declaration out of `cargo metadata` JSON.
///
/// # Errors
///
/// Never errors on absence — a package with no declaration is simply not a consumer. It
/// records DEFECTS for declarations that exist and are malformed, which the caller refuses
/// on. Returns [`AlignmentError::NoPackagesScanned`] only when the metadata yields no
/// packages at all, because that is an instrument failure rather than an empty answer.
pub fn index_consumers(metadata_json: &str) -> Result<ConsumerIndex, AlignmentError> {
    let root: Value =
        serde_json::from_str(metadata_json).map_err(|_| AlignmentError::NoPackagesScanned)?;
    let packages = root
        .get("packages")
        .and_then(Value::as_array)
        .ok_or(AlignmentError::NoPackagesScanned)?;
    if packages.is_empty() {
        return Err(AlignmentError::NoPackagesScanned);
    }
    let mut index = ConsumerIndex {
        packages_scanned: packages.len(),
        ..ConsumerIndex::default()
    };
    for package in packages {
        let crate_name = package
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("<unnamed>")
            .to_owned();
        let Some(consumes) = package
            .get("metadata")
            .and_then(|metadata| metadata.get(CONSUMER_METADATA_KEY))
            .and_then(|surface| surface.get("consumes"))
        else {
            continue;
        };
        let Some(entries) = consumes.as_array() else {
            index
                .defects
                .push(DeclarationDefect::NotAnArray { crate_name });
            continue;
        };
        for (position, entry) in entries.iter().enumerate() {
            let Some(object) = entry.as_object() else {
                index.defects.push(DeclarationDefect::NotAnObject {
                    crate_name: crate_name.clone(),
                    index: position,
                });
                continue;
            };
            let mut field = |key: &'static str| -> Option<String> {
                match object.get(key).and_then(Value::as_str) {
                    None => {
                        index.defects.push(DeclarationDefect::MissingField {
                            crate_name: crate_name.clone(),
                            index: position,
                            field: key,
                        });
                        None
                    }
                    Some(value) if value.trim().is_empty() => {
                        index.defects.push(DeclarationDefect::EmptyField {
                            crate_name: crate_name.clone(),
                            index: position,
                            field: key,
                        });
                        None
                    }
                    Some(value) => Some(value.to_owned()),
                }
            };
            let kind = field("kind");
            let name = field("name");
            let call_site = field("call_site");
            if let (Some(kind), Some(name), Some(call_site)) = (kind, name, call_site) {
                index.declarations.push(ConsumerDeclaration {
                    crate_name: crate_name.clone(),
                    kind,
                    name,
                    call_site,
                });
            }
        }
    }
    Ok(index)
}

/// Classify every derived surface entry and REFUSE if any is unclassified.
///
/// # Errors
///
/// See [`AlignmentError`]. The ordering is deliberate: instrument failures are reported
/// before content failures, because an unreadable input cannot produce a meaningful
/// unclassified list.
pub fn align(
    measured_install: &str,
    measured_version: &str,
    surface: &[SurfaceEntry],
    consumers: &ConsumerIndex,
) -> Result<AlignmentReport, AlignmentError> {
    if consumers.packages_scanned == 0 {
        return Err(AlignmentError::NoPackagesScanned);
    }
    if surface.is_empty() {
        return Err(AlignmentError::EmptySurfaceSet);
    }
    if !consumers.defects.is_empty() {
        return Err(AlignmentError::MalformedDeclarations(
            consumers.defects.clone(),
        ));
    }

    let mut rows = Vec::with_capacity(surface.len());
    let mut coverage: BTreeMap<String, KindCoverage> = BTreeMap::new();
    let mut unclassified = Vec::new();
    for entry in surface {
        let alignment = classify(entry, consumers);
        let bucket = coverage.entry(entry.kind.clone()).or_insert(KindCoverage {
            total: 0,
            consumed: 0,
            deliberately_not: 0,
            unclassified: 0,
        });
        bucket.total += 1;
        match &alignment {
            Alignment::Consumed { .. } => bucket.consumed += 1,
            Alignment::DeliberatelyNot { .. } => bucket.deliberately_not += 1,
            Alignment::Unclassified => {
                bucket.unclassified += 1;
                unclassified.push(entry.clone());
            }
        }
        rows.push(AlignmentRow {
            kind: entry.kind.clone(),
            name: entry.name.clone(),
            alignment,
        });
    }

    // An allowance row for a surface that no longer exists is stale state, not an
    // exception: report it rather than letting the list accumulate rows nobody can retire.
    let present: BTreeSet<(&str, &str)> = surface
        .iter()
        .map(|entry| (entry.kind.as_str(), entry.name.as_str()))
        .collect();
    let stale_allowances = DELIBERATELY_NOT
        .iter()
        .filter(|row| !present.contains(&(row.kind, row.name)))
        .map(|row| format!("{}:{} owner={} dies_when={}", row.kind, row.name, row.owner, row.dies_when))
        .collect();

    if !unclassified.is_empty() {
        return Err(AlignmentError::Unclassified(unclassified));
    }
    Ok(AlignmentReport {
        measured_install: measured_install.to_owned(),
        measured_version: measured_version.to_owned(),
        rows,
        packages_scanned: consumers.packages_scanned,
        coverage_by_kind: coverage,
        stale_allowances,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(consumes: &str) -> String {
        format!(
            r#"{{"packages":[
                {{"name":"ompo-doctor","metadata":{{"omp_surface":{{"consumes":[{consumes}]}}}}}},
                {{"name":"text-structure"}}
            ]}}"#
        )
    }

    fn entry(kind: &str, name: &str) -> SurfaceEntry {
        SurfaceEntry { kind: kind.to_owned(), name: name.to_owned() }
    }

    /// ACCEPTANCE G. `%20`'s proven cell is the first CONSUMED row, and it is what the old
    /// self-referential `owner_for` could not express: it named only this crate, so a real
    /// consumer elsewhere in the workspace was invisible.
    #[test]
    fn a_declaring_crate_produces_a_consumed_row_with_a_cited_call_site() {
        let json = metadata(
            r#"{"kind":"rpc_handler","name":"get_state","call_site":"src/omp_state.rs"}"#,
        );
        let consumers = index_consumers(&json).expect("index");
        assert_eq!(consumers.declarations.len(), 1);
        let report = align(
            "/opt/omp",
            "omp/18.0.11",
            &[entry("rpc_handler", "get_state")],
            &consumers,
        )
        .expect("fully classified");
        match &report.rows[0].alignment {
            Alignment::Consumed { crate_name, call_site } => {
                assert_eq!(crate_name, "ompo-doctor");
                assert_eq!(call_site, "src/omp_state.rs");
            }
            other => panic!("expected CONSUMED, got {}", other.as_str()),
        }
        assert_eq!(report.coverage_by_kind["rpc_handler"].classified_bps(), 10_000);
    }

    /// ACCEPTANCE C. An unclassified surface FAILS, and the refusal NAMES it — a count
    /// alone would not tell an operator which surface to classify.
    #[test]
    fn an_unclassified_surface_refuses_and_names_it() {
        let consumers = index_consumers(&metadata("")).expect("index");
        let error = align(
            "/opt/omp",
            "omp/18.0.11",
            &[entry("rpc_handler", "session/list")],
            &consumers,
        )
        .expect_err("must refuse");
        let rendered = error.to_string();
        assert!(rendered.contains("ALIGN_UNCLASSIFIED"), "{rendered}");
        assert!(rendered.contains("session/list"), "must name it: {rendered}");
        assert!(
            rendered.contains("Silence is not an exception"),
            "must say why: {rendered}"
        );
        assert_eq!(error.exit_code(), 2);
    }

    /// ACCEPTANCE D, known-GOOD. A fully-classified inventory PASSES. An attack-only gate
    /// ships over-strict and gets routed around, which is a slower death than none.
    #[test]
    fn a_fully_classified_inventory_passes() {
        let json = metadata(
            r#"{"kind":"transport","name":"mux","call_site":"src/lib.rs"},
               {"kind":"rpc_handler","name":"get_state","call_site":"src/omp_state.rs"}"#,
        );
        let consumers = index_consumers(&json).expect("index");
        let report = align(
            "/opt/omp",
            "omp/18.0.11",
            &[entry("transport", "mux"), entry("rpc_handler", "get_state")],
            &consumers,
        )
        .expect("must pass");
        assert_eq!(report.rows.len(), 2);
        assert!(report.rows.iter().all(|row| row.alignment.is_classified()));
        assert_eq!(report.measured_version, "omp/18.0.11");
        assert_eq!(report.measured_install, "/opt/omp");
    }

    /// ACCEPTANCE E, ANTI-VACUITY. An empty derived surface set is an ERROR, and its code
    /// is the INSTRUMENT code (3), not the content code (2): "the probe could not read the
    /// surface" and "the surface is unclassified" have opposite remedies.
    #[test]
    fn an_empty_surface_set_is_an_instrument_error_never_a_pass() {
        let consumers = index_consumers(&metadata("")).expect("index");
        let error = align("/opt/omp", "omp/18.0.11", &[], &consumers).expect_err("must refuse");
        assert_eq!(error, AlignmentError::EmptySurfaceSet);
        assert!(error.to_string().contains("UNMEASURED and never a pass"));
        assert_eq!(error.exit_code(), 3, "an instrument failure is 3, not 2");
    }

    /// Unreadable metadata is distinct from "nobody declared anything". Collapsing them
    /// would let a broken probe read as an honest empty answer.
    #[test]
    fn unreadable_metadata_is_distinct_from_nobody_declaring() {
        assert_eq!(
            index_consumers("not json").expect_err("must refuse"),
            AlignmentError::NoPackagesScanned
        );
        assert_eq!(
            index_consumers(r#"{"packages":[]}"#).expect_err("must refuse"),
            AlignmentError::NoPackagesScanned
        );
        // A package with NO declaration is not a defect -- it is simply not a consumer.
        let consumers = index_consumers(r#"{"packages":[{"name":"solo"}]}"#).expect("index");
        assert!(consumers.declarations.is_empty());
        assert!(consumers.defects.is_empty());
        assert_eq!(consumers.packages_scanned, 1);
    }

    /// A malformed declaration is REFUSED, not dropped. Dropping it would turn a typo into
    /// an invisible unclassified surface — the failure mode is silent, which is the worst
    /// direction.
    #[test]
    fn a_malformed_declaration_is_refused_rather_than_dropped() {
        let json = metadata(r#"{"kind":"rpc_handler","name":"get_state"}"#); // no call_site
        let consumers = index_consumers(&json).expect("index");
        assert_eq!(consumers.declarations.len(), 0, "the row must not be accepted");
        assert_eq!(consumers.defects.len(), 1);
        let error = align(
            "/opt/omp",
            "omp/18.0.11",
            &[entry("rpc_handler", "get_state")],
            &consumers,
        )
        .expect_err("must refuse");
        let rendered = error.to_string();
        assert!(rendered.contains("ALIGN_MALFORMED_DECLARATIONS"), "{rendered}");
        assert!(rendered.contains("ALIGN_DECL_MISSING_FIELD"), "{rendered}");
        assert!(rendered.contains("call_site"), "must name the field: {rendered}");
        assert_eq!(error.exit_code(), 2);
    }

    /// An EMPTY declared field is a defect too. `name = ""` would otherwise match nothing
    /// and read as a declaration that exists.
    #[test]
    fn an_empty_declared_field_is_a_defect() {
        let json = metadata(r#"{"kind":"rpc_handler","name":"  ","call_site":"src/x.rs"}"#);
        let consumers = index_consumers(&json).expect("index");
        assert!(consumers.declarations.is_empty());
        assert!(
            consumers
                .defects
                .iter()
                .any(|defect| matches!(defect, DeclarationDefect::EmptyField { field: "name", .. })),
            "{:?}",
            consumers.defects
        );
    }

    /// THE ALLOWANCE LIST IS EMPTY BY DESIGN, and this pins it. A future row must be added
    /// deliberately and carry its owner and death condition.
    #[test]
    fn the_deliberately_not_allowance_is_empty_and_every_row_would_carry_its_reason() {
        assert!(
            DELIBERATELY_NOT.is_empty(),
            "the allowance list is empty by design; a row here is a reviewed decision"
        );
        for row in DELIBERATELY_NOT {
            assert!(!row.owner.is_empty(), "{}:{} has no owner", row.kind, row.name);
            assert!(!row.reason.is_empty(), "{}:{} has no reason", row.kind, row.name);
            assert!(
                !row.dies_when.is_empty(),
                "{}:{} has no dies_when -- a row nobody can retire accumulates forever",
                row.kind,
                row.name
            );
        }
    }

    /// UNCLASSIFIED must never come from a default branch. Proven behaviourally: an entry
    /// classified UNCLASSIFIED becomes CONSUMED the moment a declaration exists, so the
    /// verdict is a function of evidence and not of falling through.
    #[test]
    fn unclassified_is_a_tested_residual_not_a_fallthrough() {
        let subject = entry("rpc_handler", "session/fork");
        let empty = index_consumers(&metadata("")).expect("index");
        assert_eq!(classify(&subject, &empty), Alignment::Unclassified);

        let declared = index_consumers(&metadata(
            r#"{"kind":"rpc_handler","name":"session/fork","call_site":"src/fork.rs"}"#,
        ))
        .expect("index");
        assert!(
            classify(&subject, &declared).is_classified(),
            "the same entry must classify once evidence exists"
        );
    }

    /// COVERAGE IS A RATIO, per rule 10. A ratchet on an absolute count is
    /// red-by-construction the next time the subject legitimately grows.
    #[test]
    fn coverage_is_expressed_per_kind_as_a_ratio() {
        let json = metadata(r#"{"kind":"cli","name":"read","call_site":"src/read.rs"}"#);
        let consumers = index_consumers(&json).expect("index");
        let error = align(
            "/opt/omp",
            "omp/18.0.11",
            &[entry("cli", "read"), entry("cli", "grep")],
            &consumers,
        )
        .expect_err("grep is unclassified");
        assert!(matches!(&error, AlignmentError::Unclassified(rows) if rows.len() == 1));

        // And the ratio itself, on a passing set.
        let both = index_consumers(&metadata(
            r#"{"kind":"cli","name":"read","call_site":"a.rs"},
               {"kind":"cli","name":"grep","call_site":"b.rs"}"#,
        ))
        .expect("index");
        let report = align(
            "/opt/omp",
            "omp/18.0.11",
            &[entry("cli", "read"), entry("cli", "grep")],
            &both,
        )
        .expect("passes");
        let coverage = report.coverage_by_kind["cli"];
        assert_eq!(coverage.total, 2);
        assert_eq!(coverage.consumed, 2);
        assert_eq!(coverage.classified_bps(), 10_000);
        // A zero-total kind must not divide by zero and must not read as 100%.
        assert_eq!(
            KindCoverage { total: 0, consumed: 0, deliberately_not: 0, unclassified: 0 }
                .classified_bps(),
            0
        );
    }

    /// NO TYPED COUNTS. The report carries what it measured instead, so a stale report is
    /// visibly stale rather than silently wrong. Measured 2026-09-08: the RPC surface has
    /// been published as 14, as 3, and as 39 on a different axis, while one handshake
    /// advertised 590 -- and --mode=rpc reads {id,type} frames, not JSON-RPC, so those were
    /// not even the same channel.
    #[test]
    fn the_report_names_which_omp_it_measured() {
        let consumers = index_consumers(&metadata(
            r#"{"kind":"transport","name":"mux","call_site":"src/lib.rs"}"#,
        ))
        .expect("index");
        let report = align(
            // Synthetic, NOT the author's real install root. path-literal-guard refused an
            // earlier version of this line that pasted the home path, and it was right: a
            // machine-specific literal in a test is unportable on any other checkout, and
            // the property under test is that the report CARRIES its install path -- which
            // a synthetic value exercises identically.
            "node_modules/@oh-my-pi/pi-coding-agent",
            "omp/18.0.11",
            &[entry("transport", "mux")],
            &consumers,
        )
        .expect("passes");
        assert!(report.measured_install.contains("pi-coding-agent"));
        assert!(report.measured_version.starts_with("omp/"));
        assert_eq!(report.packages_scanned, 2);
    }

    /// Exit vocabulary shared with `repair`/`undo`, not merely parallel: content refusals
    /// are 2, instrument failures are 3, and 4 stays reserved for upstream-unreachable.
    #[test]
    fn exit_vocabulary_separates_content_from_instrument_and_reserves_four() {
        let content = AlignmentError::Unclassified(vec![entry("cli", "x")]).exit_code();
        let instrument = AlignmentError::EmptySurfaceSet.exit_code();
        assert_eq!(content, 2);
        assert_eq!(instrument, 3);
        assert_ne!(content, instrument);
        for code in [content, instrument] {
            assert_ne!(code, 4, "4 is reserved for upstream-unreachable");
            assert_ne!(code, 0, "a refusal must never exit 0");
        }
    }

    /// A stale allowance row -- one naming a surface that no longer exists -- is reported
    /// rather than silently kept. With an empty list this asserts the mechanism runs and
    /// finds nothing, which is the honest state today.
    #[test]
    fn stale_allowance_rows_are_reported() {
        let consumers = index_consumers(&metadata(
            r#"{"kind":"transport","name":"mux","call_site":"src/lib.rs"}"#,
        ))
        .expect("index");
        let report = align("/opt/omp", "omp/18.0.11", &[entry("transport", "mux")], &consumers)
            .expect("passes");
        assert_eq!(
            report.stale_allowances.len(),
            DELIBERATELY_NOT.len(),
            "with an empty allowance list there is nothing stale to report"
        );
    }
}
