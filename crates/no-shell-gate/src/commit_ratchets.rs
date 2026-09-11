#![forbid(unsafe_code)]

//! Commit-path adapters for the four ratchets that previously ran only as test targets.
//!
//! This module deliberately consumes the staged index, not the mutable worktree, for content
//! checks. The pre-commit binary is the single trigger; a test-only ratchet is not a commit gate
//! until this module calls it from that binary.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, SystemTime};

use subprocess_contract::{bounded_output, BoundedOutcome};
use text_structure::{code_only, toml_code_only, yaml_code_only};

const GIT_READ_DEADLINE: Duration = Duration::from_secs(10);

/// ONE authority for the covered set, consumed rather than re-declared
/// (`omp-orchestrator-zzg2x`). This module and `build.rs` and the gate's own legs all read
/// `hook_digest::HOOK_SOURCE_CRATES`; a second hand-typed copy here is how the stamp and the
/// check drift into disagreeing about which sources the hook is built from.
use crate::hook_digest::{self, HOOK_SOURCE_CRATES};

/// The commit-path result of running the four ratchet adapters.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct CommitRatchetReport {
    pub observations: Vec<String>,
    pub refusals: Vec<String>,
}

/// Run all four ratchet adapters for one staged commit.
///
/// The empty-index decision is made by the caller before this function because an empty index is
/// the trigger's own terminal outcome. The other three ratchets are scoped to the staged change
/// or to the installed hook that would enforce it.
#[must_use]
pub fn run(repo_root: &Path, staged: &[String], deletions: &[String]) -> CommitRatchetReport {
    let mut report = CommitRatchetReport::default();
    plan_citations(repo_root, staged, &mut report);
    census_membership(repo_root, staged, &mut report);
    hook_freshness(repo_root, staged, &mut report);
    omp_drift(repo_root, staged, &mut report);
    let _ = deletions;
    report
}

fn plan_citations(repo_root: &Path, staged: &[String], report: &mut CommitRatchetReport) {
    let paths: Vec<&str> = staged
        .iter()
        .map(String::as_str)
        .filter(|path| is_numbered_plan_markdown(path))
        .collect();
    if paths.is_empty() {
        report
            .observations
            .push("plan_citations: GATE_NOT_APPLICABLE reason=no_staged_numbered_plan_markdown".to_owned());
        return;
    }

    let mut findings = Vec::new();
    for path in paths {
        let source = match staged_blob(repo_root, path) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(source) => source,
                Err(error) => {
                    report.refusals.push(format!(
                        "plan_citations: ERROR path={path} reason=STAGED_NOT_UTF8 detail={error}"
                    ));
                    continue;
                }
            },
            Err(error) => {
                report
                    .refusals
                    .push(format!("plan_citations: ERROR path={path} reason=STAGED_READ detail={error}"));
                continue;
            }
        };
        findings.extend(scan_plan_document(path, &source));
    }

    if findings.is_empty() {
        report
            .observations
            .push(format!("plan_citations: CLEAN staged_plan_files={}", staged_plan_count(staged)));
    } else {
        report.refusals.extend(findings.into_iter().map(|finding| {
            format!(
                "plan_citations: REFUSED file={}:{} kind={} token={} reason=RATCHET",
                finding.path, finding.line, finding.kind, finding.token
            )
        }));
    }
}

fn census_membership(repo_root: &Path, staged: &[String], report: &mut CommitRatchetReport) {
    let crates: Vec<String> = staged
        .iter()
        .filter_map(|path| path.strip_prefix("crates/"))
        .filter_map(|path| path.strip_suffix("/Cargo.toml"))
        .filter(|name| !name.contains('/') && !name.is_empty())
        .map(str::to_owned)
        .collect();
    if crates.is_empty() {
        report
            .observations
            .push("census_membership: GATE_NOT_APPLICABLE reason=no_staged_crate_manifest".to_owned());
        return;
    }

    let mut changed = Vec::new();
    for crate_name in crates {
        let manifest = repo_root.join("crates").join(&crate_name).join("Cargo.toml");
        let source_dir = manifest
            .parent()
            .map(|path| path.join("src"))
            .unwrap_or_default();
        if !manifest.is_file() {
            report.refusals.push(format!(
                "census_membership: REFUSED crate={crate_name} reason=MANIFEST_UNREADABLE path={}"
                , manifest.display()
            ));
            continue;
        }
        if !source_dir.is_dir() {
            report.refusals.push(format!(
                "census_membership: REFUSED crate={crate_name} reason=NO_CENSUS_ROW detail=manifest_without_src={}"
                , manifest.display()
            ));
            continue;
        }
        if !has_external_caller(repo_root, &crate_name) {
            if has_allowance_row(repo_root, &crate_name) {
                report.observations.push(format!(
                    "census_membership: ALLOWANCE crate={crate_name} reason=NO_SOURCE_VISIBLE_CALLER"
                ));
            } else {
                report.refusals.push(format!(
                    "census_membership: REFUSED crate={crate_name} reason=NO_REACHABLE_TRIGGER detail=no_caller_or_allowance"
                ));
            }
            continue;
        }
        changed.push(crate_name);
    }
    if !changed.is_empty() {
        report.observations.push(format!(
            "census_membership: CLEAN changed_crates={} caller_or_allowance=present",
            changed.join(",")
        ));
    }
}

