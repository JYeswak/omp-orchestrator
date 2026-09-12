// ONE authority for "which bytes was the installed hook built from" (`omp-orchestrator-zzg2x`).
//
// # Why this file is `include!`d by `build.rs` as well as compiled into the crate
//
// The digest is computed TWICE: at BUILD time, to stamp the artifact, and at RUN time, to
// compare the running hook against the tree it is guarding. Two hand-written copies of one
// hash is the duplicate-authority defect four lanes collapsed on 2026-09-11, and here it
// fails in the WORST direction: two implementations that disagree render a CORRECT hook
// PERMANENTLY STALE, which is a gate red by construction, which gets routed around. So
// `build.rs` does `include!("src/hook_digest.rs")` and there is exactly one implementation.
//
// # Why a digest, and not an mtime, and not a commit id
//
// mtime is a PROXY and it failed in BOTH directions. On 2026-09-11 a content-neutral bump of
// `crates/no-shell-gate/src/lib.rs` produced a false `STALE_HOOK` that refused every commit
// fleet-wide while the installed hook demonstrably carried both of the fixes it was accused
// of missing; the remedy was a ten-minute Darwin cross-build producing a functionally
// identical binary. `commit_ratchets` already documented the inverse: a `touch` satisfies
// mtime while the binary carries different logic.
//
// A commit id (`OMP_BUILD_ID`) is a THIRD proxy and is weaker still: it does not move for an
// UNCOMMITTED byte edit, so a hook built before that edit reads clean against a source it
// does not contain.
//
// # WORKTREE bytes, deliberately, and it is not a strictness increase
//
// The digest reads the bytes ON DISK. The mtime rule it replaces ALREADY reddens on any
// covered-source edit, so the TRUE positive is unchanged; what disappears is the false
// positives - a `touch`, a checkout, a build bumping mtime with no content change. Hashing
// HEAD blobs instead would be STRICTLY WEAKER than what shipped before this commit: a real
// byte edit would run against a hook that does not contain it.
//
// A worktree digest also SELF-HEALS where mtime cannot. Under mtime, editing a covered
// source and then REVERTING it leaves the gate stale, because the revert writes the file and
// moves mtime again - only a rebuild clears it. Under a content digest the revert restores
// the bytes and the gate clears itself.
//
// # NO-CLAIM
//
// This proves the artifact was built from THESE BYTES - the worktree contents of the covered
// source set. It does NOT prove the build used the same toolchain or flags, it does not claim
// the bytes match HEAD (they legitimately differ whenever the tree is dirty), and it says
// nothing about whether the resulting logic is correct.

use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

/// The crates whose `src/` the installed hook is built from. DECLARED, never inferred.
///
/// `commit_ratchets` consumes this rather than declaring a second copy, and
/// `the_ratchet_and_the_digest_cover_the_same_source_set` asserts that they cannot drift.
///
/// ⭐ WIDENED 2026-09-12 (`omp-orchestrator-p3eu5`, DEFECT B) FROM FIVE CRATES / 24 FILES TO
/// NINE / 30, AND THE SELECTION RULE IS INFLUENCE-TIMES-CHURN, NOT LINKAGE.
///
/// The four added crates are the ones for which a commit EXISTS that changed their `src/`
/// alongside a change to the hook's own decision code:
///   `pre-delete-citation-check`  f194a01 — the close-reason gate's INDEX-vs-worktree fix
///   `text-structure`             6fbb5fa — the masking recogniser
///   `subprocess-contract`        4fc649a — and its FAILURE MODE IS ITSELF A VERDICT: a
///                                `bounded_output` timeout returns `GateError::GitFailed`
///                                (`lib.rs:159-170`), so its deadline decides REFUSE vs HANG
///   `doctrine-retirement-gate`   7325930 — changed what the gate refuses
///
/// ⛔ AND FOUR CANDIDATES WERE REFUSED FOR REASONS WORTH KEEPING, because each is the sort of
/// argument the next author will otherwise re-litigate:
///   `omp-inventory-map`   QUALIFIES ON INFLUENCE (c26cd3e) AND FAILS ON ECONOMICS. 11 files at
///       34 commits/7d, which MORE THAN DOUBLES the fleet's heal rate by itself. Revisit below
///       ~5 commits/7d, or cover an isolable subset of its drift API.
///   `staged-build-gate`   DISARMED behind `OMP_STAGED_BUILD_GATE=1` (`pre-commit-gate.rs:616`).
///   `r1-breadth-gate`     DISARMED behind `OMP_R1_BREADTH_GATE=1` (`:228`).
///       Both have REAL verdict-changing commits (41cafbb, 9f9a994) whose verdicts DO NOT FIRE.
///       Compiled in, so a change moves the artifact; disarmed, so it does not move the
///       decision. THEY BECOME WATCHED THE DAY THEY ARE ARMED.
///   `preregistration-gate` UNMEASURED, not exempt-on-merit: its imported symbols ARE used in
///       the hook bin and ZERO co-commits exist. A quiet crate and a dead one look identical
///       over any window.
///
/// EVERY COMMIT TO A COVERED SOURCE IS ONE FLEET-WIDE `STALE_HEALING` EVENT. That is the cost
/// this list is priced in: 24 files / 42 commits-per-7d before, 30 / 58 after. A widening is
/// never free and the denominator is printed on every run as `covered_sources=N`.
pub const HOOK_SOURCE_CRATES: &[&str] = &[
    "no-shell-gate",
    "state-wildcard-lint",
    "path-literal-guard",
    "orchestration-tick-gate",
    "undrained-pipe-lint",
    "pre-delete-citation-check",
    "text-structure",
    "subprocess-contract",
    "doctrine-retirement-gate",
];

