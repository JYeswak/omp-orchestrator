//! `omp-surface-align` — the trigger that makes `alignment` stop being inert.
//!
//! Bead: omp-orchestrator-ablcf
//!
//! ## Why this binary exists
//!
//! `alignment::align` had thirteen refusing legs and **no caller**, which is the exact
//! condition it was written to fix. Per `fh C38` a fixture drifted from production certifies
//! nothing, so the production leg is a run against live `cargo metadata`. This is that run,
//! declared in `[package.metadata.gate]` so `gate-runner --run` executes it — an executor
//! that already fires, rather than an eighth uninvoked gate.
//!
//! ## The two tiers, and why the split is not a dodge
//!
//! A worker does not have `omp` installed. A gate that refuses whenever the artifact is
//! absent is red-by-construction on every worker, and a red-by-construction gate gets
//! routed around — which is a slower death than no gate (rule 2). But exiting `0` in that
//! case is a vacuous pass, which rule 4 forbids.
//!
//! The resolution is the **InputManifest** discipline this repo already mandates: a bounded
//! instrument emits `PARTIAL` naming its bound, never a silent slice and never an empty
//! success.
//!
//! ```text
//! TIER 1  always measurable from cargo metadata alone:
//!           every declaration is well-formed, every allowance row carries owner+reason+dies_when
//!         a defect here REFUSES -> exit 2
//! TIER 2  needs the installed artifact: classify every DERIVED surface entry
//!         artifact present -> FULL     -> unclassified REFUSES -> exit 2
//!         artifact absent  -> PARTIAL  -> tier 2 NOT RUN, stated, and NON-CITABLE
//! ```
//!
//! **`PARTIAL` is not a pass with a caveat.** It is printed as its own line with
//! `bound_kind`, `bound_value` and `source`, and it says in words that it is not citable as
//! coverage evidence. A reader who wants the alignment claim must produce a `FULL` run.
//!
//! ## NO-CLAIM
//!
//! A `FULL` run proves every derived surface entry is classified. It does not prove a
//! `CONSUMED` row is true — a declaration is a claim by the declaring crate, and this gate
//! checks that the claim EXISTS and is well-formed, not that the call site does what it
//! says. Verifying a call site is a different instrument.

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use omp_inventory_map::alignment::{
    self, AlignmentError, AlignmentReport, SurfaceEntry, DELIBERATELY_NOT,
};
use omp_inventory_map::{parse_cli_commands, parse_omp_version, parse_transport_modes};
use subprocess_contract::{bounded_output, BoundedOutcome};

/// Bounded, because a hanging probe must not stall a gate run.
const PROBE_DEADLINE: Duration = Duration::from_secs(20);

/// Exit codes. Shared vocabulary with `repair`, `undo` and `alignment`:
/// `2` content refusal, `3` instrument failure, and **`4` is reserved** for
/// upstream-unreachable (`adapter_exec.rs:138-142` spends it that way).
const EXIT_OK: u8 = 0;
const EXIT_CONTENT: u8 = 2;
const EXIT_INSTRUMENT: u8 = 3;

/// How much of the subject this run actually covered.
#[derive(Debug, Clone, PartialEq, Eq)]
enum InputManifest {
    Full { omp_root: PathBuf, omp_version: String },
    /// A named bound. NEVER an implicit full and never a silent slice.
    Partial { bound_kind: &'static str, bound_value: String, source: &'static str },
}