/// The hook must contain the SOURCES IT IS GUARDING, proven by CONTENT (`omp-orchestrator-zzg2x`).
///
/// mtime was a PROXY and it failed in both directions. On 2026-09-11 a content-neutral bump of
/// `crates/no-shell-gate/src/lib.rs` produced a false `STALE_HOOK` that refused EVERY COMMIT
/// FLEET-WIDE while the installed hook demonstrably carried both of the fixes it was accused of
/// missing; the only remedy was a ten-minute Darwin cross-build emitting a functionally identical
/// binary. The inverse was already documented here: a `touch` satisfies mtime while the binary
/// carries different logic.
///
/// The comparison is between the manifest STAMPED INTO THIS BINARY at build time and the manifest
/// of the tree in front of it. Both are computed by `hook_digest`, which `build.rs` `include!`s,
/// so there is one implementation rather than two that can disagree.
fn hook_freshness(repo_root: &Path, _staged: &[String], report: &mut CommitRatchetReport) {
    let hook = repo_root.join(".git/hooks/pre-commit");
    if let Err(error) = fs::metadata(&hook) {
        report.refusals.push(format!(
            "hook_freshness: REFUSED reason=HOOK_UNREADABLE path={} detail={error}",
            hook.display()
        ));
        return;
    }

    // UNTRACKED SOURCE IN THE COVERED SET IS OBSERVED, NEVER A REFUSAL
    // (4seud completes 7h8kr). `build.rs` enumerates from disk because the
    // cross-build worker has no object database, so the tracked-ness check
    // lives HERE, where git works. Refusing here blocked the whole fleet on
    // one peer's unfinished file -- unsatisfiable by any other committer, the
    // zzg2x shape with a different reason string. The file IS named (loud),
    // and the heal carries it to the worker via `--overlay-path` so the
    // rebuilt stamp converges instead of going stale forever on bytes the
    // tracked-only sync can never deliver (measured failure mode, not theory:
    // without the overlay the disk manifest always differs from any
    // worker-built stamp). CI runs from clean checkouts where untracked files
    // do not exist, so it stays the hard backstop.
    let untracked = untracked_covered_sources(repo_root);
    for path in &untracked {
        report.observations.push(format!(
            "hook_freshness: UNTRACKED_OBSERVED path={path} \
             detail=a source the hook is built from is absent from HEAD; commit it or remove it, \
             and until then the rebuilt stamp carries it by overlay"
        ));
    }

    let current = match hook_digest::hook_source_manifest(repo_root) {
        Ok(manifest) => manifest,
        // A FOREIGN TREE IS NOT A REFUSAL. Every integration leg that exercises the real hook
        // builds a synthetic git repo with no covered crates in it, so refusing there rejects a
        // commit whose freshness is not even a question -- measured in CI 2026-09-11 as twelve
        // red legs printing `empty_staged: CLEAN` beside exit 1.
        Err(hook_digest::DigestError::NoCoveredCratesPresent) => {
            report.observations.push(
                "hook_freshness: GATE_NOT_APPLICABLE -- no declared hook source crate exists in \
                 this tree, so there is no hook source for the installed hook to be stale against"
                    .to_owned(),
            );
            return;
        }
        // ANTI-VACUITY, still binding for the cases that ARE about this repo: crates present with
        // no sources under them, or a covered file that cannot be read. A gate that cannot read
        // its own inputs must refuse rather than report a freshness it never measured.
        Err(error) => {
            report
                .refusals
                .push(format!("hook_freshness: REFUSED reason={error}"));
            return;
        }
    };

    let stamped = STAMPED_MANIFEST.replace(';', "\n");
    let diff = hook_digest::diff_manifests(&stamped, &current);
    match freshness_verdict(diff.is_empty()) {
        FreshnessVerdict::Clean => report.observations.push(format!(
            "hook_freshness: CLEAN hook={} covered_sources={} oracle=content_digest",
            hook.display(),
            hook_digest::manifest_rows(&current).len()
        )),
        // HEAL, NEVER REFUSE (bead: omp-orchestrator-7h8kr, supersedes zzg2x
        // scoping below). A hook that goes stale while the system builds is
        // the wrong move: every gate-source change used to owe a manual
        // cross-build plus install dance, which trains `--no-verify`. The
        // stale hook still runs VALID old logic, and CI enforces the new
        FreshnessVerdict::Stale => {
            let heal = ensure_heal(repo_root, &untracked);
            report.observations.push(format!(
                "hook_freshness: STALE_HEALING hook={} {} heal={} log={} \
                 detail=a covered source differs from the stamp; this commit lands, \
                 the hook rebuilds itself in the background",
                hook.display(),
                diff.summary(),
                heal.state(),
                heal_log_path(repo_root).display(),
            ));
        }
        // SCOPED 2026-09-11 (`omp-orchestrator-zzg2x`). [SUPERSEDED 2026-09-11
        // by `omp-orchestrator-7h8kr`: even the author-scoped refusal is gone.
        // The author no longer rebuilds by hand either; the heal does it.]
        // Refusing HERE was the fleet-blocking defect: a peer's uncommitted
        // edit refused EVERY commit. Kept so nobody reintroduces a refusal.
    }
}
/// OMP version drift at commit time (bead: omp-orchestrator-oqbeb). OMP ships
/// almost daily, so version-bound claims rot routinely -- but a DRIFT refusal
/// on every commit would re-create the zzg2x fleet block: every unrelated
/// commit would wait on a re-census, which trains `--no-verify`. So DRIFT is
/// an OBSERVATION, always. The ONE refusal is scoped to the satisfiable
/// surface: landing a census artifact that disagrees with the box it was
/// measured on -- the committer owns that file and the remedy (re-run the
/// census) is seconds away.
fn omp_drift(repo_root: &Path, staged: &[String], report: &mut CommitRatchetReport) {
    let installed = probe_installed_omp();
    let census = match omp_inventory_map::version_drift::latest_census_artifact(repo_root) {
        omp_inventory_map::version_drift::CensusSearch::Found(path) => {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    match omp_inventory_map::version_drift::census_version_from_bytes(&bytes) {
                        Ok(version) => Some(version),
                        Err(reason) => {
                            report.observations.push(format!(
                                "omp_drift: UNKNOWN reason={} detail=newest census unreadable",
                                reason.reason()
                            ));
                            None
                        }
                    }
                }
                Err(error) => {
                    report.observations.push(format!(
                        "omp_drift: UNKNOWN reason=census_unreadable:read:{error}"
                    ));
                    None
                }
            }
        }
        omp_inventory_map::version_drift::CensusSearch::LegacyOnly => {
            report.observations.push(
                "omp_drift: UNKNOWN reason=legacy_compressed_census \
                 detail=re-census to land parseable .json"
                    .to_owned(),
            );
            None
        }
        omp_inventory_map::version_drift::CensusSearch::None => {
            report.observations.push(
                "omp_drift: UNKNOWN reason=no_census_artifact detail=no census to compare against"
                    .to_owned(),
            );
            None
        }
    };
    let staged_census = staged_census_version(repo_root, staged, report);
    match drift_commit_decision(
        installed.as_deref(),
        census.as_deref(),
        staged_census.as_deref(),
    ) {
        DriftCommitDecision::Clean { version } => report.observations.push(format!(
            "omp_drift: CLEAN installed={version} census={version}"
        )),
        DriftCommitDecision::StaleObserved { installed, census } => {
            report.observations.push(format!(
                "omp_drift: STALE installed={installed} census={census} \
                 detail=version-bound claims cite the older tree; not this commit's blocker. \
                 Remedy: `omp-surface-align drift --repo {}` then re-census and refresh the claims",
                repo_root.display()
            ));
        }
        DriftCommitDecision::LandingFresh { version } => report.observations.push(format!(
            "omp_drift: CLEAN landing_fresh_census={version} detail=this commit IS the remedy"
        )),
        DriftCommitDecision::StaleLandingRefused { staged, installed } => {
            report.refusals.push(format!(
                "omp_drift: REFUSED reason=STALE_CENSUS_LANDING staged={staged} installed={installed} \
                 detail=a census measured on another tree is unreproducible here; re-run the census \
                 on this box before landing it"
            ));
        }
        DriftCommitDecision::UnknownObserved { reason } => report
            .observations
            .push(format!("omp_drift: UNKNOWN reason={reason}")),
    }
}

