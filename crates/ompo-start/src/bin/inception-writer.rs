#![forbid(unsafe_code)]

use ompo_start::inception::write_inception;
use std::path::PathBuf;
use std::process::ExitCode;

fn usage() -> &'static str {
    "usage: inception-writer [--repo PATH] [--output PATH]\n\nWrites and reads back .omp-orchestrator/inception.json with the SCHEMAS.toml required keys."
}

fn main() -> ExitCode {
    let mut repo = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut output = None;
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--repo" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!("INCEPTION_ARGUMENT_ERROR --repo requires PATH\n{}", usage());
                    return ExitCode::from(2);
                };
                repo = PathBuf::from(value);
            }
            "--output" => {
                index += 1;
                let Some(value) = args.get(index) else {
                    eprintln!(
                        "INCEPTION_ARGUMENT_ERROR --output requires PATH\n{}",
                        usage()
                    );
                    return ExitCode::from(2);
                };
                output = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                println!("{}", usage());
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!(
                    "INCEPTION_ARGUMENT_ERROR unknown argument {other}\n{}",
                    usage()
                );
                return ExitCode::from(2);
            }
        }
        index += 1;
    }

    let destination = output.unwrap_or_else(|| repo.join(".omp-orchestrator/inception.json"));
    match write_inception(&repo, &destination) {
        Ok(manifest) => {
            println!(
                "INCEPTION_WRITTEN path={} schema_version={} project_id={} required_tools={} readback=PASS",
                destination.display(),
                manifest.schema_version,
                manifest.project_id,
                manifest.required_tools.len()
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("INCEPTION_ERROR {error}");
            ExitCode::from(2)
        }
    }
}