/// Path deps LINKED INTO the hook binary that are deliberately NOT covered by the digest, each
/// with the reason it cannot alter hook behaviour.
///
/// ⛔ THIS EXISTS BECAUSE THE HAND LIST ABOVE WAS A SILENT FLOOR. `no-shell-gate` declares many
/// more path deps than the five it watches; editing an unwatched one changes the compiled
/// artifact and leaves the manifest identical, so the gate reports CLEAN about a hook it no
/// longer describes. That is DEFECT B in `omp-orchestrator-tfdki`, and the bead predicted a
/// hand-maintained set would re-acquire it — it has now done so three times, most recently when
/// `omp-inventory-map` was added.
///
/// ⭐ THE REMEDY HERE IS NOT DERIVATION, AND THAT IS A RULING RATHER THAN AN OMISSION. Deriving
/// the covered set from the dependency closure widens it from 24 files to 40+, which changes the
/// STAMP, which makes every installed hook in the fleet report `STALE_HEALING` on its next
/// commit. It also contradicts this bead's own ruling that LINKAGE IS NOT INFLUENCE: most of
/// these are unproven, and watching everything is the over-strict gate this repo calls the
/// slower death. What was actually missing was not coverage, it was SILENCE — so every linked
/// dep must now be watched or listed HERE with a reason, and a new dependency is neither, which
/// fails `every_linked_path_dep_is_watched_or_declared_unwatched` until somebody classifies it.
///
/// NO-CLAIM: these reasons say why each crate is not EXPECTED to alter the hook's decisions.
/// They are arguments from role, not measurements — two crates in this list were measured to
/// influence hook behaviour before (`pre-delete-citation-check` at f194a01, `text-structure` at
/// 6fbb5fa), which is why both are WATCHED rather than listed here. A row that turns out to be
/// wrong belongs in `HOOK_SOURCE_CRATES`, not in a widened exemption.
pub const DELIBERATELY_UNWATCHED: &[(&str, &str)] = &[
    (
        "finding",
        "a typed record format the gate emits INTO; it carries no decision the hook makes, and \
         zero `finding::` references exist anywhere in this crate's src",
    ),
    (
        "crate-atom-gate",
        "DISARMED: the AUTHORITATIVE statement of that is now the typed row in \
         `armed_gates::ARMED_GATES` (gate=\"crate-atom-gate\", var=\"OMP_CRATE_ATOM_GATE\"), \
         which is what the freshness leg keys on since omp-orchestrator-0rfwk; the call site \
         behind that var (pre-commit-gate.rs:632) is the CODE the registry describes, not the \
         record of it, and a source substring is a rendering of a state rather than the state. \
         A default commit never spawns the binary and cannot refuse. ZERO library \
         references (`crate_atom_gate::`) in this crate's src: the hook reaches it ONLY by \
         SPAWNING target/debug/crate-atom-gate or ~/.local/bin/crate-atom-gate \
         (pre-commit-gate.rs:1403-1406, BINARY_ABSENT arm). hook_digest does not stamp that \
         second artifact. A PRESENT AND STALE spawned binary is UNDETECTABLE -- BINARY_ABSENT \
         is a different arm and is not evidence of freshness. When ARMED, refusals.push feeds \
         PreCommitOutcome::Violation so the verdict IS load-bearing; WATCH IT THE DAY IT IS \
         ARMED and then stamp that binary (9yf5s (a) or (b)). omp-orchestrator-9yf5s answer (c) \
         until then",
    ),
    (
        "convergence-stamp",
        "stamps convergence artifacts after a decision is taken, never inside one; zero \
         `convergence_stamp::` references in this crate's src",
    ),
    (
        "preregistration-gate",
        "UNMEASURED, NOT EXEMPT ON MERIT: its imported symbols ARE used in the hook bin, and no \
         commit exists in which its src moved alongside a hook decision change. A quiet crate \
         and a dead one look identical over any window, so this row records ignorance, not \
         innocence -- promote it the moment a co-commit appears",
    ),
    (
        "omp-inventory-map",
        "QUALIFIES ON INFLUENCE (c26cd3e moved commit_ratchets::omp_drift) AND FAILS ON \
         ECONOMICS: 11 src files at 34 commits/7d, which MORE THAN DOUBLES the fleet's \
         STALE_HEALING rate on its own. Revisit below ~5 commits/7d, or cover an isolable \
         subset of its version_drift API",
    ),
    (
        "staged-build-gate",
        "DISARMED: its call site is behind `OMP_STAGED_BUILD_GATE=1` (pre-commit-gate.rs:616), \
         so 41cafbb's real verdict change does not fire on a default commit. Compiled in, so a \
         change moves the artifact; disarmed, so it does not move the decision. WATCH IT THE \
         DAY IT IS ARMED",
    ),
    (
        "r1-breadth-gate",
        "DISARMED: its call site is behind `OMP_R1_BREADTH_GATE=1` (pre-commit-gate.rs:228), so \
         9f9a994's commit-path verdict partition does not fire on a default commit. WATCH IT \
         THE DAY IT IS ARMED",
    ),
    // NO ROWS FOR REGISTRY CRATES OR DEV-DEPENDENCIES, ON PURPOSE. `serde`/`sha2` are registry
    // deps, and `asupersync-conformance`/`omp-orchestrator` appear only under
    // `[dev-dependencies]` — none of them is a `[dependencies]` path dep, so none is linked
    // into the hook binary the way this list is about. A row for a crate that is not a linked
    // path dep is a stale exemption, and
    // `every_linked_path_dep_is_watched_or_declared_unwatched` fails on exactly that.
];