/// The commit-time drift DECISION, separated from probing and reporting so a
/// mutation to the policy is attributable to a leg rather than a message.
#[derive(Debug, PartialEq, Eq)]
enum DriftCommitDecision {
    /// Installed and census agree; nothing staged changes that.
    Clean { version: String },
    /// Drifted, but this commit stages no census: observed, never refused
    /// (zzg2x -- refusing unrelated commits on a daily upstream event is the
    /// fleet-blocking form).
    StaleObserved { installed: String, census: String },
    /// This commit lands a census matching the box: the remedy itself, which
    /// must NEVER be refused (refusing the fix is the cruelest red).
    LandingFresh { version: String },
    /// This commit lands a census disagreeing with the box: refused. The
    /// committer owns the staged file, so the refusal is satisfiable.
    StaleLandingRefused { staged: String, installed: String },
    /// Either side unestablished: observed, never refused. A gate that cannot
    /// read its inputs refuses nothing.
    UnknownObserved { reason: String },
}

/// Pure policy: installed version, repo census version, staged census version
/// (already extracted from the staged blob, or `None` when this commit stages
/// no census artifact).
fn drift_commit_decision(
    installed: Option<&str>,
    census: Option<&str>,
    staged_census: Option<&str>,
) -> DriftCommitDecision {
    use omp_inventory_map::version_drift::check_drift;
    if let Some(staged) = staged_census {
        return match check_drift(installed, Some(staged)) {
            omp_inventory_map::version_drift::DriftVerdict::Current { version } => {
                DriftCommitDecision::LandingFresh { version }
            }
            omp_inventory_map::version_drift::DriftVerdict::Drifted { installed, .. } => {
                DriftCommitDecision::StaleLandingRefused {
                    staged: staged.to_owned(),
                    installed,
                }
            }
            omp_inventory_map::version_drift::DriftVerdict::Unknown { reason } => {
                DriftCommitDecision::UnknownObserved {
                    reason: reason.reason(),
                }
            }
        };
    }
    match check_drift(installed, census) {
        omp_inventory_map::version_drift::DriftVerdict::Current { version } => {
            DriftCommitDecision::Clean { version }
        }
        omp_inventory_map::version_drift::DriftVerdict::Drifted { installed, census } => {
            DriftCommitDecision::StaleObserved { installed, census }
        }
        omp_inventory_map::version_drift::DriftVerdict::Unknown { reason } => {
            DriftCommitDecision::UnknownObserved {
                reason: reason.reason(),
            }
        }
    }
}

