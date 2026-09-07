#![forbid(unsafe_code)]

use std::process::Command;
use std::time::Duration;
use subprocess_contract::{bounded_output, BoundedOutcome};

fn main() {
    println!("cargo:rerun-if-env-changed=OMP_BUILD_ID");
    for path in [
        "../../.git/HEAD",
        "../../.git/index",
        "../../.git/packed-refs",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let build_id = std::env::var("OMP_BUILD_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let mut command = Command::new("git");
            command.args(["rev-parse", "HEAD"]);
            match bounded_output(&mut command, Duration::from_secs(5)) {
                BoundedOutcome::Completed(output) if output.status.success() => {
                    Some(String::from_utf8_lossy(&output.stdout).trim().to_owned())
                }
                BoundedOutcome::Completed(_) => Some(String::new()),
                BoundedOutcome::TimedOut => {
                    println!("cargo:warning=git build id probe timed out before its deadline");
                    None
                }
                BoundedOutcome::Unspawned(error) => {
                    println!("cargo:warning=git build id probe could not spawn: {error}");
                    None
                }
            }
        })
        .unwrap_or_else(|| "unavailable".to_owned());
    println!("cargo:rustc-env=OMP_BUILD_ID={build_id}");
}
