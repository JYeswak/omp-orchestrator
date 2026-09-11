//! Typed preconditions for host-only test legs: UNMEASURABLE is not a failure.
//!
//! # Why this exists (`omp-orchestrator-g5j5b`)
//!
//! `cargo test -p omp-orchestrator --lib` returned `227 passed / 5 failed /
//! exit=101` at `3bcd95a`, and those five failed identically BEFORE and AFTER
//! every change, because the Contabo build workers have no tmux pane and no
//! `br` on `PATH`:
//!
//! ```text
//! SENDER_IDENTITY_REFUSED reason=TMUX_PANE_missing        x4
//! br init: Process(NotFound("br"))                        x1
//! ```
//!
//! **So the passing baseline for this crate was a NONZERO EXIT.** That defeats
//! AGENTS.md gate rule 7 on this crate specifically: a known-bad leg must pin
//! its message AND its exit code because neither subsumes the other, but here
//! `exit=101` was both the green and the red, so the exit half carried zero
//! information and every mutation leg silently degraded to message-only. It was
//! measured that way in practice — the `M4` mutation on `3bcd95a` was
//! `222 passed / 10 failed, exit=101` against a green of
//! `227 passed / 5 failed, exit=101`.
//!
//! It also makes a suite verdict useless as a restore oracle, which is the
//! third of tonight's plant-protocol rules: "the suite went green" returns the
//! same answer whether or not a byte-identical restore worked.
//!
//! # The classification is not invented here
//!
//! `gate-runner` already classifies a whole crate as `UNMEASURABLE` with a
//! remedy-selecting reason code for exactly this condition (`br` and `.beads`
//! absent on the workers), and refuses to launder it into either a pass or a
//! failure. This is that classification one level down, at the leg. The two
//! codes are kept distinct for the same reason `gate-runner` keeps
//! `MISSING_EXECUTABLE` apart from `POLICY_UNAVAILABLE`: an absent tmux pane
//! and an absent executable are repaired in different places, and a report that
//! printed both as "skipped" would send both repairs to the wrong one.
//!
//! # Why this is not `#[ignore]`
//!
//! An `#[ignore]`d leg is not a passing leg, and `--include-ignored` has zero
//! callers in this repo, so ignoring converts a VISIBLE false red into an
//! INVISIBLE coverage hole. Here the leg still runs, still asserts whenever the
//! host can answer, and says so when it cannot.
//!
//! # The trap this module is written to avoid
//!
//! A classifier whose compliant verdict is the DEFAULT branch re-acquires the
//! defect every time someone adds a case. So [`classify`] requires POSITIVE
//! evidence for every named requirement and treats the residual as
//! unmeasurable, and [`live_probe`] matches [`HostRequirement`] exhaustively
//! with no wildcard arm: a new variant fails to compile rather than defaulting
//! to "present".

use std::path::{Path, PathBuf};

