#![forbid(unsafe_code)]

//! installer — one-touch install with four-way identity proof.
//!
//! main wires the subprocess calls (git, cargo) to the lib's identity check.
//! Single writer per file: SilverWolf owns main.rs; pane 1 owns lib.rs.

use installer::RepoOwnership;
use lifecycle_event::{EmitOutcome, Layer};
use std::path::PathBuf;
use std::process::ExitCode;
#[used]
static BUILD_ID_MARKER: &[u8] = concat!("build_id=", env!("OMP_BUILD_ID")).as_bytes();
// The roster moved to `installer::OWNED_BINARIES`. It was a private const HERE, which
// is how it drifted unobserved for six days: no test could reach it, so nothing
// noticed that the installed artifact's baked copy still named `omp-orchestrator`
// while the shipped flagship had been renamed to `ompo`. See the doc comment there.

fn main() -> ExitCode {
    let raw_args: Vec<String> = std::env::args().skip(1).collect();
    let ParsedArgs {
        positional: args,
        bin_dir,
        expected_sha256,
        pane,
        incarnation,
    } = match parse_cli_args(raw_args) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("INSTALLER ERROR: {error}");
            return ExitCode::from(2);
        }
    };
    let repo_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crate lives two levels below repo root")
        .to_path_buf();
    // One attempt identity per operator invocation, shared by every event
    // this run emits. Pane/incarnation arrive explicitly via flags (empty
    // means unknown, never fabricated); the attempt token is minted here.
    let identity = installer::AttemptIdentity {
        pane,
        incarnation,
        attempt: installer::mint_attempt_id(),
    };
    match args.first().map(String::as_str) {
        Some("--check") if args.len() == 1 => run_check(&repo_root, &bin_dir, &identity),
        Some("--install") if args.len() == 2 => run_install(
            &repo_root,
            &bin_dir,
            &args[1],
            expected_sha256.as_deref(),
            &identity,
        ),
        Some("--install") => {
            eprintln!("INSTALLER ERROR: --install requires exactly one target");
            usage();
            ExitCode::from(2)
        }
        Some("--version") => {
            println!("installer 0.1.0 build_id={}", env!("OMP_BUILD_ID"));
            ExitCode::SUCCESS
        }
        Some("-h") | Some("--help") => {
            usage();
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("installer: unknown verb {other:?}");
            usage();
            ExitCode::from(2)
        }
        None => run_check(&repo_root, &bin_dir, &identity),
    }
}

struct ParsedArgs {
    positional: Vec<String>,
    bin_dir: PathBuf,
    expected_sha256: Option<String>,
    pane: String,
    incarnation: String,
}

fn parse_cli_args(raw_args: Vec<String>) -> Result<ParsedArgs, String> {
    let mut positional = Vec::new();
    let mut explicit_bin_dir = None;
    let mut expected_sha256 = None;
    let mut pane = String::new();
    let mut incarnation = String::new();
    // Repeated digest flags follow --bin-dir: the last occurrence wins.
    let mut args = raw_args.into_iter();
    while let Some(arg) = args.next() {
        if arg == "--bin-dir" {
            let value = args
                .next()
                .ok_or_else(|| "--bin-dir requires a path".to_owned())?;
            if value.is_empty() {
                return Err("--bin-dir requires a non-empty path".to_owned());
            }
            explicit_bin_dir = Some(PathBuf::from(value));
        } else if let Some(value) = arg.strip_prefix("--bin-dir=") {
            if value.is_empty() {
                return Err("--bin-dir requires a non-empty path".to_owned());
            }
            explicit_bin_dir = Some(PathBuf::from(value));
        } else if arg == "--sha256" {
            let value = args
                .next()
                .ok_or_else(|| "--sha256 requires a digest".to_owned())?;
            expected_sha256 = Some(value);
        } else if let Some(value) = arg.strip_prefix("--sha256=") {
            expected_sha256 = Some(value.to_owned());
        } else if arg == "--pane" {
            pane = args
                .next()
                .ok_or_else(|| "--pane requires an id".to_owned())?;
        } else if let Some(value) = arg.strip_prefix("--pane=") {
            pane = value.to_owned();
        } else if arg == "--incarnation" {
            incarnation = args
                .next()
                .ok_or_else(|| "--incarnation requires an id".to_owned())?;
        } else if let Some(value) = arg.strip_prefix("--incarnation=") {
            incarnation = value.to_owned();
        } else {
            positional.push(arg);
        }
    }
    let bin_dir = explicit_bin_dir
        .or_else(|| {
            std::env::var_os("INSTALL_BIN_DIR")
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        })
        .or_else(|| dirs_home().map(|home| home.join(".local/bin")))
        .ok_or_else(|| "INSTALL_BIN_DIR, --bin-dir, or HOME must be set".to_owned())?;
    Ok(ParsedArgs {
        positional,
        bin_dir,
        expected_sha256,
        pane,
        incarnation,
    })
}

