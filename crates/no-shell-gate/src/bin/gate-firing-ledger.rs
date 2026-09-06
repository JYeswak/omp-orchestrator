#![forbid(unsafe_code)]

//! Query and census surface for the commit/build gate firing ledger.

use no_shell_gate::firing_ledger::{
    census_from_names, default_ledger_path, discover_gate_crates, query_gate, CENSUS_INPUT_EMPTY,
};
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        eprintln!(
            "gate-firing-ledger census --root PATH\n\
             gate-firing-ledger query --gate NAME [--ledger PATH] [--root PATH]"
        );
        return ExitCode::SUCCESS;
    }
    match args.first().map(String::as_str) {
        Some("census") => census_cmd(&args[1..]),
        Some("query") => query_cmd(&args[1..]),
        _ => {
            eprintln!("usage: gate-firing-ledger census|query");
            ExitCode::from(2)
        }
    }
}

fn flag<'a>(args: &'a [String], name: &str) -> Option<&'a str> {
    let index = args.iter().position(|arg| arg == name)?;
    args.get(index + 1).map(String::as_str)
}

fn census_cmd(args: &[String]) -> ExitCode {
    let root = PathBuf::from(flag(args, "--root").unwrap_or("."));
    let crates = root.join("crates");
    match discover_gate_crates(&crates) {
        Err(error) => {
            println!("{{\"status\":\"{CENSUS_INPUT_EMPTY}\",\"error\":\"{error}\"}}");
            ExitCode::from(2)
        }
        Ok(surface) => {
            let runtime = runtime_literals(&root, &surface);
            match census_from_names(&surface, &runtime) {
                Ok(census) => {
                    println!(
                        "{{\"status\":\"ok\",\"surface\":{},\"runtime_hit\":{},\"commit_build_only\":{}}}",
                        json_array(&census.surface),
                        json_array(&census.runtime_hit),
                        json_array(&census.commit_build_only),
                    );
                    ExitCode::SUCCESS
                }
                Err(error) => {
                    println!("{{\"status\":\"{CENSUS_INPUT_EMPTY}\",\"error\":\"{error}\"}}");
                    ExitCode::from(2)
                }
            }
        }
    }
}

fn query_cmd(args: &[String]) -> ExitCode {
    let Some(gate) = flag(args, "--gate") else {
        eprintln!("query requires --gate");
        return ExitCode::from(2);
    };
    let root = PathBuf::from(flag(args, "--root").unwrap_or("."));
    let ledger = flag(args, "--ledger")
        .map(PathBuf::from)
        .unwrap_or_else(|| default_ledger_path(&root));
    match query_gate(&ledger, gate) {
        Ok(result) => {
            println!("{result}");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

fn runtime_literals(root: &std::path::Path, surface: &[String]) -> Vec<String> {
    let src = root.join("crates/omp-orchestrator/src");
    let text = read_tree_rs(&src);
    let mut hits: Vec<String> = surface
        .iter()
        .filter(|name| {
            text.contains(name.as_str()) || text.contains(&name.replace('-', "_"))
        })
        .cloned()
        .collect();
    if text.contains("dispatch-claim-fence") || text.contains("dispatch_claim_fence") {
        hits.push("dispatch-claim-fence".to_owned());
    }
    hits.sort();
    hits.dedup();
    hits
}

fn read_tree_rs(dir: &std::path::Path) -> String {
    let mut out = String::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.is_dir() {
            out.push_str(&read_tree_rs(&path));
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(text) = std::fs::read_to_string(&path) {
                out.push_str(&text);
                out.push('\n');
            }
        }
    }
    out
}

fn json_array(values: &[String]) -> String {
    format!(
        "[{}]",
        values
            .iter()
            .map(|value| format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\"")))
            .collect::<Vec<_>>()
            .join(",")
    )
}