/// A fact about the HOST that a leg cannot synthesise.
///
/// Not "a thing the test needs" in general — fixtures cover those. These are
/// the properties that belong to the machine the suite happens to run on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostRequirement {
    /// The process must be inside a tmux pane. `mail_sender_pane_identity`
    /// resolves `TMUX_PANE` immediately before sending and refuses
    /// `SENDER_IDENTITY_REFUSED reason=TMUX_PANE_missing` without it, so every
    /// leg reaching `prepare_bead_dispatch` inherits the requirement.
    TmuxPane,
    /// `br` must be resolvable on `PATH`. The finding kernel shells the real
    /// tracker; a worker without it reports `Process(NotFound("br"))`.
    TrackerBinary,
    /// `$HOME/.local/bin/cargo` — the guarded cargo shim. `target_ownership`
    /// SPAWNS it to observe the wrapper's own guard behaviour, so a host
    /// without it reports `spawn wrapper cargo: NotFound`.
    HostCargoShim,
    /// `$HOME/.rustup/toolchains/nightly-aarch64-apple-darwin/bin/cargo-rch-real`.
    /// THE PLATFORM IS IN THE PATH LITERAL, so this can only ever hold on an
    /// Apple-Silicon Darwin host: `spawn real cargo: NotFound` anywhere else.
    DarwinToolchainCargo,
    /// `/usr/bin/shasum`. macOS ships it; Linux ships `sha256sum` and has no
    /// such path, which is why the leg that hashes the wrapper panics on a
    /// bare `hash` expect rather than on a comparison.
    ShasumTool,
    /// The registered target root must be MOUNTED. A path under `/Volumes` is
    /// absent rather than empty when its volume is not attached, and those are
    /// different facts.
    RegisteredRootMount,
    /// `CARGO_TARGET_DIR` must be UNSET or resolve INSIDE the repository — the
    /// build's own target directory, not a relocated one.
    ///
    /// ⛔ NAMED IN THE POSITIVE ON PURPOSE, because every other variant means
    /// "this must be PRESENT for the leg to be measurable" and `live_probe`
    /// returns `true` on presence. A `RelocatedTargetDir` spelling would read
    /// "relocation is required", its probe would return `true` exactly when
    /// relocated, and the guarded legs would skip on the UNRELOCATED host —
    /// the one place they CAN be measured. That is an inverted guard reached
    /// by a naming choice rather than by a logic error, so the variant names
    /// the REQUIRED state and `code()` names the VIOLATION. Inverting either
    /// alone is visible; inverting both cancels and reads correct while
    /// behaving backwards.
    ///
    /// `rch exec` relocates the target dir to
    /// `.rch-target-<worker>-pool-<hash>` by construction, so a leg asserting
    /// about `<repo>/target` cannot hold on the only lane an agent may drive.
    UnrelocatedTargetDir,
}

impl HostRequirement {
    /// Every variant, so a census cannot silently omit one.
    pub const ALL: [Self; 7] = [
        Self::TmuxPane,
        Self::TrackerBinary,
        Self::HostCargoShim,
        Self::DarwinToolchainCargo,
        Self::ShasumTool,
        Self::RegisteredRootMount,
        Self::UnrelocatedTargetDir,
    ];

    /// The thing that is absent, as an operator would name it.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::TmuxPane => "TMUX_PANE",
            Self::TrackerBinary => "br",
            Self::HostCargoShim => "host_cargo_shim",
            Self::DarwinToolchainCargo => "darwin_toolchain_cargo",
            Self::ShasumTool => "shasum",
            Self::RegisteredRootMount => "registered_root_mount",
            Self::UnrelocatedTargetDir => "unrelocated_target_dir",
        }
    }

    /// The REMEDY-SELECTING code. Two absences that are repaired in different
    /// places must not share a code — and these four are repaired in four
    /// different places, three of which are "use a different machine".
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::TmuxPane => "TMUX_PANE_ABSENT",
            Self::TrackerBinary => "MISSING_EXECUTABLE",
            Self::HostCargoShim => "HOST_SHIM_ABSENT",
            Self::DarwinToolchainCargo => "WRONG_HOST_PLATFORM",
            Self::ShasumTool => "MISSING_PLATFORM_TOOL",
            Self::RegisteredRootMount => "VOLUME_NOT_MOUNTED",
            Self::UnrelocatedTargetDir => "CARGO_TARGET_DIR_RELOCATED",
        }
    }

    /// Where the repair lives, so the line is actionable without a lookup.
    #[must_use]
    pub const fn remedy(self) -> &'static str {
        match self {
            Self::TmuxPane => "run this leg from a tmux pane on the host",
            Self::TrackerBinary => "install br on the worker PATH",
            Self::HostCargoShim => "run on the host that installs ~/.local/bin/cargo",
            Self::DarwinToolchainCargo => "run on an aarch64-apple-darwin host; this leg cannot hold on Linux",
            Self::ShasumTool => "run on macOS, or port the leg to sha256sum",
            Self::RegisteredRootMount => "mount the registered target volume",
            Self::UnrelocatedTargetDir => "run where the build owns <repo>/target; rch relocates CARGO_TARGET_DIR by construction",
        }
    }

    /// Absolute path this requirement resolves to, when it is a filesystem
    /// fact. `None` for requirements answered some other way (an env var).
    #[must_use]
    pub fn host_path(self) -> Option<PathBuf> {
        let home = || std::env::var_os("HOME").map(PathBuf::from);
        match self {
            Self::TmuxPane | Self::TrackerBinary | Self::UnrelocatedTargetDir => None,
            Self::HostCargoShim => Some(home()?.join(".local/bin/cargo")),
            Self::DarwinToolchainCargo => Some(
                home()?.join(".rustup/toolchains/nightly-aarch64-apple-darwin/bin/cargo-rch-real"),
            ),
            Self::ShasumTool => Some(PathBuf::from("/usr/bin/shasum")),
            Self::RegisteredRootMount => Some(PathBuf::from(
                "/Volumes/ZestData/zeststream-offload-20260609/build-cache/cargo-targets",
            )),
        }
    }
}