fn usage() {
    eprintln!("installer [--check | --install TARGET | --version] [--bin-dir PATH] [--sha256 DIGEST] [--pane ID] [--incarnation ID]");
}
fn dirs_home() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}
fn run_check(
    repo_root: &PathBuf,
    bin_dir: &PathBuf,
    identity: &installer::AttemptIdentity,
) -> ExitCode {
    // Emit-time manifest: FULL state attested; the digest value does not
    // exist yet (verification happens downstream), so the value check
    // lives at report assembly, never here.
    let manifest = installer::InputManifest::Full {
        digest: String::new(),
    };
    let head = match installer::git_head(repo_root) {
        Ok(sha) => sha,
        Err(error) => {
            eprintln!("INSTALLER ERROR: {error}");
            let _ = installer::emit_refusal(
                repo_root,
                Layer::L1,
                "S1.L1",
                "CHECK_GIT_HEAD_REFUSED",
                identity,
                &manifest,
            );
            return ExitCode::from(3);
        }
    };
    let head_short = installer::git_rev_parse_short(repo_root).unwrap_or_default();
    println!("installer --check: HEAD={head_short}");

    // Every counter is the report's, counted explicitly per row and never derived from
    // the roster length: a ratio whose denominator includes rows it never examined is
    // unverifiable — the same defect class as the retired "81 JSON-RPC methods, 17
    // used" figure.
    let report = installer::sweep_installed_identity(
        repo_root,
        bin_dir,
        &head,
        installer::OWNED_BINARIES,
    );
    for row in &report.rows {
        println!("  {row}");
    }

    if report.roster_stale > 0 {
        eprintln!(
            "INSTALLER ROSTER STALE: {}/{} roster entries name a bin target this workspace does not build — the identity proof has a HOLE exactly where those artifacts live",
            report.roster_stale,
            report.rows.len()
        );
    }
    if report.mismatches > 0 {
        eprintln!(
            "INSTALLER IDENTITY DRIFT: {}/{} owned binaries disagree with HEAD {head_short}",
            report.mismatches, report.probed
        );
    }
    if report.unstamped > 0 {
        // NOT folded into the DRIFT line above. The remedy differs: drift says
        // "reinstall from HEAD", and an unstamped artifact would be reinstalled from a
        // source tree that cannot derive a commit either, reproducing the same state
        // while the line claims progress. Same vocabulary `ompo health` already uses
        // for this input (`PROVENANCE_UNSTAMPED`).
        eprintln!(
            "INSTALLER IDENTITY UNSTAMPED: {}/{} owned binaries name no commit on any leg — identity is UNMEASURED, not drifted. Remedy: stamp the build (OMP_BUILD_ID, or a HEAD the build host can resolve), not another reinstall.",
            report.unstamped, report.probed
        );
    }
    if report.foreign > 0 {
        println!(
            "INSTALLER: {} foreign artifact(s) named — excluded from drift denominator",
            report.foreign
        );
    }
    if report.unavailable > 0 {
        println!(
            "INSTALLER: {} artifact(s) have unavailable source ownership — excluded from drift denominator",
            report.unavailable
        );
    }
    if report.not_installed > 0 {
        println!(
            "INSTALLER: {} roster binar(ies) absent from {} — NAMED, not probed; absence is not consistency",
            report.not_installed,
            bin_dir.display()
        );
    }
    if report.drifted() {
        let _ = installer::emit_refusal(repo_root, Layer::L1, "S1.L1", "IDENTITY_DRIFT_REFUSED", identity, &manifest);
        return ExitCode::from(report.exit_code());
    }
    if report.probed == 0 {
        // A 0/0 "IDENTITY OK" is the vacuous green this check shipped with: the one
        // wired invocation (gate.yml -> gate-runner --run -> the declared check in
        // this crate's Cargo.toml) points at an EMPTY scratch directory, so every
        // row was skipped and the gate passed having compared nothing. The roster
        // integrity leg above is what that invocation now actually measures; the
        // identity leg says so instead of claiming a proof it does not have.
        println!(
            "INSTALLER IDENTITY UNPROVEN: 0 of {} roster binaries were probed in {} — no identity was compared. NO-CLAIM: this is not an OK, and roster integrity alone is what passed here.",
            report.rows.len(),
            bin_dir.display()
        );
        return ExitCode::SUCCESS;
    }
    installer::guard_success(installer::emit_s1(repo_root, Layer::L1, "S1.L1", EmitOutcome::Emitted, "IDENTITY_OK", identity, &manifest), &format!(
        "INSTALLER IDENTITY OK: {}/{} binaries consistent with HEAD {head_short}",
        report.probed, report.probed
    ))
}