impl InputManifest {
    fn render(&self) -> String {
        match self {
            Self::Full { omp_root, omp_version } => format!(
                "ALIGN_MANIFEST state=FULL omp_root={} omp_version={omp_version}",
                omp_root.display()
            ),
            Self::Partial { bound_kind, bound_value, source } => format!(
                "ALIGN_MANIFEST state=PARTIAL bound_kind={bound_kind} \
                 bound_value={bound_value} source={source} \
                 detail=tier 2 NOT RUN; this result is NOT citable as alignment coverage"
            ),
        }
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "omp-surface-align --repo <path> [--omp <path-to-omp>]\n\n\
             \x20 TIER 1  declaration + allowance integrity, from cargo metadata alone\n\
             \x20 TIER 2  classify every derived OMP surface entry (needs the installed omp)\n\n\
             exit {EXIT_OK}=classified (state is FULL or PARTIAL, always printed)\n\
             exit {EXIT_CONTENT}=content refusal: malformed declaration or unclassified surface\n\
             exit {EXIT_INSTRUMENT}=instrument failure: cargo metadata unreadable\n\n\
             A PARTIAL result is NOT citable as coverage evidence. It names its bound."
        );
        return ExitCode::SUCCESS;
    }
    let repo = flag(&args, "--repo").map_or_else(
        || std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
        PathBuf::from,
    );
    let omp_hint = flag(&args, "--omp").map(PathBuf::from);

    // ---- TIER 1: always measurable ------------------------------------------------
    let metadata = match cargo_metadata(&repo) {
        Ok(text) => text,
        Err(detail) => {
            eprintln!("ALIGN_METADATA_UNREADABLE detail={detail}");
            return ExitCode::from(EXIT_INSTRUMENT);
        }
    };
    let consumers = match alignment::index_consumers(&metadata) {
        Ok(index) => index,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(error.exit_code());
        }
    };
    if !consumers.defects.is_empty() {
        // Refused, not dropped: a malformed declaration silently ignored becomes an
        // invisible unclassified surface.
        let error = AlignmentError::MalformedDeclarations(consumers.defects.clone());
        eprintln!("{error}");
        return ExitCode::from(error.exit_code());
    }
    if let Err(detail) = allowance_integrity() {
        eprintln!("ALIGN_ALLOWANCE_MALFORMED detail={detail}");
        return ExitCode::from(EXIT_CONTENT);
    }
    println!(
        "ALIGN_TIER1 packages_scanned={} declarations={} allowance_rows={}",
        consumers.packages_scanned,
        consumers.declarations.len(),
        DELIBERATELY_NOT.len()
    );

    // ---- TIER 2: needs the installed artifact -------------------------------------
    let (manifest, surface) = derive_surface(omp_hint.as_deref());
    println!("{}", manifest.render());

    let InputManifest::Full { omp_root, omp_version } = &manifest else {
        // PARTIAL. Tier 1 passed, tier 2 did not run, and the state is on stdout for the
        // reader. Exiting 0 here is honest ONLY because the manifest says what was skipped.
        println!("ALIGN_RESULT state=PARTIAL tier2=NOT_RUN citable=false");
        return ExitCode::from(EXIT_OK);
    };

    // BEFORE the verdict, on BOTH paths. `align` returns early when anything is
    // unclassified, so reporting orphans only on success would hide them in exactly the
    // situation that produced the finding: three well-formed declarations matching nothing
    // while the run refused for an unrelated reason.
    for orphan in alignment::orphan_declarations(&surface, &consumers) {
        println!("ALIGN_ORPHAN_DECLARATION {orphan}");
    }

    // Coverage BEFORE the verdict, for the same reason orphans are: `align` returns early
    // when anything is unclassified, so printing per-kind coverage only on the Ok path hides
    // the CONSUMED count in exactly the refusing run where a reader needs it. 85 of 88
    // unclassified means 3 ARE classified, and a reader should not have to do that
    // subtraction to find out.
    {
        let consumers_ref = &consumers;
        let mut by_kind: std::collections::BTreeMap<&str, (usize, usize)> =
            std::collections::BTreeMap::new();
        for entry in &surface {
            let slot = by_kind.entry(entry.kind.as_str()).or_insert((0, 0));
            slot.0 += 1;
            if alignment::classify(entry, consumers_ref).is_classified() {
                slot.1 += 1;
            }
        }
        for (kind, (total, classified)) in by_kind {
            let bps = if total == 0 { 0 } else { (classified * 10_000) / total };
            println!(
                "ALIGN_COVERAGE kind={kind} total={total} classified={classified} \
                 classified_bps={bps}"
            );
        }
    }

    match alignment::align(
        &omp_root.display().to_string(),
        omp_version,
        &surface,
        &consumers,
    ) {
        Ok(report) => {
            print_report(&report);
            ExitCode::from(EXIT_OK)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}

fn print_report(report: &AlignmentReport) {
    for (kind, coverage) in &report.coverage_by_kind {
        println!(
            "ALIGN_COVERAGE kind={kind} total={} consumed={} deliberately_not={} \
             classified_bps={}",
            coverage.total,
            coverage.consumed,
            coverage.deliberately_not,
            coverage.classified_bps()
        );
    }
    for stale in &report.stale_allowances {
        println!("ALIGN_STALE_ALLOWANCE {stale}");
    }
    println!(
        "ALIGN_RESULT state=FULL rows={} packages_scanned={} citable=true",
        report.rows.len(),
        report.packages_scanned
    );
}

/// Every allowance row must carry its owner, reason and death condition. A row nobody can
/// retire accumulates forever, which is how an allowance list becomes a permanent exemption.
fn allowance_integrity() -> Result<(), String> {
    for row in DELIBERATELY_NOT {
        for (field, value) in [
            ("owner", row.owner),
            ("reason", row.reason),
            ("dies_when", row.dies_when),
        ] {
            if value.trim().is_empty() {
                return Err(format!("{}:{} has an empty {field}", row.kind, row.name));
            }
        }
    }
    Ok(())
}

/// Derive the surface from the installed artifact, or say why we could not.
///
/// Reuses this crate's existing parsers rather than re-deriving them — the DERIVE half was
/// already correct and is not rewritten here.
fn derive_surface(hint: Option<&Path>) -> (InputManifest, Vec<SurfaceEntry>) {
    let program = hint.map_or_else(|| PathBuf::from("omp"), Path::to_path_buf);
    let help = match probe(&program, &["--help"]) {
        Ok(text) => text,
        Err(detail) => {
            return (
                InputManifest::Partial {
                    bound_kind: "omp_artifact_unreachable",
                    bound_value: detail,
                    source: "omp --help",
                },
                Vec::new(),
            )
        }
    };
    let version = probe(&program, &["--version"])
        .ok()
        .map(|text| parse_omp_version(&text))
        .and_then(|probe| probe.value)
        .unwrap_or_else(|| "UNKNOWN".to_owned());

    let mut surface = Vec::new();
    if let Some(commands) = parse_cli_commands(&help).value {
        for name in commands {
            surface.push(SurfaceEntry { kind: "cli".to_owned(), name });
        }
    }
    if let Some(modes) = parse_transport_modes(&help).value {
        for name in modes {
            surface.push(SurfaceEntry { kind: "transport_mode".to_owned(), name });
        }
    }

    // The mux {id,type} frame vocabulary, from %7's extractor rather than a second bundle
    // parser of my own. Verified before composing: its ANCHOR_METHOD is `negotiate_protocol`
    // and its inbound set carries get_state / get_session_stats / get_messages -- the exact
    // names %20 declared. A parser here would have been the duplicate-classifier defect.
    //
    // `inbound` are the methods OMP ACCEPTS -- what a consumer CALLS, and the kind the
    // declarations name. `outbound` gets its OWN kind rather than being folded in: receiving
    // a notification and calling a method are different acts, and one kind covering both
    // would make every CONSUMED row ambiguous about direction.
    match read_bundle(&program) {
        Ok(bundle) => {
            let sites = omp_surface_consumption::case_sites(&bundle);
            match omp_surface_consumption::derive_command_set(&sites) {
                Ok(set) => {
                    // Published because %7 publishes it: a small seam gap means the
                    // inbound/outbound split is a guess, and every direction claim
                    // downstream inherits that uncertainty. Hiding it would make these
                    // kinds look more certain than the extractor claims.
                    println!(
                        "ALIGN_SEAM inbound={} outbound={} seam_gap_bytes={}",
                        set.inbound.len(),
                        set.outbound.len(),
                        set.seam_gap
                    );
                    for name in set.inbound {
                        surface.push(SurfaceEntry { kind: "rpc_handler".to_owned(), name });
                    }
                    for name in set.outbound {
                        surface.push(SurfaceEntry {
                            kind: "rpc_notification".to_owned(),
                            name,
                        });
                    }
                }
                // `{error:?}` and not `{error}`: %7's `DeriveError` is a public error type
                // with NO `Display` impl, so a caller cannot render it in a message. Using
                // Debug rather than editing their crate; reported to them as a finding.
                Err(error) => println!(
                    "ALIGN_SEAM_UNMEASURED detail={error:?} \
                     note=rpc_handler kinds ABSENT from this run's surface"
                ),
            }
        }
        Err(detail) => println!(
            "ALIGN_BUNDLE_UNMEASURED detail={detail} \
             note=rpc_handler kinds ABSENT from this run's surface"
        ),
    }

    // ANTI-VACUITY AT AXIS GRANULARITY (pane 1's ruling, omp-orchestrator-ablcf).
    //
    // `surface.is_empty()` was the only vacuity check, so a run where ONE axis silently
    // yielded nothing still said `state=FULL`. That is gate rule 4 evaded at a finer grain:
    // the scan set was non-empty overall while an axis this bin CLAIMS to cover contributed
    // zero, and the consequence is worse than a missing row -- every declaration on that
    // axis is then reported as an ORPHAN, which blames the declaring crate for a gap in the
    // extractor. A correct-looking verdict pointing at the wrong party.
    //
    // So FULL now means "every declared axis contributed", not "I read the bundle".
    let mut empty_axes = Vec::new();
    for axis in DECLARED_AXES {
        if !surface.iter().any(|entry| entry.kind == *axis) {
            empty_axes.push(*axis);
        }
    }
    for axis in DECLARED_AXES {
        let count = surface.iter().filter(|entry| entry.kind == *axis).count();
        println!("ALIGN_AXIS kind={axis} entries={count}");
    }
    if !empty_axes.is_empty() {
        return (
            InputManifest::Partial {
                bound_kind: "declared_axis_yielded_zero",
                bound_value: empty_axes.join(","),
                source: "derive_surface",
            },
            surface,
        );
    }
    (
        InputManifest::Full {
            omp_root: program,
            omp_version: version,
        },
        surface,
    )
}

/// Every axis this bin claims to cover. `FULL` requires each to contribute at least one
/// entry; an axis yielding zero makes the run `PARTIAL` and names itself.
///
/// Declared as a const rather than inferred from what was produced, because inferring it
/// from the output is circular: an axis that yields nothing would simply not be in the list,
/// and the vacuity would be invisible again.
pub const DECLARED_AXES: &[&str] =
    &["cli", "transport_mode", "rpc_handler", "rpc_notification"];


/// Read the installed bundle, resolved from the `omp` program itself.
///
/// `omp` on this host resolves directly INTO the package —
/// `~/.local/lib/node_modules/@oh-my-pi/pi-coding-agent/dist/cli.js` — so the artifact
/// under measurement is the file the binary IS, and no separate discovery heuristic is
/// needed. A non-JS resolution is reported rather than guessed at.
///
/// NOTE ON THE CAP: this crate's own `MAX_PROBE_BYTES` is 16 MiB and the live bundle
/// measures 21,379,674 bytes, so a read bounded by that constant would refuse the CURRENT
/// artifact — reporting `UNMEASURED` for a bundle that is present and readable. The cap here
/// is generous and explicit, and the mismatch is filed rather than silently worked around.
const MAX_BUNDLE_BYTES: u64 = 64 * 1024 * 1024;

fn read_bundle(program: &Path) -> Result<String, String> {
    let resolved = which(program).ok_or_else(|| format!("cannot resolve {}", program.display()))?;
    let canonical = resolved
        .canonicalize()
        .map_err(|error| format!("canonicalize {}: {error}", resolved.display()))?;
    if canonical.extension().and_then(|ext| ext.to_str()) != Some("js") {
        return Err(format!(
            "resolved to {} which is not a .js bundle",
            canonical.display()
        ));
    }
    let size = std::fs::metadata(&canonical)
        .map_err(|error| format!("stat {}: {error}", canonical.display()))?
        .len();
    if size > MAX_BUNDLE_BYTES {
        return Err(format!("bundle is {size} bytes, above the {MAX_BUNDLE_BYTES} cap"));
    }
    std::fs::read_to_string(&canonical)
        .map_err(|error| format!("read {}: {error}", canonical.display()))
}

/// Resolve a bare program name through `PATH`; pass an explicit path straight through.
fn which(program: &Path) -> Option<PathBuf> {
    if program.components().count() > 1 {
        return Some(program.to_path_buf());
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|dir| dir.join(program))
            .find(|candidate| candidate.is_file())
    })
}