/// Whether a leg can be measured on this host.
///
/// `Measurable` is only reachable with positive evidence for every named
/// requirement; everything else is `Unmeasurable`, which is neither a pass nor
/// a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostPrecondition {
    /// Every named requirement is present. The leg MUST assert its property.
    Measurable,
    /// At least one named requirement is absent.
    Unmeasurable(UnmeasurableLeg),
}

/// A leg that could not be measured, and exactly why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnmeasurableLeg {
    pub test: &'static str,
    /// Non-empty by construction: an `UnmeasurableLeg` with nothing absent
    /// would be a skip with no reason, which is the hole this replaces.
    pub absent: Vec<HostRequirement>,
}

impl UnmeasurableLeg {
    /// The reported line. Carries the COUNT and the per-requirement reason, so
    /// "skipped" is never silent and never bare.
    #[must_use]
    pub fn render(&self) -> String {
        let reasons = self
            .absent
            .iter()
            .map(|requirement| format!("{}:{}", requirement.label(), requirement.code()))
            .collect::<Vec<_>>()
            .join(",");
        let remedies = self
            .absent
            .iter()
            .map(|requirement| requirement.remedy())
            .collect::<Vec<_>>()
            .join("; ");
        format!(
            "UNMEASURABLE test={} skipped=1 absent={} requirements={reasons} remedy={remedies}",
            self.test,
            self.absent.len(),
        )
    }
}

/// The line a MEASURED leg reports, so measured and skipped are
/// distinguishable in the output rather than both reading `... ok`.
#[must_use]
pub fn measured_line(test: &'static str, required: &[HostRequirement]) -> String {
    let present = required
        .iter()
        .map(|requirement| requirement.label())
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "MEASURED test={test} skipped=0 absent=0 requirements={}",
        if present.is_empty() {
            "NONE".to_owned()
        } else {
            present
        }
    )
}

/// Classify a leg against `required`, asking `probe` for each requirement.
///
/// `probe` returning `true` is the POSITIVE evidence that the requirement
/// holds. Anything else lands in `absent`.
pub fn classify(
    test: &'static str,
    required: &[HostRequirement],
    probe: &dyn Fn(HostRequirement) -> bool,
) -> HostPrecondition {
    let absent: Vec<HostRequirement> = required
        .iter()
        .copied()
        .filter(|requirement| !probe(*requirement))
        .collect();
    if absent.is_empty() {
        return HostPrecondition::Measurable;
    }
    HostPrecondition::Unmeasurable(UnmeasurableLeg { test, absent })
}

