#![forbid(unsafe_code)]

use std::process::Command;

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
            Command::new("git")
                .args(["rev-parse", "HEAD"])
                .output()
                .ok()
                .filter(|output| output.status.success())
                .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
                .filter(|value| !value.is_empty())
        })
        // The old fallback was the literal `unavailable`, which `parse_build_id`
        // refuses BY DESIGN as an anonymous sentinel. So on any host whose object
        // database cannot resolve HEAD — the build workers are a bare `git init` —
        // this crate stamped an identity its own verifier rejects, and
        // `real_atomic_install_publishes_complete_binary` failed there while CI,
        // which has a real HEAD, passed. That is the producer's bug, not the
        // verifier's: `crates/omp-orchestrator/build.rs` already resolves the same
        // case to `nogit-<epoch>`, and this mirrors it.
        //
        // NO-CLAIM: this makes the id PRESENT and UNIQUE PER BUILD, not TRUE. It
        // says "underived", which is honest; it does not name a commit.
        .unwrap_or_else(|| {
            format!(
                "nogit-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|elapsed| elapsed.as_secs())
                    .unwrap_or(0)
            )
        });
    println!("cargo:rustc-env=OMP_BUILD_ID={build_id}");
}
