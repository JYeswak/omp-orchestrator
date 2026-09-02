use asupersync::process::Command;
use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use extraction_roster::{build_roster, Metadata, Roster, RosterError};
use serde::Serialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use subprocess_contract::run_output;

#[derive(Debug)]
struct Args {
    source_manifest: PathBuf,
    target_manifest: PathBuf,
    output: Option<PathBuf>,
}

#[derive(Debug, Serialize)]
struct ErrorEnvelope {
    schema_version: &'static str,
    status: &'static str,
    error: String,
}

fn main() {
    let args = match parse_args(env::args().skip(1)) {
        Ok(args) => args,
        Err(error) => fail(error),
    };
    let runtime = RuntimeBuilder::current_thread()
        .build()
        .unwrap_or_else(|error| fail(format!("runtime build failed: {error}")));
    let result = runtime.block_on(async { collect_and_build(&args).await });
    match result {
        Ok(roster) => {
            let rendered = serde_json::to_string_pretty(&roster)
                .unwrap_or_else(|error| fail(format!("roster serialization failed: {error}")));
            if let Some(output) = args.output {
                if let Err(error) = write_atomic(&output, rendered.as_bytes()) {
                    fail(format!("artifact write failed: {error}"));
                }
            }
            println!("{rendered}");
        }
        Err(error) => fail(error.to_string()),
    }
}

async fn collect_and_build(args: &Args) -> Result<Roster, RosterError> {
    let source = cargo_metadata(&args.source_manifest).await?;
    let target = cargo_metadata(&args.target_manifest).await?;
    let target_repo_name = repo_name(&args.target_manifest);
    let source_repo_name = repo_name(&args.source_manifest);
    let mut roster = build_roster(&source, &target, &target_repo_name, &source_repo_name)?;
    roster.source_manifest = args.source_manifest.display().to_string();
    roster.target_manifest = args.target_manifest.display().to_string();
    Ok(roster)
}

async fn cargo_metadata(manifest: &Path) -> Result<Metadata, RosterError> {
    if !manifest.is_file() {
        return Err(RosterError::Io {
            path: manifest.display().to_string(),
            detail: "manifest does not exist".to_owned(),
        });
    }
    let mut command = Command::new(env::var_os("CARGO").unwrap_or_else(|| "cargo".into()));
    command.args([
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
        "--manifest-path",
    ]);
    command.arg(manifest);
    let cx = Cx::current().ok_or_else(|| RosterError::Io {
        path: manifest.display().to_string(),
        detail: "no active cancellation context".to_owned(),
    })?;
    let output = run_output(&cx, command)
        .await
        .map_err(|error| RosterError::Io {
            path: manifest.display().to_string(),
            detail: format!("cargo metadata failed: {error}"),
        })?;
    if !output.status.success() {
        return Err(RosterError::Io {
            path: manifest.display().to_string(),
            detail: format!(
                "cargo metadata exited {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    serde_json::from_slice(&output.stdout).map_err(|error| RosterError::Io {
        path: manifest.display().to_string(),
        detail: format!("cargo metadata JSON invalid: {error}"),
    })
}

fn repo_name(manifest: &Path) -> String {
    manifest.parent().and_then(Path::file_name).map_or_else(
        || "unknown-repo".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    )
}

fn parse_args(values: impl Iterator<Item = String>) -> Result<Args, String> {
    let mut source_manifest = None;
    let mut target_manifest = None;
    let mut output = None;
    let mut values = values.peekable();
    while let Some(argument) = values.next() {
        let destination = match argument.as_str() {
            "--source-manifest" => &mut source_manifest,
            "--target-manifest" => &mut target_manifest,
            "--output" => &mut output,
            "--help" | "-h" => {
                println!(
                    "usage: extraction-roster --source-manifest PATH --target-manifest PATH [--output PATH]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument `{other}`")),
        };
        let value = values
            .next()
            .ok_or_else(|| format!("{argument} requires a value"))?;
        *destination = Some(PathBuf::from(value));
    }
    Ok(Args {
        source_manifest: source_manifest.ok_or("--source-manifest is required")?,
        target_manifest: target_manifest.ok_or("--target-manifest is required")?,
        output,
    })
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension("json.tmp");
    fs::write(&temporary, bytes).map_err(|error| error.to_string())?;
    fs::rename(&temporary, path).map_err(|error| error.to_string())
}

fn fail(error: String) -> ! {
    let envelope = ErrorEnvelope {
        schema_version: "omp.extraction-roster.v1",
        status: "ERROR",
        error,
    };
    eprintln!(
        "{}",
        serde_json::to_string(&envelope).unwrap_or_else(|_| {
            "{\"schema_version\":\"omp.extraction-roster.v1\",\"status\":\"ERROR\",\"error\":\"serialization failure\"}".to_owned()
        })
    );
    std::process::exit(2)
}