/// Is `program` an executable file on any entry of `path_value`?
///
/// Split out from [`live_probe`] so it can be measured against a real directory
/// instead of whatever `PATH` the suite inherits.
#[must_use]
pub fn executable_on(path_value: &str, program: &str) -> bool {
    path_value
        .split(':')
        .filter(|entry| !entry.is_empty())
        .map(|entry| Path::new(entry).join(program))
        .any(|candidate| is_executable_file(&candidate))
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .map(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

/// The live host probe. Exhaustive by construction: a new [`HostRequirement`]
/// variant must be answered here or the crate does not compile, so "present"
/// can never become a default.
/// The repository root, derived from this crate's manifest directory rather
/// than from a literal, so the probe answers the same question in any checkout.
fn repo_root() -> Option<PathBuf> {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .map(std::path::Path::to_path_buf)
}

#[must_use]
pub fn live_probe(requirement: HostRequirement) -> bool {
    match requirement {
        HostRequirement::TmuxPane => std::env::var("TMUX_PANE")
            .map(|pane| !pane.trim().is_empty())
            .unwrap_or(false),
        HostRequirement::TrackerBinary => std::env::var("PATH")
            .map(|path| executable_on(&path, "br"))
            .unwrap_or(false),
        // Filesystem facts. `host_path` owns the literal so the probe and the
        // remedy cannot disagree about WHICH path was missing — a remedy
        // naming a different path than the probe checked is a false remedy.
        HostRequirement::HostCargoShim
        | HostRequirement::DarwinToolchainCargo
        | HostRequirement::ShasumTool => requirement
            .host_path()
            .is_some_and(|path| is_executable_file(&path)),
        // A DIRECTORY, and an unmounted volume is ABSENT rather than empty.
        HostRequirement::RegisteredRootMount => {
            requirement.host_path().is_some_and(|path| path.is_dir())
        }
        // TRUE when the target dir is the build's OWN `<repo>/target` -- unset,
        // empty, or exactly that path. Returns true on the MEASURABLE state,
        // like every other arm, so the guard cannot invert on a reader.
        //
        // ⛔ `starts_with(repo)` IS THE WRONG PREDICATE AND WAS MEASURED WRONG:
        // `rch` relocates to `<repo>/.rch-target-<worker>-pool-<hash>`, which
        // IS inside the repository, so an inside-the-repo test reported
        // MEASURED on the very lane this requirement exists to exclude. The
        // guarded leg caught it by then failing for its real reason. The
        // requirement is the EXACT owned path, not repo containment.
        HostRequirement::UnrelocatedTargetDir => match std::env::var_os("CARGO_TARGET_DIR") {
            None => true,
            Some(value) if value.is_empty() => true,
            Some(value) => repo_root()
                .is_some_and(|repo| PathBuf::from(value) == repo.join("target")),
        },
    }
}

/// Gate a host-only leg against the live host.
///
/// Returns `true` when the leg MUST go on to assert its property. Returns
/// `false` after REPORTING why it could not be measured, and the caller returns
/// without asserting. `#[must_use]` so the guard cannot be called and dropped.
#[must_use]
pub fn measurable_here(test: &'static str, required: &[HostRequirement]) -> bool {
    match classify(test, required, &live_probe) {
        HostPrecondition::Measurable => {
            println!("{}", measured_line(test, required));
            true
        }
        HostPrecondition::Unmeasurable(leg) => {
            println!("{}", leg.render());
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LEG: &str = "a_host_only_leg";

    fn present(_: HostRequirement) -> bool {
        true
    }

    fn absent(_: HostRequirement) -> bool {
        false
    }

    #[test]
    fn an_absent_requirement_is_unmeasurable_and_never_measurable() {
        let verdict = classify(LEG, &HostRequirement::ALL, &absent);
        assert_eq!(
            verdict,
            HostPrecondition::Unmeasurable(UnmeasurableLeg {
                test: LEG,
                absent: HostRequirement::ALL.to_vec(),
            })
        );
        assert_ne!(
            verdict,
            HostPrecondition::Measurable,
            "an unmeasurable leg must never classify as measurable"
        );
    }

    #[test]
    fn positive_evidence_for_every_requirement_is_measurable() {
        assert_eq!(
            classify(LEG, &HostRequirement::ALL, &present),
            HostPrecondition::Measurable
        );
        // And an empty requirement list is measurable, which is what makes the
        // guard a no-op for legs that need nothing from the host.
        assert_eq!(classify(LEG, &[], &absent), HostPrecondition::Measurable);
    }

    #[test]
    fn one_absent_of_many_is_unmeasurable_and_names_only_the_absent_ones() {
        // ANTI-VACUITY for the classifier itself: the compliant verdict is not
        // a default branch, so a partially-present host cannot pass.
        //
        // MEMBERSHIP, NOT CARDINALITY (rule 10). The expected set is DERIVED
        // from `ALL` minus the one present requirement, so a seventh variant
        // cannot silently satisfy this leg — and it cannot break it either for
        // the wrong reason. A transcribed `vec![TrackerBinary]` here asserted
        // "ALL has exactly two", which is not the property under test.
        let present_one = HostRequirement::TmuxPane;
        let verdict = classify(LEG, &HostRequirement::ALL, &|requirement| {
            requirement == present_one
        });
        let HostPrecondition::Unmeasurable(leg) = verdict else {
            panic!("a partially-present host cannot be measurable");
        };
        let expected: Vec<HostRequirement> = HostRequirement::ALL
            .into_iter()
            .filter(|requirement| *requirement != present_one)
            .collect();
        assert!(
            !expected.is_empty(),
            "vacuous unless at least one requirement is absent"
        );
        assert_eq!(leg.absent, expected);
        let line = leg.render();
        assert!(line.contains(&format!("absent={}", expected.len())), "{line}");
        // The named absence is still pinned by NAME, not by position.
        assert!(line.contains("br:MISSING_EXECUTABLE"), "{line}");
        assert!(
            !line.contains("TMUX_PANE"),
            "a present requirement must not be reported absent: {line}"
        );
    }

    #[test]
    fn measured_and_unmeasurable_lines_are_distinguishable() {
        // NEGATIVE CONTROL (AGENTS.md rule 8i): if the two look the same the
        // output cannot answer whether the leg ran, which is the hole a silent
        // skip re-creates.
        let measured = measured_line(LEG, &HostRequirement::ALL);
        let skipped = UnmeasurableLeg {
            test: LEG,
            absent: HostRequirement::ALL.to_vec(),
        }
        .render();
        assert_ne!(measured, skipped);
        assert!(measured.starts_with("MEASURED "), "{measured}");
        assert!(skipped.starts_with("UNMEASURABLE "), "{skipped}");
        assert!(measured.contains("skipped=0"), "{measured}");
        assert!(skipped.contains("skipped=1"), "{skipped}");
        assert!(measured.contains("absent=0"), "{measured}");
        assert!(
            skipped.contains(&format!("absent={}", HostRequirement::ALL.len())),
            "{skipped}"
        );
        assert!(
            !measured.contains("UNMEASURABLE"),
            "the measured line must not carry the skip marker: {measured}"
        );
    }

    /// How many DISTINCT values a requirement accessor yields across `ALL`.
    /// Distinctness is the property; the count is derived from `ALL` so it is
    /// never a transcribed number.
    fn collect_distinct(of: fn(HostRequirement) -> &'static str) -> usize {
        let mut seen: Vec<&str> = HostRequirement::ALL.into_iter().map(of).collect();
        seen.sort_unstable();
        seen.dedup();
        seen.len()
    }

    #[test]
    fn every_requirement_carries_a_distinct_remedy() {
        // gate-runner's leg 3, at leg scope: two absences repaired in
        // different places must not share a code, or one repair is dispatched
        // to the wrong machine. Asserted ACROSS `ALL` rather than for one named
        // pair, so a new variant cannot arrive sharing an existing code.
        for (what, rendered) in [
            ("code", collect_distinct(HostRequirement::code)),
            ("remedy", collect_distinct(HostRequirement::remedy)),
            ("label", collect_distinct(HostRequirement::label)),
        ] {
            assert_eq!(
                rendered,
                HostRequirement::ALL.len(),
                "every requirement needs its own {what}"
            );
        }
        for requirement in HostRequirement::ALL {
            assert!(!requirement.label().is_empty());
            assert!(!requirement.code().is_empty());
            assert!(!requirement.remedy().is_empty());
        }
    }

    #[test]
    fn path_resolution_answers_both_ways_against_a_real_directory() {
        // The probe half, measured rather than asserted about: a directory
        // holding an executable resolves, an empty one does not, and a
        // non-executable file of the right name does not either.
        let temp = tempfile::tempdir().expect("path fixture");
        let empty = tempfile::tempdir().expect("empty path fixture");
        let program = temp.path().join("br");
        std::fs::write(&program, b"#!/bin/sh\nexit 0\n").expect("write fixture program");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755))
                .expect("make fixture executable");
        }
        let with = temp.path().display().to_string();
        let without = empty.path().display().to_string();
        assert!(executable_on(&with, "br"), "an executable br must resolve");
        assert!(
            !executable_on(&without, "br"),
            "an empty PATH entry must not resolve br"
        );
        assert!(
            !executable_on(&with, "no-such-program"),
            "resolution must be by name, not by directory"
        );
        assert!(
            !executable_on("", "br"),
            "an empty PATH resolves nothing at all"
        );

        let plain = temp.path().join("bv");
        std::fs::write(&plain, b"not executable\n").expect("write non-executable");
        #[cfg(unix)]
        assert!(
            !executable_on(&with, "bv"),
            "a readable file that is not executable is not a resolvable program"
        );
        let _ = plain;
    }
}
