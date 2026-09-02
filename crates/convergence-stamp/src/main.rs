#![forbid(unsafe_code)]
//! `convergence-stamp` — cut the single stamped round document, or refuse a new round.
//!
//! ```text
//! cargo run -p convergence-stamp                # report; exits 1 if a round would be refused
//! cargo run -p convergence-stamp -- --write     # (re)cut ROUNDS.md + STAMP.toml
//! cargo run -p convergence-stamp -- --check     # gate mode: refuse a new round, or exit 0
//! ```
//!
//! `--check` exists so the refusal is reachable from a human, a hook, and CI with the same
//! command. A gate only a test can invoke is one `cargo test` away from being unreachable.

use convergence_stamp::{census, parse_stamp, refusals, render_document, render_stamp};
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

fn repo_root(args: &[String]) -> PathBuf {
    args.iter()
        .position(|a| a == "--repo")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn now_utc() -> String {
    // Seconds since epoch is enough to order two stamps and cannot drift with a locale.
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("epoch:{secs}")
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = repo_root(&args);
    let write = args.iter().any(|a| a == "--write");
    let check = args.iter().any(|a| a == "--check");

    let c = census(&root);
    if c.rows.is_empty() || c.section_digests.is_empty() {
        eprintln!(
            "STAMP ERROR: census found {} round rows and {} sections under {} — an empty scan \
             is an ERROR, never a pass",
            c.rows.len(),
            c.section_digests.len(),
            root.display()
        );
        return ExitCode::from(2);
    }

    let stamp_path = root.join("docs/plan/STAMP.toml");
    let doc_path = root.join("docs/plan/ROUNDS.md");

    if write {
        let cut_at = now_utc();
        let doc = render_document(&c, &cut_at);
        let stamp = render_stamp(&c, &cut_at);
        if let Some(parent) = doc_path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Err(error) = fs::write(&doc_path, doc.as_bytes()) {
            eprintln!("STAMP ERROR: write {}: {error}", doc_path.display());
            return ExitCode::from(2);
        }
        if let Err(error) = fs::write(&stamp_path, stamp.as_bytes()) {
            eprintln!("STAMP ERROR: write {}: {error}", stamp_path.display());
            return ExitCode::from(2);
        }
        println!(
            "STAMP CUT: {} rounds, {} rows, {} declared, {} dispositioned, {} sections",
            c.declared_by_round.len(),
            c.rows.len(),
            c.declared_by_round.values().sum::<u64>(),
            c.dispositioned.len(),
            c.section_digests.len()
        );
        println!("  {}", doc_path.display());
        println!("  {}", stamp_path.display());
    }

    let stamp = fs::read_to_string(&stamp_path).ok().map(|t| parse_stamp(&t));
    let refused = refusals(&c, stamp.as_ref());

    if refused.is_empty() {
        println!(
            "ROUND ADMITTED: stamp covers all {} rounds and every section digest matches",
            c.declared_by_round.len()
        );
        return ExitCode::SUCCESS;
    }

    println!("ROUND REFUSED — {} reason(s):", refused.len());
    for r in &refused {
        println!("  {r}");
    }
    if check || !write {
        return ExitCode::FAILURE;
    }
    ExitCode::FAILURE
}
