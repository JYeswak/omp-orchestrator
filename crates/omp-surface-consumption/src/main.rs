#![forbid(unsafe_code)]
//! Print the OMP inbound-RPC consumption table, derived live from the installed bundle.
//!
//! The re-runnable command acceptance 3 asks for IS this binary, and it prints the
//! bundle identity it read beside the table so a reader can tell a stale answer from a
//! current one — which is the whole defect that made AGENTS.md's 42 unverifiable.
//!
//! Exit codes:
//!   0  table printed, at least one method mapped
//!   2  derivation refused (empty enumeration, anchor absent, cluster too small,
//!      bundle unreadable) or ZERO methods mapped, which is either the headline or a
//!      broken scan and must not read as a clean pass

use omp_surface_consumption::{
    build_table, case_sites, cli_probe, derive_command_set, render_table, rpc_probe, DeriveError,
};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

/// Resolve the installed bundle without a path literal.
///
/// **The first version hardcoded an absolute path under one developer's home, and
/// `path-literal-guard` refused the commit — correctly.** That literal is a fact about
/// one machine's checkout, and a binary carrying it audits the BUILD machine rather
/// than the adopter's. The gate caught it on the commit path before it could land,
/// which is the first time tonight a gate stopped me rather than the reverse.
///
/// Order: an explicit `OMP_BUNDLE` override, then `omp` on `PATH` resolved through its
/// symlinks. No fallback literal — an unresolvable bundle is a typed refusal naming
/// both remedies, because guessing a path is how the literal got there.
fn resolve_bundle() -> Result<PathBuf, String> {
    if let Some(explicit) = std::env::var_os("OMP_BUNDLE") {
        return Ok(PathBuf::from(explicit));
    }
    let mut command = Command::new("sh");
    command.args(["-c", "command -v omp"]);
    let launcher = match subprocess_contract::bounded_output(&mut command, Duration::from_secs(15))
    {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_owned()
        }
        _ => {
            return Err(
                "omp is not on PATH; set OMP_BUNDLE to the installed dist/cli.js".to_owned()
            )
        }
    };
    if launcher.is_empty() {
        return Err("`command -v omp` printed nothing; set OMP_BUNDLE".to_owned());
    }
    // The launcher is a symlink into the package; canonicalize reaches the bundle
    // itself, so no part of the path is written down here.
    std::fs::canonicalize(&launcher)
        .map_err(|error| format!("cannot canonicalize {launcher}: {error}; set OMP_BUNDLE"))
}

fn resolve_version() -> Result<String, String> {
    let mut command = Command::new("omp");
    command.arg("--version");
    match subprocess_contract::bounded_output(&mut command, Duration::from_secs(15)) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() =>
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .find(|line| line.starts_with("omp/") && line.len() > 4)
                .map(str::to_owned)
                .ok_or_else(|| "omp --version returned no omp/<version> line".to_owned()),
        subprocess_contract::BoundedOutcome::Completed(output) => Err(format!(
            "omp --version exited {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr).trim()
        )),
        subprocess_contract::BoundedOutcome::TimedOut =>
            Err("omp --version timed out; version is UNMEASURED".to_owned()),
        subprocess_contract::BoundedOutcome::Unspawned(error) =>
            Err(format!("omp --version unavailable: {error}")),
    }
}

fn run_cli_probe(name: &str) -> ExitCode {
    let Some(probe) = cli_probe::CliProbe::parse(name) else {
        eprintln!("OMP_CLI_ERROR reason=unknown_probe name={name} expected=models|stats|usage");
        return ExitCode::from(2);
    };
    match cli_probe::probe_json("omp", probe) {
        Ok(value) => {
            println!("{}", cli_probe::summary(probe, &value));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("OMP_CLI_ERROR reason={error}");
            ExitCode::from(2)
        }
    }
}
fn run_rpc_event_probe() -> ExitCode {
    match rpc_probe::probe() {
        Ok(report) => {
            println!("{}", rpc_probe::summary(&report));
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("OMP_RPC_EVENT_ERROR reason={error}");
            ExitCode::from(2)
        }
    }
}
fn main() -> ExitCode {
    if std::env::args().nth(1).as_deref() == Some("--probe-rpc-events") {
        return run_rpc_event_probe();
    }
    if std::env::args().nth(1).as_deref() == Some("--probe-cli") {
        let Some(name) = std::env::args().nth(2) else {
            eprintln!("OMP_CLI_ERROR reason=missing_probe expected=models|stats|usage");
            return ExitCode::from(2);
        };
        return run_cli_probe(&name);
    }
    let bundle = match resolve_bundle() {
        Ok(bundle) => bundle,
        Err(error) => {
            eprintln!("OMP_SURFACE_ERROR reason=bundle_unresolved detail=\"{error}\" next_action=set-OMP_BUNDLE-or-install-omp");
            return ExitCode::from(4);
        }
    };
    let text = match std::fs::read_to_string(&bundle) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "OMP_SURFACE_ERROR reason=bundle_unreadable path={} error=\"{error}\" next_action=set-OMP_BUNDLE",
                bundle.display()
            );
            return ExitCode::from(2);
        }
    };
    let set = match derive_command_set(&case_sites(&text)) {
        Ok(set) => set,
        Err(error) => {
            eprintln!("OMP_SURFACE_ERROR reason={} next_action=inspect-the-anchor", describe(&error));
            return ExitCode::from(2);
        }
    };
    let version = match resolve_version() {
        Ok(version) => version,
        Err(error) => {
            eprintln!("OMP_SURFACE_ERROR reason=version_unmeasured detail=\"{error}\"");
            return ExitCode::from(2);
        }
    };
    // Bundle identity FIRST. A table without it is a snapshot nobody can date, and
    // this bundle changed version, size, and sha within one day.
    println!(
        "OMP_BUNDLE version={} path={} bytes={} sha256={}",
        version,
        bundle.display(),
        text.len(),
        sha256_hex(text.as_bytes())
    );

    println!(
        "OMP_RPC_COMMAND_SET inbound={} outbound={} seam_gap_bytes={} anchor={}",
        set.inbound.len(),
        set.outbound.len(),
        set.seam_gap,
        omp_surface_consumption::ANCHOR_METHOD
    );

    let repo = match repo_root() {
        Ok(repo) => repo,
        Err(error) => {
            eprintln!("OMP_SURFACE_ERROR reason=repo_root detail=\"{error}\"");
            return ExitCode::from(2);
        }
    };
    let mut consumption: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for method in &set.inbound {
        // The token that ACTUALLY appears in code: the quoted method name. The bead's
        // `omp/` and `Command::new("omp")` greps cannot match it, which is why it
        // recorded ZERO consumption.
        let crates = grep_crates(&repo, &format!("\"{method}\""));
        if !crates.is_empty() {
            consumption.insert(method.clone(), crates);
        }
    }

    let rows = match build_table(&set.inbound, &consumption) {
        Ok(rows) => rows,
        Err(error) => {
            eprintln!("OMP_SURFACE_ERROR reason={}", describe(&error));
            return ExitCode::from(2);
        }
    };
    println!("{}", render_table(&rows));

    if consumption.is_empty() {
        // Acceptance 5: a zero here is either the headline or a broken scan. It is not
        // a pass, and the caller must not be able to read it as one.
        eprintln!(
            "OMP_SURFACE_ERROR reason=ZERO_METHODS_MAPPED next_action=verify-the-grep-token-before-publishing-a-zero -- a scan that finds nothing reads identically to a repo that consumes nothing"
        );
        return ExitCode::from(2);
    }
    ExitCode::SUCCESS
}