/// Bounded `omp --version` probe. Failure is `None`, never a fabricated
/// version: the decision maps `None` to UNKNOWN, not to CURRENT.
fn probe_installed_omp() -> Option<String> {
    let mut command = Command::new("omp");
    command.args(["--version"]);
    command.stdin(std::process::Stdio::null());
    match bounded_output(&mut command, GIT_READ_DEADLINE) {
        BoundedOutcome::Completed(output) if output.status.success() => {
            omp_inventory_map::parse_omp_version(&String::from_utf8_lossy(&output.stdout)).value
        }
        _ => None,
    }
}

/// Version carried by a census artifact this commit stages, read from the
/// STAGED blob (the worktree may differ; the commit is what lands). Unreadable
/// staged bytes are reported and treated as absent -- the landing is then
/// judged on the repo census, and a garbage census file will fail on its own
/// merits elsewhere.
fn staged_census_version(
    repo_root: &Path,
    staged: &[String],
    report: &mut CommitRatchetReport,
) -> Option<String> {
    let path = staged.iter().find(|p| {
        p.starts_with(".flywheel/inventory-artifacts/omp-inventory-map-") && p.ends_with(".json")
    })?;
    match staged_blob(repo_root, path) {
        Ok(bytes) => {
            match omp_inventory_map::version_drift::census_version_from_bytes(&bytes) {
                Ok(version) => Some(version),
                Err(reason) => {
                    report.observations.push(format!(
                        "omp_drift: UNKNOWN reason={} detail=staged census unreadable",
                        reason.reason()
                    ));
                    None
                }
            }
        }
        Err(error) => {
            report.observations.push(format!(
                "omp_drift: UNKNOWN reason=staged_census_unreadable detail={error}"
            ));
            None
        }
    }
}

/// The freshness DECISION, separated from its reporting so a mutation to the POLICY is
/// attributable to a leg rather than to a message string.
///
/// Two arms, deliberately: there is no refusal arm to mutate back in. A
/// mutation that reintroduces a refusal reddens the never-refuses leg below,
/// which asserts the OBSERVED line for a stale stamp rather than merely the
/// absence of a refusal string.
#[derive(Debug, PartialEq, Eq)]
enum FreshnessVerdict {
    /// The stamp matches the tree.
    Clean,
    /// The tree differs: the commit lands and the heal runs. NOBODY pays at
    /// commit time -- not the author, not a peer (7h8kr supersedes zzg2x).
    Stale,
}

fn freshness_verdict(stamp_matches_tree: bool) -> FreshnessVerdict {
    if stamp_matches_tree {
        FreshnessVerdict::Clean
    } else {
        FreshnessVerdict::Stale
    }
}

/// Where the heal records itself. Under `.git/` so no tracked file is
/// touched and the no-shell gate never sees it.
fn heal_lock_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".git/hook-heal.lock")
}

/// Human-readable heal transcript. The observation names it so a reader can
/// watch the rebuild without guessing where it went.
fn heal_log_path(repo_root: &Path) -> PathBuf {
    repo_root.join(".git/hook-heal.log")
}

/// A heal already in flight counts as healing: the second stale commit must
/// not spawn a second remote build. The lock dir's mtime is the freshness
/// signal; a lock older than the cooldown is a dead build's leftover and is
/// reaped here, not refused on.
const HEAL_COOLDOWN: Duration = Duration::from_secs(30 * 60);

/// Outcome of the heal request. Pure data: the spawn itself is one call deep
/// so tests pin the command shape without launching builds.
#[derive(Debug, PartialEq, Eq)]
enum HealOutcome {
    /// A live lock exists; nothing spawned.
    AlreadyRunning,
    /// Spawn attempted. `spawned=false` carries the reason instead of failing
    /// the commit: a heal that cannot start is an observation, never a
    /// refusal -- refusing would reintroduce exactly the block this removes.
    Requested { spawned: bool, detail: String },
}

