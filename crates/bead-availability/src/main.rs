#![forbid(unsafe_code)]

use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use bead_availability::collect_live;
use std::env;
use std::process::ExitCode;

fn usage() -> &'static str {
    "usage: bead-availability [--json] [--br PATH]\n       reports live blockers for every non-terminal bead; closed blockers are RELEASED"
}

fn main() -> ExitCode {
    let mut json = false;
    let mut br_program = String::from("br");
    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json = true,
            "--help" | "-h" => {
                println!("{}", usage());
                return ExitCode::SUCCESS;
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