fn describe(error: &DeriveError) -> String {
    match error {
        DeriveError::EmptyEnumeration => "EMPTY_ENUMERATION".to_owned(),
        DeriveError::AnchorAbsent { anchor } => format!("ANCHOR_ABSENT anchor={anchor}"),
        DeriveError::ClusterTooSmall { found } => format!("CLUSTER_TOO_SMALL found={found}"),
    }
}

/// Crate directory names whose `src/` carries `needle`.
fn grep_crates(repo: &Path, needle: &str) -> Vec<String> {
    let mut command = Command::new("git");
    command
        .current_dir(repo)
        .args(["grep", "-l", "--no-index", "-F", needle, "--", "crates/*/src/*"]);
    let out = match subprocess_contract::bounded_output(&mut command, Duration::from_secs(60)) {
        subprocess_contract::BoundedOutcome::Completed(output) => output,
        // A refusal here must NOT read as "no consumers": that is the same collapse as
        // an unreadable marker reading as absent.
        _ => return vec!["GREP_UNAVAILABLE".to_owned()],
    };
    // SELF-EXCLUSION, and it is not hygiene. The FIRST live run reported
    // `mapped=12`, of which THREE rows -- `compact`, `get_messages_page`, `login` --
    // had exactly one "consumer": THIS CRATE, matching the method names written in
    // its own doc comments and tests. It also inflated `negotiate_protocol` and
    // `get_state`.
    //
    // Fourth instance of the self-referential-scanner class tonight, and it landed in
    // the file that documents the class. The name comes from the manifest rather than
    // a literal, so a rename cannot silently re-open it.
    let own = env!("CARGO_PKG_NAME");
    let mut crates: Vec<String> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|line| line.split('/').nth(1).map(str::to_owned))
        .filter(|crate_name| crate_name != own)
        .collect();
    crates.sort_unstable();
    crates.dedup();
    crates
}

fn repo_root() -> Result<PathBuf, String> {
    let mut command = Command::new("git");
    command.args(["rev-parse", "--show-toplevel"]);
    match subprocess_contract::bounded_output(&mut command, Duration::from_secs(30)) {
        subprocess_contract::BoundedOutcome::Completed(output) if output.status.success() => {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            if path.is_empty() {
                Err("git printed an empty toplevel".to_owned())
            } else {
                Ok(PathBuf::from(path))
            }
        }
        subprocess_contract::BoundedOutcome::Completed(output) => Err(format!(
            "git rev-parse exited {:?}",
            output.status.code()
        )),
        subprocess_contract::BoundedOutcome::TimedOut => Err("git rev-parse timed out".to_owned()),
        subprocess_contract::BoundedOutcome::Unspawned(error) => Err(error.to_string()),
    }
}

/// Minimal SHA-256, so the bundle identity is printed without a dependency. The digest
/// is identity, not security: it exists so a stale table is distinguishable from a
/// current one.
fn sha256_hex(data: &[u8]) -> String {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = data.to_vec();
    let bit_len = (data.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for (i, word) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([word[0], word[1], word[2], word[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ ((!v[4]) & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v = [
                t1.wrapping_add(t2),
                v[0],
                v[1],
                v[2],
                v[3].wrapping_add(t1),
                v[4],
                v[5],
                v[6],
            ];
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    h.iter().map(|word| format!("{word:08x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The digest is identity; a wrong implementation would silently make every table
    /// undateable. Checked against the two canonical NIST vectors.
    #[test]
    fn the_bundle_digest_matches_known_vectors() {
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // A 64-byte boundary case, where the padding block logic is easiest to break.
        assert_eq!(
            sha256_hex(&[b'a'; 64]),
            "ffe054fe7ae0cb6dc65c3af9b61d5209f439851db43d0ba5997337df154668eb"
        );
    }
}