/// Why a digest could not be computed.
///
/// ANTI-VACUITY: an empty covered set is an ERROR, never a clean zero. A hook that cannot find
/// its own inputs must refuse rather than report freshness it did not measure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DigestError {
    /// NONE of the declared crates exist as directories: this is not the repo the hook guards.
    ///
    /// ⛔ THIS IS NOT A REFUSAL AND CONFLATING IT WITH ONE BROKE TWELVE LEGS. Every integration
    /// test that runs the real hook builds a SYNTHETIC git repo, where no covered crate exists --
    /// so an anti-vacuity error there refuses a commit whose freshness is not even a question.
    /// Measured in CI on 2026-09-11: `no-shell-gate` red with `empty_staged: CLEAN staged_files=1`
    /// printed beside exit 1, the refusal coming from a gate about a tree the test never had.
    ///
    /// The discriminator is DIRECTORY PRESENCE: no crates at all means a foreign tree; crates
    /// present with no `.rs` under them means the scan broke, which is the real corruption.
    NoCoveredCratesPresent,
    /// The crates ARE present and yielded no `.rs` at all - the scan broke, or the files moved.
    EmptySourceSet,
    /// A covered file exists and cannot be read. Absence of bytes is not agreement.
    Unreadable { path: String, detail: String },
}

impl std::fmt::Display for DigestError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoCoveredCratesPresent => formatter.write_str(
                "NO_COVERED_CRATES_PRESENT: none of the declared hook source crates exist here, \
                 so this is not the repo the hook guards and its freshness is not a question",
            ),
            Self::EmptySourceSet => formatter.write_str(
                "EMPTY_COVERED_SET: no .rs found under any declared hook source crate; \
                 an empty scan set is an ERROR, not a pass",
            ),
            Self::Unreadable { path, detail } => {
                write!(formatter, "UNREADABLE_COVERED_SOURCE path={path} detail={detail}")
            }
        }
    }
}

