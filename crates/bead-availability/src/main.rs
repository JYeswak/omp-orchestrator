#![forbid(unsafe_code)]

use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use bead_availability::{classify, collect_live, parse_graph_json, ScanError};
use std::env;
use std::path::Path;
use std::process::ExitCode;

#[used]
static BUILD_ID_MARKER: &[u8] = concat!("build_id=", env!("OMP_BUILD_ID")).as_bytes();

fn usage() -> &'static str {
    "usage: bead-availability [--json] [--br PATH] [--version]\n\
            bead-availability --graph PATH.json\n\
            live blockers: closed blockers are RELEASED\n\
            --graph: classify live blocks-edges as Direct/Transitive/InheritanceFailure"
}

fn main() -> ExitCode {
    let mut json = false;
    let mut br_program = String::from("br");
    let mut graph: Option<String> = None;
    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json = true,
            "--version" => {
                println!("bead-availability 0.1.0 build_id={}", env!("OMP_BUILD_ID"));
                return ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                println!("{}", usage());
                return ExitCode::SUCCESS;
            }
            value if value == "--graph" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    eprintln!("BEAD_AVAILABILITY_USAGE missing value for --graph");
                    return ExitCode::from(2);
                };
                graph = Some(path.clone());
            }
            value if value.starts_with("--graph=") => {
                graph = Some(value[8..].to_owned());
            }
            value if value == "--br" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    eprintln!("BEAD_AVAILABILITY_USAGE missing value for --br");
                    return ExitCode::from(2);
                };
                br_program.clone_from(path);
            }
            value if value.starts_with("--br=") => {
                br_program = value[5..].to_owned();
            }
            value => {
                eprintln!("BEAD_AVAILABILITY_USAGE unknown argument={value}");
                return ExitCode::from(2);
            }
        }
        index += 1;
    }

    if let Some(path) = graph {
        return run_graph(Path::new(&path));
    }

    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("BEAD_AVAILABILITY_ERROR reason=runtime_build detail={error}");
            return ExitCode::from(2);
        }
    };
    let result = runtime.block_on(async move {
        let cx = Cx::current()
            .ok_or_else(|| "BEAD_AVAILABILITY_ERROR reason=no_runtime_context".to_owned())?;
        collect_live(&cx, &br_program).await
    });
    match result {
        Ok(report) => {
            if json {
                match serde_json::to_string_pretty(&report) {
                    Ok(output) => println!("{output}"),
                    Err(error) => {
                        eprintln!("BEAD_AVAILABILITY_ERROR reason=render_json detail={error}");
                        return ExitCode::from(2);
                    }
                }
            } else {
                print!("{}", report.render_text());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("BEAD_AVAILABILITY_ERROR {error}");
            ExitCode::from(2)
        }
    }
}

fn run_graph(path: &Path) -> ExitCode {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "INVERSION_GRAPH_UNREADABLE path={} error={error}",
                path.display()
            );
            return ExitCode::from(3);
        }
    };
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("INVERSION_GRAPH_MALFORMED json={error}");
            return ExitCode::from(3);
        }
    };
    let (issues, edges) = match parse_graph_json(&value) {
        Ok(graph) => graph,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(error.exit_code());
        }
    };
    match classify(&issues, &edges) {
        Ok(classification) => {
            eprint!("{}", classification.report());
            ExitCode::from(classification.gate_exit())
        }
        Err(ScanError::EmptyIssueSet) => {
            eprintln!("{}", ScanError::EmptyIssueSet);
            ExitCode::from(2)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}