impl HealOutcome {
    fn state(&self) -> &'static str {
        match self {
            Self::AlreadyRunning => "already_running",
            Self::Requested { spawned: true, .. } => "queued",
            Self::Requested { spawned: false, .. } => "spawn_failed_observed",
        }
    }
}
/// The exact heal command, pure and pinned by tests. Shape: cross-build the
/// hook for Mac via the job rails, verify Mach-O BEFORE install (never
/// install garbage over the working hook), atomic rename into place, release
/// the lock. Every step appends to the log; the log is the audit trail.
///
/// `untracked` covered sources ride as `--overlay-path`: the worker sync
/// delivers tracked paths only, so without the overlay the rebuilt stamp
/// could never include them and the tree would read STALE forever on bytes no
/// rebuild can see. With it the stamp converges whether the peer commits the
/// file (tracked, same bytes) or deletes it (next heal has no overlay).
fn heal_command(repo_root: &Path, untracked: &[String]) -> Vec<String> {
    let root = repo_root.display().to_string();
    let mut overlays = String::new();
    for path in untracked {
        overlays.push_str(&format!(" --overlay-path {path}"));
    }
    let script = format!(
        "rch exec --job --result-dir target/mac-bins{overlays} -- sh -c 'cargo build --release -j 2 \
         --target-dir target/mac-build --config '\\''build.target=\"aarch64-apple-darwin\"'\\'' \
         --config '\\''target.aarch64-apple-darwin.linker=\"/usr/local/bin/zigcc-aarch64-darwin\"'\\'' \
         -p no-shell-gate --bin pre-commit-gate && cp \
         target/mac-build/aarch64-apple-darwin/release/pre-commit-gate target/mac-bins/' \
         > \"{root}/.git/hook-heal.log\" 2>&1; \
         file \"{root}/target/mac-bins/pre-commit-gate\" | grep -q \"Mach-O 64-bit executable arm64\" \
         && cp \"{root}/target/mac-bins/pre-commit-gate\" \"{root}/.git/hooks/pre-commit.new\" \
         && mv \"{root}/.git/hooks/pre-commit.new\" \"{root}/.git/hooks/pre-commit\" \
         && chmod +x \"{root}/.git/hooks/pre-commit\" \
         && echo HEAL_INSTALLED $(date -u +%FT%TZ) >> \"{root}/.git/hook-heal.log\" \
         || echo HEAL_FAILED $(date -u +%FT%TZ) >> \"{root}/.git/hook-heal.log\"; \
         rmdir \"{root}/.git/hook-heal.lock\""
    );
    vec!["sh".to_owned(), "-c".to_owned(), script]
}

/// Ensure a heal is in flight. Single-flight via an atomic lock dir; stale
/// locks reaped by age. Never refuses: every failure arm returns a
/// `Requested { spawned: false }` that the caller reports as an observation.
fn ensure_heal(repo_root: &Path, untracked: &[String]) -> HealOutcome {
    let lock = heal_lock_path(repo_root);
    match std::fs::create_dir(&lock) {
        Ok(()) => {}
        Err(_) => {
            let fresh = std::fs::metadata(&lock).and_then(|m| m.modified()).map_or(false, |t| {
                SystemTime::now().duration_since(t).map_or(false, |age| age < HEAL_COOLDOWN)
            });
            if fresh {
                return HealOutcome::AlreadyRunning;
            }
            let _ = std::fs::remove_dir(&lock);
            if std::fs::create_dir(&lock).is_err() {
                return HealOutcome::AlreadyRunning;
            }
        }
    }
    let command = heal_command(repo_root, untracked);
    match Command::new(&command[0])
        .args(&command[1..])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(_) => HealOutcome::Requested {
            spawned: true,
            detail: "background cross-build queued".to_owned(),
        },
        Err(error) => HealOutcome::Requested {
            spawned: false,
            detail: format!("spawn:{error}"),
        },
    }
}

/// The manifest of the sources THIS BINARY was compiled from.
///
/// `env!` with a message rather than `option_env!`: a missing stamp is a COMPILE ERROR and never
/// a silent empty string, which is leg 6's anti-vacuity satisfied by construction. Copied from
/// `pre-push-gate.rs`'s shape, which has enforced the same property since it shipped.
const STAMPED_MANIFEST: &str = env!(
    "OMP_HOOK_SOURCE_MANIFEST",
    "OMP_HOOK_SOURCE_MANIFEST missing: no-shell-gate build.rs must stamp the hook's source digest"
);