/// Every `.rs` under each covered crate's `src/`, SORTED.
///
/// Sorted because a digest over an unsorted directory walk is a function of filesystem
/// iteration order, which differs between the build host and the guarded checkout and would
/// make a correct hook read stale.
pub fn hook_source_files(repo_root: &Path) -> Vec<PathBuf> {
    fn collect(root: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(root) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect(&path, out);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                out.push(path);
            }
        }
    }

    let mut files = Vec::new();
    for crate_name in HOOK_SOURCE_CRATES {
        collect(&repo_root.join("crates").join(crate_name).join("src"), &mut files);
    }
    files.sort();
    files
}

/// The per-file digest manifest: one `<relative path> <sha256>` row per covered source.
///
/// A single whole-set hash can only say MISMATCH. That is a red nobody can diagnose from the
/// message, and the remedy it implies - a Darwin cross-build - is expensive enough that an
/// undiagnosable red is the kind of gate that gets routed around. Per-file rows let the
/// refusal NAME the file that differs, which turns a ten-minute mystery into a one-line
/// decision: rebuild, or ask the peer holding that hunk.
pub fn hook_source_manifest(repo_root: &Path) -> Result<String, DigestError> {
    let files = hook_source_files(repo_root);
    if files.is_empty() {
        // DIRECTORY PRESENCE is the discriminator between "wrong repo" and "broken scan".
        // Checked per crate against `<repo>/crates/<name>/src`, which is exactly the root the
        // collector walks -- asking a different question than the scan would reintroduce the
        // conflation this replaces.
        let any_crate_present = HOOK_SOURCE_CRATES.iter().any(|crate_name| {
            repo_root.join("crates").join(crate_name).join("src").is_dir()
        });
        return Err(if any_crate_present {
            DigestError::EmptySourceSet
        } else {
            DigestError::NoCoveredCratesPresent
        });
    }

    let mut manifest = String::new();
    for path in &files {
        let bytes = fs::read(path).map_err(|error| DigestError::Unreadable {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        // The PATH is recorded with its bytes: MOVING a covered file without editing it is a
        // change to the source set, and a bag-of-hashes cannot see it.
        let relative = path.strip_prefix(repo_root).unwrap_or(path);
        manifest.push_str(&relative.to_string_lossy().replace('\\', "/"));
        manifest.push(' ');
        manifest.push_str(&hex(&hasher.finalize()));
        manifest.push('\n');
    }
    Ok(manifest)
}

/// Lowercase hex, written out rather than relying on a digest type implementing `LowerHex` -
/// sha2 0.11 returns a `hybrid-array` value that does not.
pub fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// The rows of a manifest, as `(path, digest)` pairs. Malformed rows are dropped by the
/// caller's comparison rather than silently treated as matches.
pub fn manifest_rows(manifest: &str) -> Vec<(&str, &str)> {
    manifest
        .lines()
        .filter_map(|line| line.split_once(' '))
        .collect()
}

/// How the running artifact's manifest differs from the tree in front of it.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ManifestDiff {
    /// Present in both, different bytes.
    pub changed: Vec<String>,
    /// In the tree, absent from the artifact's manifest - a source added since the build.
    pub added: Vec<String>,
    /// In the artifact's manifest, absent from the tree - a source deleted or moved.
    pub removed: Vec<String>,
}

impl ManifestDiff {
    pub fn is_empty(&self) -> bool {
        self.changed.is_empty() && self.added.is_empty() && self.removed.is_empty()
    }

    /// A refusal line that names the files, not just the fact of a mismatch.
    pub fn summary(&self) -> String {
        let mut parts = Vec::new();
        if !self.changed.is_empty() {
            parts.push(format!("changed={}", self.changed.join(",")));
        }
        if !self.added.is_empty() {
            parts.push(format!("added={}", self.added.join(",")));
        }
        if !self.removed.is_empty() {
            parts.push(format!("removed={}", self.removed.join(",")));
        }
        parts.join(" ")
    }
}

/// Compare the manifest stamped into the artifact against the manifest of the tree now.
pub fn diff_manifests(stamped: &str, current: &str) -> ManifestDiff {
    let stamped_rows = manifest_rows(stamped);
    let current_rows = manifest_rows(current);
    let mut diff = ManifestDiff::default();

    for (path, digest) in &current_rows {
        match stamped_rows.iter().find(|(other, _)| other == path) {
            Some((_, stamped_digest)) if stamped_digest == digest => {}
            Some(_) => diff.changed.push((*path).to_owned()),
            None => diff.added.push((*path).to_owned()),
        }
    }
    for (path, _) in &stamped_rows {
        if !current_rows.iter().any(|(other, _)| other == path) {
            diff.removed.push((*path).to_owned());
        }
    }
    diff
}
