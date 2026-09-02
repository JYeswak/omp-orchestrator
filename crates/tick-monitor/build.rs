#![forbid(unsafe_code)]

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=OMP_BUILD_ID");
    for path in ["../../.git/HEAD", "../../.git/index", "../../.git/packed-refs"] {
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
        .unwrap_or_else(|| "unavailable".to_owned());
    println!("cargo:rustc-env=OMP_BUILD_ID={build_id}");
}