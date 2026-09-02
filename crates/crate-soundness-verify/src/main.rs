#![forbid(unsafe_code)]

use crate_soundness_verify::{
    derive_crates, forbid_failure, resolve_deny_bin, run_binary, run_command, selftest_output,
    Config, EXIT_RED,
};
use std::process::ExitCode;

fn usage_error(message: impl std::fmt::Display) -> ExitCode {
    eprintln!("usage error: {message}");
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mode = args.next().unwrap_or_default();
    if mode == "--help" || mode == "-h" {
        println!("crate-soundness-verify [--quiet|--list-crates|--selftest]");
        return ExitCode::SUCCESS;
    }
    if mode == "--selftest" {
        return match selftest_output() {
            Ok(lines) => {
                for line in lines {
                    println!("{line}");
                }
                ExitCode::SUCCESS
            }
            Err(error) => {
                println!("crate-soundness-verify: RED selftest={error}");
                ExitCode::from(EXIT_RED as u8)
            }
        };
    }
    if !mode.is_empty() && mode != "--quiet" && mode != "--list-crates" {
        return usage_error(format!("unknown argument {mode}"));
    }
    if args.next().is_some() {
        return usage_error("unexpected extra argument");
    }
    let config = match Config::from_env() {
        Ok(config) => config,
        Err(error) => return usage_error(error),
    };
    let crates = match derive_crates(&config.repo) {
        Ok(crates) => crates,
        Err(error) => {
            println!("crate-soundness-verify: RED stage=derive reason={error}");
            return ExitCode::from(EXIT_RED as u8);
        }
    };
    println!(
        "DERIVED_CRATE_SET count={} source={}/Cargo.toml",
        crates.len(),
        config.repo.display()
    );
    if mode == "--list-crates" {
        for name in crates {
            println!("{name}");
        }
        return ExitCode::SUCCESS;
    }
    if let Err(error) = std::fs::create_dir_all(&config.target_dir) {
        println!(
            "crate-soundness-verify: RED stage=setup reason=cannot create target {}: {error}",
            config.target_dir.display()
        );
        return ExitCode::from(EXIT_RED as u8);
    }
    println!("==> [1/5] forbid(unsafe_code) and no unsafe-code escape hatch");
    let mut failures = Vec::new();
    for name in &crates {
        if let Some(error) = forbid_failure(&config.repo, name) {
            println!("crate-soundness-verify: RED stage=forbid reason={error}");
            failures.push(format!("forbid:{name}"));
        }
    }
    if failures.is_empty() {
        println!("    ok: derived crates forbid unsafe with no escape hatch");
    } else {
        println!(
            "crate-soundness-verify: RED failures={} (later stages are not meaningful)",
            failures.join(",")
        );
        return ExitCode::from(EXIT_RED as u8);
    }

    println!("==> [2/5] async-surface tripwire (informational drift signal, non-fatal)");
    for name in &crates {
        let source = config.repo.join("crates").join(name).join("src");
        if source_has_async(&source) {
            println!("    WARNING {name} introduced async surface");
        }
    }
    println!("    async tripwire complete");

    println!("==> [3/5] clippy -D warnings (bounded child)");
    for name in &crates {
        let result = run_command(
            &config,
            name,
            &["clippy", "--quiet", "--all-targets", "--", "-D", "warnings"],
        );
        if result.timed_out {
            println!(
                "crate-soundness-verify: TIMEOUT stage=clippy crate={name} elapsed={}s",
                config.timeout.as_secs()
            );
            failures.push(format!("timeout:clippy:{name}"));
        } else if result
            .status
            .map(|status| !status.success())
            .unwrap_or(true)
        {
            println!("crate-soundness-verify: RED stage=clippy crate={name} reason=nonzero child");
            failures.push(format!("clippy:{name}"));
        }
    }

    println!("==> [4/5] tests + fires-on-known-bad self-tests (no fail-fast)");
    for name in &crates {
        let result = run_command(&config, name, &["test", "--quiet", "--no-fail-fast"]);
        if result.timed_out {
            println!(
                "crate-soundness-verify: TIMEOUT stage=test crate={name} elapsed={}s",
                config.timeout.as_secs()
            );
            failures.push(format!("timeout:test:{name}"));
        } else if result
            .status
            .map(|status| !status.success())
            .unwrap_or(true)
        {
            println!("crate-soundness-verify: RED stage=test crate={name} reason=nonzero child");
            failures.push(format!("test:{name}"));
        }
    }

    println!("==> [5/5] dependency soundness");
    // One workspace graph, one verdict. Per-crate `cargo deny` re-walks the same
    // Cargo.lock; advisories-only (not licenses/bans) because a default license
    // check is permanently red without a deny.toml allowlist we do not have.
    let deny_override = std::env::var_os("CRATE_SOUNDNESS_DENY_BIN").map(std::path::PathBuf::from);
    let deps_unrun = match resolve_deny_bin(&config.cargo, deny_override.as_deref()) {
        Ok(deny) => {
            println!("    cargo-deny {}", deny.display());
            let result = run_binary(
                &config.cargo,
                &config.repo,
                &config.target_dir,
                &["deny", "check", "advisories"],
                config.timeout,
            );
            if result.timed_out {
                println!(
                    "crate-soundness-verify: TIMEOUT stage=deps elapsed={}s",
                    config.timeout.as_secs()
                );
                failures.push("timeout:deps".into());
            } else if result
                .status
                .map(|status| !status.success())
                .unwrap_or(true)
            {
                println!("crate-soundness-verify: RED stage=deps reason=advisory");
                failures.push("deps:advisories".into());
            }
            None
        }
        Err(reason) => {
            println!("UNRUN stage=deps reason={reason}");
            Some(reason)
        }
    };
    if failures.is_empty() {
        match deps_unrun {
            None => println!(
                "crate-soundness-verify: PASS (derived set; forbid airtight; bounded clippy/test; deps advisories clean)"
            ),
            Some(reason) => println!(
                "crate-soundness-verify: PASS (derived set; forbid airtight; bounded clippy/test) UNRUN stage=deps reason={reason}"
            ),
        }
        ExitCode::SUCCESS
    } else {
        println!(
            "crate-soundness-verify: RED failures={} (all crates were evaluated)",
            failures.join(",")
        );
        ExitCode::from(EXIT_RED as u8)
    }
}

fn source_has_async(root: &std::path::Path) -> bool {
    let Ok(entries) = std::fs::read_dir(root) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            source_has_async(&path)
        } else {
            std::fs::read_to_string(path)
                .map(|text| {
                    text.contains("async fn")
                        || text.contains(".await")
                        || text.contains("tokio")
                        || text.contains("async-std")
                })
                .unwrap_or(false)
        }
    })
}
