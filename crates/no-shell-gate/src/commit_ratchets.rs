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
fn hook_freshness(repo_root: &Path, staged: &[String], report: &mut CommitRatchetReport) {
    let hook = repo_root.join(".git/hooks/pre-commit");
    if let Err(error) = fs::metadata(&hook) {
        report.refusals.push(format!(
            "hook_freshness: REFUSED reason=HOOK_UNREADABLE path={} detail={error}",
            hook.display()
        ));
        return;
    }

    // UNTRACKED SOURCE IN THE COVERED SET IS LOUD, NEVER BAKED. `build.rs` enumerates from disk
    // because the cross-build worker has no object database (measured: `git ls-tree -r HEAD` ->
    // "fatal: Not a valid object name HEAD", `git ls-files` -> 0 rows), so the tracked-ness check
    // lives HERE, where git works. A file present on disk and absent from HEAD would otherwise be
    // stamped into an artifact no clean clone can reproduce - the E0583 class, aimed at the gate
    // that refuses every commit.
    for path in untracked_covered_sources(repo_root) {
        report.refusals.push(format!(
            "hook_freshness: REFUSED reason=UNTRACKED_COVERED_SOURCE path={path} \
             detail=a source the hook is built from is absent from HEAD, so the stamp would \
             describe a tree no clean checkout can reproduce; commit it or remove it"
        ));
    }

    let current = match hook_digest::hook_source_manifest(repo_root) {
        Ok(manifest) => manifest,
        // ANTI-VACUITY: an unreadable or empty covered set is an ERROR. A gate that cannot read
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
    match freshness_verdict(diff.is_empty(), staged_touches_covered_source(staged)) {
        FreshnessVerdict::Clean => report.observations.push(format!(
            "hook_freshness: CLEAN hook={} covered_sources={} oracle=content_digest",
            hook.display(),
            hook_digest::manifest_rows(&current).len()
        )),
        // The committer is CHANGING the gate, so the rebuild is its own cost and is satisfiable
        // by the party paying it. The refusal NAMES THE FILES AND THE CHEAP REMEDY: a bare
        // "content mismatch" is a red nobody can diagnose, and an undiagnosable red whose implied
        // remedy is a ten-minute cross-build is the shape that gets routed around. CI already
        // builds this binary natively on arm64, proves its arch, and uploads it with a sha256
        // sidecar (`build-hook-macos`), so the remedy is a 5-second DOWNLOAD rather than a build.
        FreshnessVerdict::Refuse => report.refusals.push(format!(
            "hook_freshness: REFUSED reason=HOOK_CONTENT_MISMATCH hook={} {} \
             detail=this commit stages a source the hook is built from; install the hook CI built \
             for this commit -- `gh run download <run> -n pre-commit-gate-macos-arm64` then verify \
             with the .sha256 sidecar and copy over {} -- or restore the sources it was built from",
            hook.display(),
            diff.summary(),
            hook.display()
        )),
        // SCOPED 2026-09-11 (`omp-orchestrator-zzg2x`). Refusing HERE was the fleet-blocking
        // defect, and it was UNSATISFIABLE BY THE COMMITTER: a peer's uncommitted edit to any of
        // the covered sources refused EVERY commit in the repo, including commits touching only
        // documentation. The committer cannot rebuild from a tree it does not own and must not
        // revert a live peer's work, so its only remaining moves were to wait or to reach for
        // `--no-verify`. Measured cost on the day this shipped: FIVE forced Darwin cross-builds
        // and hours of a twelve-agent fleet unable to land anything.
        //
        // A stale hook still runs VALID gate logic; the exposure is exactly the gate change in
        // flight, and the repo-wide sweep under `gate-runner` in CI measures that from a clean
        // checkout where a rebuild is free. This NARROWS the refusal to the surface where it is
        // satisfiable rather than weakening it: the Refuse arm stays strict for the author.
        FreshnessVerdict::StaleUnscoped => report.observations.push(format!(
            "hook_freshness: STALE_UNSCOPED hook={} {} \
             detail=a covered source differs from the stamp and this commit stages none of them; \
             not this committer's blocker. CI enforces the rebuild from a clean checkout",
            hook.display(),
            diff.summary()
        )),
    }
}

/// The freshness DECISION, separated from its reporting so a mutation to the POLICY is
/// attributable to a leg rather than to a message string.
///
/// Testing `staged_touches_covered_source` alone proves the DISCRIMINATOR and not the BRANCH: a
/// mutation that deletes the scoping and refuses unconditionally leaves every predicate leg green.
/// This enum is what makes the fleet-blocking form detectable by a test.
#[derive(Debug, PartialEq, Eq)]
enum FreshnessVerdict {
    /// The stamp matches the tree.
    Clean,
    /// The tree differs AND this commit stages a covered source -- the committer owns the rebuild.
    Refuse,
    /// The tree differs and this commit stages none of it -- not this committer's blocker.
    StaleUnscoped,
}

fn freshness_verdict(stamp_matches_tree: bool, commit_changes_the_gate: bool) -> FreshnessVerdict {
    if stamp_matches_tree {
        FreshnessVerdict::Clean
    } else if commit_changes_the_gate {
        FreshnessVerdict::Refuse
    } else {
        FreshnessVerdict::StaleUnscoped
    }
}

/// Does the staged set include a source the hook is built from?
///
/// The discriminator between "you are changing the gate" and "someone else left the tree dirty".
/// Prefix match on `crates/<covered>/src/` rather than the stamped manifest, so a NEWLY ADDED
/// covered source counts before it has ever been stamped.
///
/// `/src/` is load-bearing and my first version omitted it, writing `crates/<covered>/` while this
/// comment already said `src`. `crates/no-shell-gate/tests/gate.rs` therefore counted as a build
/// input, which would have forced a cross-build to commit a TEST -- the fleet-blocking cost this
/// function exists to remove, reintroduced on a narrower path. Caught by the known-good leg below,
/// not by review: `hook_digest::covered_files` collects from `crates/<name>/src` ONLY, so any
/// wider predicate here claims the binary was built from bytes it never saw.
fn staged_touches_covered_source(staged: &[String]) -> bool {
    HOOK_SOURCE_CRATES.iter().any(|crate_name| {
        let root = format!("crates/{crate_name}/src/");
        staged
            .iter()
            .any(|path| path.starts_with(&root) && path.ends_with(".rs"))
    })
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
    /// KNOWN-BAD: a commit that STAGES a covered source must be seen as a gate change.
    ///
    /// The mutation that reverts the scoping (dropping the `staged_touches_covered_source` arm and
    /// refusing unconditionally) leaves this leg GREEN, which is why the known-good leg below is
    /// mandatory rather than decorative -- this one alone cannot detect the fleet-blocking form.
    #[test]
    fn staging_a_covered_source_is_a_gate_change() {
        for path in [
            "crates/no-shell-gate/src/commit_ratchets.rs",
            "crates/path-literal-guard/src/lib.rs",
            "crates/undrained-pipe-lint/src/main.rs",
            "crates/no-shell-gate/src/bin/pre-commit-gate.rs",
        ] {
            assert!(
                staged_touches_covered_source(&[path.to_owned()]),
                "{path} is a source the hook is built from and must make its committer own the rebuild"
            );
        }
    }

    /// KNOWN-GOOD, and the leg the whole change exists for.
    ///
    /// Refusing these was UNSATISFIABLE BY THE COMMITTER: a peer's uncommitted edit to a covered
    /// source refused every commit in the repo, including documentation-only ones. Measured
    /// 2026-09-11 at five forced Darwin cross-builds in one day.
    ///
    /// `.md` under a covered crate is included deliberately: the discriminator is `.rs` under the
    /// crate, not the crate name, because a doc edit inside `crates/no-shell-gate/` cannot change
    /// what the binary was compiled from.
    #[test]
    fn an_unrelated_commit_never_inherits_a_peers_stale_gate() {
        for path in [
            "docs/skills/mutation-proof.md",
            "AGENTS.md",
            ".beads/issues.jsonl",
            "crates/tick-monitor/src/main.rs",
            "crates/no-shell-gate/README.md",
            "crates/no-shell-gate/tests/gate.rs",
        ] {
            assert!(
                !staged_touches_covered_source(&[path.to_owned()]),
                "{path} does not change what the hook was built from, so its committer must not \
                 inherit another pane's stale gate"
            );
        }
    }

    /// ANTI-VACUITY: an empty staged set is not a gate change, and the predicate must SAY so
    /// rather than answering by falling off the end of an iterator nobody entered.
    #[test]
    fn an_empty_staged_set_is_not_a_gate_change() {
        assert!(!staged_touches_covered_source(&[]));
        // POSITIVE CONTROL in the same shape: the predicate can still return true here.
        assert!(staged_touches_covered_source(&[
            "docs/skills/mutation-proof.md".to_owned(),
            "crates/state-wildcard-lint/src/lib.rs".to_owned(),
        ]));
    }

    /// THE WHOLE DECISION TABLE, all four inputs, because the branch is the claim.
    ///
    /// Row 3 is the one the change exists for and the one the reverting mutation reddens: a stale
    /// tree that this commit did not cause is NOT this committer's refusal. Row 2 proves the
    /// scoping did not weaken the author's obligation.
    #[test]
    fn a_stale_gate_refuses_only_the_commit_that_changes_it() {
        assert_eq!(freshness_verdict(true, false), FreshnessVerdict::Clean);
        assert_eq!(freshness_verdict(true, true), FreshnessVerdict::Clean);
        assert_eq!(freshness_verdict(false, true), FreshnessVerdict::Refuse);
        assert_eq!(
            freshness_verdict(false, false),
            FreshnessVerdict::StaleUnscoped,
            "a peer's uncommitted gate edit must not refuse an unrelated commit -- refusing here \
             is unsatisfiable by the committer and blocked the whole fleet on 2026-09-11"
        );
    }
}