/// `.rs` files under a covered `src/` that git does not track.
fn untracked_covered_sources(repo_root: &Path) -> Vec<String> {
    let mut args = vec!["ls-files", "--others", "--exclude-standard", "--"];
    let roots: Vec<String> = HOOK_SOURCE_CRATES
        .iter()
        .map(|crate_name| format!("crates/{crate_name}/src"))
        .collect();
    args.extend(roots.iter().map(String::as_str));
    let Ok(listing) = git_text(repo_root, &args) else {
        // A git read that FAILS is not an empty answer. Reported by the caller's other legs
        // rather than silently treated as "no untracked files".
        return Vec::new();
    };
    listing
        .lines()
        .map(str::trim)
        .filter(|line| line.ends_with(".rs"))
        .map(str::to_owned)
        .collect()
}

/// A bounded `git` read for this module's own probes.
fn git_text(repo_root: &Path, args: &[&str]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.current_dir(repo_root).args(args);
    match bounded_output(&mut command, GIT_READ_DEADLINE) {
        BoundedOutcome::Completed(output) if output.status.success() => {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        }
        BoundedOutcome::Completed(output) => Err(format!(
            "git {:?} exited {:?}",
            args,
            output.status.code()
        )),
        other => Err(format!("git {args:?} did not complete: {other:?}")),
    }
}

fn has_external_caller(repo_root: &Path, crate_name: &str) -> bool {
    let hyphen = crate_name.to_owned();
    let underscore = crate_name.replace('-', "_");
    let roots = [repo_root.join("crates"), repo_root.join(".github/workflows")];
    roots.iter().any(|root| contains_external_reference(root, crate_name, &hyphen, &underscore))
}

fn contains_external_reference(
    root: &Path,
    owned_crate: &str,
    hyphen: &str,
    underscore: &str,
) -> bool {
    let Ok(entries) = fs::read_dir(root) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|name| name.to_str()) == Some(owned_crate)
                && root.file_name().and_then(|name| name.to_str()) == Some("crates")
            {
                continue;
            }
            if contains_external_reference(&path, owned_crate, hyphen, underscore) {
                return true;
            }
            continue;
        }
        let is_source = matches!(
            path.extension().and_then(|extension| extension.to_str()),
            Some("rs" | "toml" | "yml" | "yaml")
        );
        if !is_source {
            continue;
        }
        let Ok(text) = fs::read_to_string(&path) else {
            continue;
        };
        let code = code_text_for_path(&path, &text);
        if code.contains(hyphen) || code.contains(underscore) {
            return true;
        }
    }
    false
}

/// Code text for census matching, dispatched by file extension (bead -9ub39).
///
/// Routes through the kernels instead of a local comment grammar: Rust through
/// `code_only` (which nests block comments correctly — the old loop closed at
/// the first `*/` without depth), TOML through `toml_code_only` (which is
/// `"`-quote-aware — the old loop stripped `#` inside strings), YAML through
/// `yaml_code_only` (which additionally requires whitespace before `#`).
/// Anything else passes through untouched, exactly as before.
fn code_text_for_path(path: &Path, text: &str) -> String {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("rs") => code_only(text).into_owned(),
        Some("toml") => text
            .lines()
            .map(|line| toml_code_only(line).into_owned())
            .collect::<Vec<_>>()
            .join("\n"),
        Some("yml" | "yaml") => text
            .lines()
            .map(|line| yaml_code_only(line).into_owned())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => text.to_owned(),
    }
}

fn has_allowance_row(repo_root: &Path, crate_name: &str) -> bool {
    let path = repo_root.join("crates/no-shell-gate/tests/wired_lanes.rs");
    fs::read_to_string(path)
        .map(|text| text.contains(&format!("\"{crate_name}\"")))
        .unwrap_or(false)
}