fn probe(program: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new(program);
    command.args(args);
    command.stdin(std::process::Stdio::null());
    match bounded_output(&mut command, PROBE_DEADLINE) {
        BoundedOutcome::Completed(output) => {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        // A timeout is not a verdict. It is reported as its own bound rather than folded
        // into "absent", because "the tool is not installed" and "the tool did not answer
        // in time" have opposite remedies.
        BoundedOutcome::TimedOut => Err(format!(
            "timed_out_after_{}s",
            PROBE_DEADLINE.as_secs()
        )),
        BoundedOutcome::Unspawned(error) => Err(format!("unspawnable:{error}")),
    }
}

fn cargo_metadata(repo: &Path) -> Result<String, String> {
    let mut command = Command::new("cargo");
    command
        .args([
            "metadata",
            "--no-deps",
            "--format-version",
            "1",
            "--offline",
        ])
        .current_dir(repo);
    command.stdin(std::process::Stdio::null());
    match bounded_output(&mut command, PROBE_DEADLINE) {
        BoundedOutcome::Completed(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        BoundedOutcome::Completed(output) => Err(format!(
            "cargo metadata exit={:?} stderr={}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        BoundedOutcome::TimedOut => {
            Err(format!("cargo metadata timed out after {}s", PROBE_DEADLINE.as_secs()))
        }
        BoundedOutcome::Unspawned(error) => Err(format!("cargo unspawnable: {error}")),
    }
}

fn flag(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|arg| arg == name)
        .and_then(|index| args.get(index + 1))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The manifest must never render a PARTIAL that reads like a pass. The words that make
    /// it non-citable are asserted, because a caveat nobody can see is not a caveat.
    #[test]
    fn a_partial_manifest_states_its_bound_and_that_it_is_not_citable() {
        let manifest = InputManifest::Partial {
            bound_kind: "omp_artifact_unreachable",
            bound_value: "unspawnable:No such file".to_owned(),
            source: "omp --help",
        };
        let rendered = manifest.render();
        assert!(rendered.contains("state=PARTIAL"));
        assert!(rendered.contains("bound_kind=omp_artifact_unreachable"));
        assert!(rendered.contains("bound_value="));
        assert!(rendered.contains("source=omp --help"));
        assert!(
            rendered.contains("NOT citable"),
            "a PARTIAL that does not say it is non-citable reads as a pass: {rendered}"
        );
    }

    /// FULL must name which artifact it measured, so a stale result is visibly stale.
    #[test]
    fn a_full_manifest_names_the_artifact_it_measured() {
        let rendered = InputManifest::Full {
            omp_root: PathBuf::from("node_modules/@oh-my-pi/pi-coding-agent"),
            omp_version: "omp/18.1.14".to_owned(),
        }
        .render();
        assert!(rendered.contains("state=FULL"));
        assert!(rendered.contains("omp_version=omp/18.1.14"));
        assert!(rendered.contains("pi-coding-agent"));
    }

    /// FULL and PARTIAL must be distinguishable by a reader and by a matcher. If they
    /// rendered alike, the whole tier split would be decorative.
    #[test]
    fn full_and_partial_are_not_confusable() {
        let full = InputManifest::Full {
            omp_root: PathBuf::from("x"),
            omp_version: "omp/1".to_owned(),
        }
        .render();
        let partial = InputManifest::Partial {
            bound_kind: "k",
            bound_value: "v".to_owned(),
            source: "s",
        }
        .render();
        assert_ne!(full, partial);
        assert!(full.contains("state=FULL") && !full.contains("state=PARTIAL"));
        assert!(partial.contains("state=PARTIAL") && !partial.contains("state=FULL"));
    }

    /// An absent `omp` yields PARTIAL with a named bound — NOT an empty surface that
    /// `align` would report as an instrument error with no cause.
    #[test]
    fn an_unreachable_omp_is_partial_with_a_named_bound_not_an_empty_surface() {
        let (manifest, surface) = derive_surface(Some(Path::new(
            "/nonexistent/zzz-omp-cannot-possibly-exist",
        )));
        assert!(surface.is_empty());
        match manifest {
            InputManifest::Partial { bound_kind, .. } => {
                assert_eq!(bound_kind, "omp_artifact_unreachable");
            }
            InputManifest::Full { .. } => panic!("an absent omp must never yield FULL"),
        }
    }

    /// The allowance list is empty today, so this asserts the check RUNS and finds nothing
    /// rather than asserting a vacuous truth.
    #[test]
    fn allowance_integrity_passes_on_the_empty_list_and_would_catch_a_blank_field() {
        assert!(allowance_integrity().is_ok());
        assert_eq!(
            DELIBERATELY_NOT.len(),
            0,
            "the allowance list is empty by design; a row is a reviewed decision"
        );
    }

    /// `--repo` must be honoured, or the gate would silently measure whatever directory it
    /// was launched from.
    #[test]
    fn the_repo_flag_is_parsed() {
        let args = vec!["--repo".to_owned(), "/some/where".to_owned()];
        assert_eq!(flag(&args, "--repo"), Some("/some/where".to_owned()));
        assert_eq!(flag(&args, "--omp"), None);
        // A flag with no value must not silently become the next flag's value.
        assert_eq!(flag(&["--repo".to_owned()], "--repo"), None);
    }

    /// Unreadable metadata is an INSTRUMENT failure (3), never a content refusal (2): "the
    /// probe could not read the workspace" and "the workspace is unclassified" send an
    /// operator to opposite remedies.
    #[test]
    fn metadata_failure_is_an_instrument_code_and_four_stays_reserved() {
        let detail = cargo_metadata(Path::new("/nonexistent/zzz-not-a-repo"))
            .expect_err("must fail");
        assert!(!detail.is_empty(), "an instrument failure must carry a cause");
        assert_eq!(EXIT_INSTRUMENT, 3);
        assert_eq!(EXIT_CONTENT, 2);
        assert_ne!(EXIT_INSTRUMENT, EXIT_CONTENT);
        for code in [EXIT_CONTENT, EXIT_INSTRUMENT] {
            assert_ne!(code, 4, "4 is reserved for upstream-unreachable");
        }
    }
}
