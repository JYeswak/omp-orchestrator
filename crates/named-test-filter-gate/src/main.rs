#![forbid(unsafe_code)]

use named_test_filter_gate::{census_beads, grade, implemented_test_fns, Grade};
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("census") => census(args.next().map(PathBuf::from)),
        Some("grade") => {
            let exit: i32 = args
                .next()
                .and_then(|s| s.parse().ok())
                .unwrap_or(0);
            let mut buf = String::new();
            if io::stdin().read_to_string(&mut buf).is_err() {
                eprintln!("NAMED_TEST_UNPARSEABLE: stdin unread");
                return ExitCode::from(2);
            }
            match grade(&buf, exit) {
                Grade::Admit { passed } => {
                    println!("NAMED_TEST_ADMIT passed={passed}");
                    ExitCode::SUCCESS
                }
                Grade::Refuse(err) => {
                    eprintln!("{err}");
                    ExitCode::from(1)
                }
            }
        }
        _ => {
            eprintln!("usage: named-test-filter-gate grade <exit> < cargo-output");
            eprintln!("       named-test-filter-gate census [repo-root]");
            ExitCode::from(2)
        }
    }
}

fn census(root: Option<PathBuf>) -> ExitCode {
    let root = root.unwrap_or_else(|| PathBuf::from("."));
    let jsonl = match fs::read_to_string(root.join(".beads/issues.jsonl")) {
        Ok(text) => text,
        Err(_) => {
            eprintln!("NAMED_TEST_UNPARSEABLE: .beads/issues.jsonl unread");
            return ExitCode::from(2);
        }
    };
    let implemented = implemented_test_fns(&root.join("crates")).unwrap_or_default();
    match census_beads(&jsonl, &implemented) {
        Ok(rows) => {
            let unresolved = rows.iter().filter(|row| !row.resolved).count();
            println!(
                "named_test_refs={} unresolved={} implemented_fns={}",
                rows.len(),
                unresolved,
                implemented.len()
            );
            for row in &rows {
                println!(
                    "{} {} {}",
                    if row.resolved { "RESOLVED" } else { "UNRESOLVED" },
                    row.bead_id,
                    row.test_fn
                );
            }
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!("{err}");
            ExitCode::from(2)
        }
    }
}