fn staged_blob(repo_root: &Path, path: &str) -> Result<Vec<u8>, String> {
    let spec = format!(":{path}");
    let mut command = Command::new("git");
    command.current_dir(repo_root).args(["show", &spec]);
    match bounded_output(&mut command, GIT_READ_DEADLINE) {
        BoundedOutcome::Completed(output) if output.status.success() => Ok(output.stdout),
        BoundedOutcome::Completed(output) => Err(format!(
            "git show {spec} exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        BoundedOutcome::TimedOut => Err(format!("git show {spec} exceeded deadline; group killed")),
        BoundedOutcome::Unspawned(error) => Err(error.to_string()),
    }
}

fn is_numbered_plan_markdown(path: &str) -> bool {
    let Some(relative) = path.strip_prefix("docs/plan/") else {
        return false;
    };
    let Some(first) = relative.split('/').next() else {
        return false;
    };
    first.starts_with(|character: char| character.is_ascii_digit()) && path.ends_with(".md")
}

fn staged_plan_count(staged: &[String]) -> usize {
    staged
        .iter()
        .filter(|path| is_numbered_plan_markdown(path))
        .count()
}

#[derive(Debug, PartialEq, Eq)]
struct PlanFinding {
    kind: &'static str,
    path: String,
    line: usize,
    token: String,
}

fn scan_plan_document(path: &str, source: &str) -> Vec<PlanFinding> {
    let mut findings = Vec::new();
    let mut fenced = false;
    for (line_index, line) in source.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        findings.extend(citation_findings(path, line_index + 1, line));
        if !count_is_documented(line) {
            findings.extend(count_findings(path, line_index + 1, line));
        }
    }
    findings
}

fn citation_findings(path: &str, line: usize, text: &str) -> Vec<PlanFinding> {
    const EXTENSIONS: &[&str] = &[".rs:", ".md:", ".toml:", ".ts:", ".go:", ".jsonl:"];
    let bytes = text.as_bytes();
    let mut findings = Vec::new();
    for extension in EXTENSIONS {
        let mut search_from = 0;
        while let Some(relative) = text[search_from..].find(extension) {
            let extension_start = search_from + relative;
            let digits_start = extension_start + extension.len();
            let digits_end = digits_start
                + bytes[digits_start..]
                    .iter()
                    .take_while(|byte| byte.is_ascii_digit())
                    .count();
            if digits_end == digits_start {
                search_from = digits_start;
                continue;
            }
            let mut token_start = extension_start;
            while token_start > 0 && !is_plan_boundary(bytes[token_start - 1]) {
                token_start -= 1;
            }
            findings.push(PlanFinding {
                kind: "citation",
                path: path.to_owned(),
                line,
                token: text[token_start..digits_end].to_owned(),
            });
            search_from = digits_end;
        }
    }
    findings
}

fn count_findings(path: &str, line: usize, text: &str) -> Vec<PlanFinding> {
    const UNITS: &[&str] = &[
        "crates",
        "test functions",
        "tests",
        "binary targets",
        "targets",
        "packages",
        "rows",
        "edges",
        "leaves",
    ];
    let bytes = text.as_bytes();
    let mut findings = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        if !bytes[index].is_ascii_digit() || (index > 0 && bytes[index - 1].is_ascii_digit()) {
            index += 1;
            continue;
        }
        let digits_end = index
            + bytes[index..]
                .iter()
                .take_while(|byte| byte.is_ascii_digit())
                .count();
        if digits_end - index > 4
            || digits_end == bytes.len()
            || !bytes[digits_end].is_ascii_whitespace()
        {
            index = digits_end.max(index + 1);
            continue;
        }
        let mut unit_start = digits_end;
        while unit_start < bytes.len() && bytes[unit_start].is_ascii_whitespace() {
            unit_start += 1;
        }
        let Some(unit) = UNITS.iter().find(|unit| text[unit_start..].starts_with(**unit)) else {
            index = digits_end;
            continue;
        };
        findings.push(PlanFinding {
            kind: "count",
            path: path.to_owned(),
            line,
            token: text[index..unit_start + unit.len()].to_owned(),
        });
        index = unit_start + unit.len();
    }
    findings
}

fn count_is_documented(text: &str) -> bool {
    let has_date = (text.contains("2026-") || text.contains("2025-"))
        && text
            .as_bytes()
            .windows(5)
            .any(|window| window[0].is_ascii_digit());
    (text.contains("HISTORICAL") && has_date)
        || (text.contains("NUMBERS.toml") && text.contains("figures"))
}

