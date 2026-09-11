#![forbid(unsafe_code)]

use std::env;
use std::path::PathBuf;
use std::process::Command;
// ONE implementation of the digest, shared with the crate that checks it at run time.
// `src/hook_digest.rs` is `mod hook_digest;` in the library and `include!`d here INSIDE a module
// block -- the block is required, not cosmetic: the file opens with `//!` inner docs and brings
// its own `use` items, both of which are errors at build-script top level. Two hand-written
// copies of one hash fail in the worst direction: they render a CORRECT hook permanently stale.
mod hook_digest {
    include!("src/hook_digest.rs");
}
use hook_digest::{hook_source_files, hook_source_manifest};

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    (!value.is_empty()).then_some(value)
}

fn repo_root() -> PathBuf {
    // build.rs runs with the CRATE as cwd; every member sits at crates/<name>/.
    PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
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

    // THE CONTENT STAMP (`omp-orchestrator-zzg2x`).
    //
    // The covered paths are enumerated FROM DISK and not from `git ls-tree HEAD`, and that is a
    // measurement rather than a preference: this build script runs on the cross-build worker,
    // whose checkout is a SYNCED WORKTREE with no resolvable refs. Measured 2026-09-11 on the
    // lane -- `git ls-tree -r HEAD` returns "fatal: Not a valid object name HEAD" and
    // `git ls-files` returns ZERO rows, so a git-based enumeration here yields an EMPTY covered
    // set and the anti-vacuity assert below turns that into an unbuildable hook.
    //
    // The property HEAD-enumeration was wanted for -- that an UNTRACKED `.rs` under a covered
    // `src/` cannot enter the stamp -- is enforced at RUN time by `commit_ratchets::hook_freshness`,
    // which runs on a host where git works and refuses with the path NAMED. A clean clone has no
    // untracked files, so the two enumerations agree there; they differ only in a dirty tree,
    // which is exactly where a loud refusal beats a silent hash.
    let root = repo_root();
    let files = hook_source_files(&root);
    assert!(
        !files.is_empty(),
        "ANTI-VACUITY: the hook's covered source set is EMPTY, so the stamp would describe \
         nothing. An empty covered set is an ERROR, never a pass."
    );

    // WITHOUT THESE THE WHOLE UNIT IS DECORATION. The watches above are `.git/HEAD`,
    // `.git/index` and `.git/packed-refs` -- and a bare worktree edit moves NONE of the three,
    // so cargo would serve a CACHED stamp describing an older tree: a digest that is stale
    // exactly when it matters. Name the covered FILES, never the git plumbing.
    for path in &files {
        println!("cargo:rerun-if-changed={}", path.display());
    }

    let manifest = match hook_source_manifest(&root) {
        Ok(manifest) => manifest,
        // A covered file that cannot be read is a REFUSAL, never a hash of nothing: hashing the
        // empty string would make an ABSENT file and an EMPTY file identical, and skipping it
        // would let a deletion read GREEN.
        Err(error) => panic!("HOOK_DIGEST_REFUSED: {error}"),
    };
    println!("cargo:rustc-env=OMP_HOOK_SOURCE_MANIFEST={}", manifest.replace('\n', ";"));
    println!("cargo:rustc-env=OMP_HOOK_SOURCE_FILE_COUNT={}", files.len());
}
