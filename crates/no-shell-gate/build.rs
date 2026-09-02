#![forbid(unsafe_code)]

use std::env;
use std::process::Command;

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn main() {
    println!("cargo:rerun-if-env-changed=OMP_BUILD_ID");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
    println!("cargo:rerun-if-changed=../../.git/packed-refs");

    let build_id = env::var("OMP_BUILD_ID")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| {
            let head = git_output(&["rev-parse", "HEAD"])?;
            let dirty = git_output(&["status", "--porcelain"]).is_some_and(|output| !output.is_empty());
            Some(if dirty { format!("{head}-dirty") } else { head })
        })
        .unwrap_or_else(|| "nogit".to_owned());
    println!("cargo:rustc-env=OMP_BUILD_ID={build_id}");
}