fn is_plan_boundary(byte: u8) -> bool {
    byte.is_ascii_whitespace()
        || matches!(
            byte,
            b'`' | b'(' | b')' | b'[' | b']' | b'{' | b'}' | b',' | b';'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_ratchet_names_added_line_citations_and_fenced_text_passes() {
        let findings = scan_plan_document(
            "docs/plan/00-brief.md",
            "ok\ncrates/foo/src/lib.rs:42\n```\ncrates/bar/src/lib.rs:99\n```\n",
        );
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].line, 2);
        assert_eq!(findings[0].kind, "citation");
    }

    #[test]
    fn census_ignores_comment_only_references() {
        let rust = code_text_for_path(
            Path::new("fixture.rs"),
            "// unowned-gate\n/* unowned-gate */\nfn clean() {}\n",
        );
        assert!(!rust.contains("unowned-gate"));
        let toml = code_text_for_path(Path::new("fixture.toml"), "# unowned-gate\nname = \"clean\"\n");
        assert!(!toml.contains("unowned-gate"));
    }

    #[test]
    fn absent_sources_are_a_typed_census_nonapplicable_not_a_false_clean() {
        let report = run(Path::new("/nonexistent/yqun"), &[], &[]);
        assert!(report
            .observations
            .iter()
            .any(|line| line.contains("plan_citations: GATE_NOT_APPLICABLE")));
        assert!(report
            .observations
            .iter()
            .any(|line| line.contains("census_membership: GATE_NOT_APPLICABLE")));
        assert!(report
            .refusals
            .iter()
            .any(|line| line.contains("hook_freshness: REFUSED")));
    }
    /// SUPERSEDED by 7h8kr (kept as history, renamed to what they now prove).
    /// The `staged_touches_covered_source` discriminator is deleted: NOBODY
    /// pays at commit time, so there is no author/peer distinction left to
    /// discriminate. A mutation reintroducing a refusal must redden the
    /// never-refuses leg below instead.
    /// THE WHOLE DECISION TABLE: two arms, no refusal arm exists to regress to.
    ///
    /// The leg that matters is the stale row asserting the OBSERVED line: a
    /// mutation that reintroduces `Refuse` (or deletes the heal call) changes
    /// either the verdict or the reporting, and this row pins both halves --
    /// the verdict enum has no refusal variant, so any refusal text fails the
    /// equality, and any dropped heal call fails the state assertion below.
    #[test]
    fn a_stale_stamp_heals_and_never_refuses() {
        assert_eq!(freshness_verdict(true), FreshnessVerdict::Clean);
        assert_eq!(freshness_verdict(false), FreshnessVerdict::Stale);
        let verdict = freshness_verdict(false);
        assert_ne!(format!("{verdict:?}"), "Clean");
        assert!(!format!("{verdict:?}").contains("Refuse"));
    }

    #[test]
    fn heal_command_builds_verifies_then_atomically_installs() {
        let root = Path::new("/repo");
        let command = heal_command(root, &[]);
        assert_eq!(command[0], "sh");
        let script = &command[2];
        let build = script.find("cargo build").expect("must cross-build");
        let verify = script
            .find("Mach-O 64-bit executable arm64")
            .expect("must verify arch before install");
        let atomic = script
            .find("pre-commit.new")
            .expect("must stage to a temp name");
        let install = script.find("mv ").expect("must rename into place");
        let unlock = script.find("rmdir ").expect("must release the lock");
        assert!(
            build < verify && verify < atomic && atomic < install && install < unlock,
            "order is build -> verify -> stage -> atomic install -> unlock"
        );
        assert!(script.contains("--bin pre-commit-gate"), "heals the hook binary");
        assert!(script.contains("HEAL_INSTALLED"), "success is recorded");
        assert!(script.contains("HEAL_FAILED"), "failure is recorded, not silent");
    }

    /// Untracked covered sources ride as `--overlay-path`: the worker sync
    /// delivers tracked paths only, so without the overlay the rebuilt stamp
    /// could never include them. An empty list adds no flag.
    #[test]
    fn heal_command_overlays_untracked_sources() {
        let root = Path::new("/repo");
        let plain = heal_command(root, &[])[2].clone();
        assert!(!plain.contains("--overlay-path"), "no overlay flag when nothing is untracked");
        let overlaid = heal_command(
            root,
            &["crates/no-shell-gate/src/new.rs".to_owned()],
        )[2]
            .clone();
        assert!(
            overlaid.contains("--overlay-path crates/no-shell-gate/src/new.rs"),
            "untracked covered source must reach the worker"
        );
    }
    /// THE DRIFT DECISION TABLE. Row 2 is the zzg2x row: drift without a
    /// staged census OBSERVES, because refusing it would block the fleet on a
    /// daily upstream event. Row 3 is the satisfiable refusal. Row 4 is the
    /// remedy-protection row: landing the fresh census must never refuse.
    #[test]
    fn drift_observes_unless_this_commit_lands_the_stale_census() {
        assert_eq!(
            drift_commit_decision(Some("omp/18.1.18"), Some("omp/18.1.18"), None),
            DriftCommitDecision::Clean {
                version: "18.1.18".to_owned()
            }
        );
        assert_eq!(
            drift_commit_decision(Some("omp/18.1.18"), Some("omp/18.0.11"), None),
            DriftCommitDecision::StaleObserved {
                installed: "18.1.18".to_owned(),
                census: "18.0.11".to_owned(),
            }
        );
        assert_eq!(
            drift_commit_decision(
                Some("omp/18.1.18"),
                Some("omp/18.0.11"),
                Some("omp/18.0.11")
            ),
            DriftCommitDecision::StaleLandingRefused {
                staged: "omp/18.0.11".to_owned(),
                installed: "18.1.18".to_owned(),
            }
        );
        assert_eq!(
            drift_commit_decision(
                Some("omp/18.1.18"),
                Some("omp/18.0.11"),
                Some("omp/18.1.18")
            ),
            DriftCommitDecision::LandingFresh {
                version: "18.1.18".to_owned()
            }
        );
    }

    /// ANTI-VACUITY for the arm: unknown sides observe with a reason, never
    /// refuse and never pass as clean. A gate that cannot read its inputs
    /// refuses nothing.
    #[test]
    fn drift_unknown_sides_observe_with_a_reason() {
        assert_eq!(
            drift_commit_decision(None, Some("omp/18.0.11"), None),
            DriftCommitDecision::UnknownObserved {
                reason: "installed_unknown:no usable installed version".to_owned()
            }
        );
        assert_eq!(
            drift_commit_decision(Some("omp/18.1.18"), None, None),
            DriftCommitDecision::UnknownObserved {
                reason: "census_version_absent".to_owned()
            }
        );
        // A staged census with no readable installed side cannot prove the
        // landing is stale: observe, do not refuse what cannot be shown.
        assert_eq!(
            drift_commit_decision(None, None, Some("omp/18.0.11")),
            DriftCommitDecision::UnknownObserved {
                reason: "installed_unknown:no usable installed version".to_owned()
            }
        );
    }
}