fn run_install(
    repo_root: &PathBuf,
    bin_dir: &PathBuf,
    target: &str,
    expected_sha256: Option<&str>,
    identity: &installer::AttemptIdentity,
) -> ExitCode {
    // Emit-time manifest: FULL state attested; the digest value does not
    // exist yet (verification happens downstream), so the value check
    // lives at report assembly, never here.
    let manifest = installer::InputManifest::Full {
        digest: String::new(),
    };
    if let Err(error) = installer::check_build_fence(repo_root) {
        eprintln!("INSTALLER BLOCKED: {error}");
        let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_FENCE_BLOCKED", identity, &manifest);
        return ExitCode::from(75);
    }
    let Some((crate_name, binary_name)) = installer::OWNED_BINARIES
        .iter()
        .find(|(_, name)| *name == target)
        .copied()
    else {
        eprintln!("INSTALLER ERROR: unknown target {target:?}; expected one of ompo, tick-monitor, pane-truth, installer, bead-availability");
        let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_UNKNOWN_TARGET", identity, &manifest);
        return ExitCode::from(2);
    };
    let ownership = installer::resolve_repo_ownership(repo_root, crate_name);
    if let RepoOwnership::Foreign { repo } = &ownership {
        eprintln!("INSTALLER ERROR: target {binary_name} is FOREIGN (source in {repo}); install it from its owning repository");
        let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_FOREIGN_TARGET", identity, &manifest);
        return ExitCode::from(3);
    }
    let head = match installer::git_head(repo_root) {
        Ok(sha) => sha,
        Err(error) => {
            eprintln!("INSTALLER ERROR: {error}");
            let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_GIT_HEAD_REFUSED", identity, &manifest);
            return ExitCode::from(3);
        }
    };
    let platform = match installer::current_platform_triple() {
        Ok(platform) => platform,
        Err(error) => {
            eprintln!("INSTALLER PLATFORM REFUSED: {error}");
            let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_PLATFORM_REFUSED", identity, &manifest);
            return ExitCode::from(2);
        }
    };
    println!(
        "INSTALLER PLATFORM: artifact={} fallback={:?}",
        platform.artifact_triple,
        platform.fallback
    );
    let before_start = match installer::running_process_start(binary_name) {
        Ok(start) => start,
        Err(error) => {
            eprintln!("INSTALLER RESTART READ FAILED: {error}");
            let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_RESTART_READ_REFUSED", identity, &manifest);
            return ExitCode::from(2);
        }
    };
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "~/.cargo/bin/cargo".to_owned());
    let cargo = shellexpand_path(&cargo);
    if let Err(error) = installer::build_target(repo_root, &cargo, crate_name, &head) {
        eprintln!("INSTALLER BUILD REFUSED: {error}");
        let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_BUILD_REFUSED", identity, &manifest);
        return ExitCode::from(2);
    }
    // L0-PLATFORM-TRIPLE composition: the build output directory is read as
    // an explicit artifact catalog and the resolved triple selects from it.
    // A musl host takes the gnu artifact only when the catalog proves a
    // nonempty existing file for it; the blind target/release/<binary> join
    // assumed existence instead. main -> run_install -> resolver ->
    // catalog_artifact_dir -> select_fallback_artifact -> install_binary.
    let release_dir = repo_root.join("target/release");
    let catalog = match installer::catalog_artifact_dir(&release_dir, platform.artifact_triple) {
        Ok(catalog) => catalog,
        Err(error) => {
            eprintln!("INSTALLER ERROR: {error}");
            let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_CATALOG_REFUSED", identity, &manifest);
            return ExitCode::from(1);
        }
    };
    let source = match installer::select_fallback_artifact(&platform, &catalog, binary_name) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("INSTALLER ERROR: {error}");
            let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_SELECT_REFUSED", identity, &manifest);
            return ExitCode::from(1);
        }
    };
    // `check` binds here (not inside the Ok arm) because the skills phase
    // and the summary report below both consume the installed identity.
    // First-aid scoping by pane=%49 for an active peer hunk; logic untouched.
    let check = match installer::verify_sha256_before_install(&source, expected_sha256, || {
        installer::install_binary(&source, bin_dir, &head, &ownership)
    }) {
        Ok(check) => {
            println!("  INSTALLED {binary_name}: {check}");
            check
        }
        Err(error) => {
            eprintln!("INSTALLER ERROR: {error}");
            let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_VERIFY_REFUSED", identity, &manifest);
            return ExitCode::from(1);
        }
    };
    match installer::restart_and_verify(
        binary_name,
        &bin_dir.join(binary_name),
        &head,
        before_start,
    ) {
        Ok(outcome) => println!("  RESTART {binary_name}: {outcome}"),
        Err(error) => {
            eprintln!("INSTALLER RESTART FAILED: {error}");
            let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_RESTART_REFUSED", identity, &manifest);
            return ExitCode::from(1);
        }
    }
    // L0-B11: per-family skill installation gates install success. Identity
    // is the installed identity `check` above (the operator entry owns
    // execution); the phase itself executes nothing. Any skills/seal
    // failure refuses with a typed reason; per-family outcomes print on
    // every path so partial progress survives refusal.
    let phase = match installer::skill_install::install_skills_phase(
        repo_root,
        binary_name,
        &head,
        check.clone(),
    ) {
        Ok(phase) => {
            println!(
                "  SKILLS families={} digest={}",
                phase
                    .outcomes
                    .iter()
                    .map(|row| format!("{}={}", row.family, row.outcome))
                    .collect::<Vec<_>>()
                    .join(","),
                phase.digest
            );
            phase
        }
        Err(error) => {
            eprintln!("INSTALLER SKILLS REFUSED: {error}");
            if let installer::InstallError::SkillInstallFailed { outcomes, .. } = &error {
                for row in outcomes {
                    eprintln!("  SKILLS {}={}", row.family, row.outcome);
                }
            }
            let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_SKILLS_REFUSED", identity, &manifest);
            return ExitCode::from(1);
        }
    };
    // L0-B12: durable summary report gates install success. Every input is
    // produced above (skills outcomes/backups, installed identity) or read
    // here (verified digest re-read at report time, read-only PATH scan
    // recorded but never refused). Any assembly failure refuses typed with
    // the per-family outcomes preserved on the refusal path.
    let digest_hex = match installer::verify_sha256(&source, expected_sha256) {
        Ok(hex) => hex,
        Err(error) => {
            eprintln!("INSTALLER REPORT REFUSED: {error}");
            let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_REPORT_REFUSED", identity, &manifest);
            return ExitCode::from(1);
        }
    };
    let path_hits = installer::path_collision_hits(
        binary_name,
        &std::env::var("PATH").unwrap_or_default(),
    );
    let manifest = installer::InputManifest::Full { digest: digest_hex };
    match installer::assemble_install_report(
        repo_root,
        &phase.scan,
        &phase.outcomes,
        &phase.backups,
        &path_hits,
        &check,
        &manifest,
    ) {
        Ok(assembled) => println!(
            "  REPORT digest={} artifact={}",
            assembled.report.digest,
            assembled.artifact.display()
        ),
        Err(error) => {
            eprintln!("INSTALLER REPORT REFUSED: {error}");
            for row in &phase.outcomes {
                eprintln!("  REPORT {}={}", row.family, row.outcome);
            }
            let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_REPORT_REFUSED", identity, &manifest);
            return ExitCode::from(1);
        }
    }
    // L0-B15: the sealed report's event row must be observable, the
    // artifact must re-verify, and the freshness gate must pass before
    // any success verdict. Any stage refusing denies install success
    // with its typed reason; per-family outcomes already printed above
    // survive on this path too.
    match installer::gate_observability(repo_root, &manifest) {
        Ok(gate) => println!(
            "  OBSERVE rows={} fresh={} manifest={manifest}",
            gate.rows, gate.fresh
        ),
        Err(error) => {
            eprintln!("INSTALLER OBSERVE REFUSED: {error}");
            let _ = installer::emit_refusal(repo_root, Layer::L0, "S1.L0", "INSTALL_OBSERVE_REFUSED", identity, &manifest);
            return ExitCode::from(1);
        }
    }
    installer::guard_success(installer::emit_s1(repo_root, Layer::L0, "S1.L0", EmitOutcome::Emitted, "INSTALL_VERIFIED", identity, &manifest), &format!("INSTALLER: target {binary_name} installed and verified"))
}

fn shellexpand_path(path: &str) -> String {
    if path.starts_with("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return format!("{}/{}", PathBuf::from(home).display(), &path[2..]);
        }
    }
    path.to_owned()
}

#[cfg(test)]
#[path = "main_tests.rs"]
mod tests;
