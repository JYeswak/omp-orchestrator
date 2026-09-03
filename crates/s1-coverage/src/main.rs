#![forbid(unsafe_code)]

use asupersync::process::Command;
use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use s1_coverage::{
    compute, parse_beads_jsonl, render_markdown, BeadRecord, CoverageInput, CoverageReport,
    SourceText, CONTRACT_PATHS,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use subprocess_contract::run_output;

const S1_TOML_PATH: &str = "docs/plan/flow/boxes/S1.toml";
const BEADS_PATH: &str = ".beads/issues.jsonl";

#[derive(Debug, Clone, Copy)]
enum Mode {
    Tree,
    Worktree,
    Both,
}

impl Mode {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "tree" => Some(Self::Tree),
            "worktree" => Some(Self::Worktree),
            "both" => Some(Self::Both),
            _ => None,
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Tree => "tree",
            Self::Worktree => "worktree",
            Self::Both => "both",
        }
    }
}

#[derive(Debug, Clone)]
struct Args {
    repo: PathBuf,
    revision: String,
    mode: Mode,
    json: bool,
    output: Option<PathBuf>,
}

fn usage() -> &'static str {
    "usage: s1-coverage [--repo PATH] [--rev REV] [--mode tree|worktree|both] [--json] [--output PATH]\n\nReads six S1 contracts, boxes/S1.toml, and .beads/issues.jsonl. Default input is git REV=HEAD; use --mode both to compare the clean tree with local edits."
}

fn parse_args() -> Result<Args, String> {
    let mut repo = PathBuf::from(".");
    let mut revision = String::from("HEAD");
    let mut mode = Mode::Tree;
    let mut json = false;
    let mut output = None;
    let values = env::args().skip(1).collect::<Vec<_>>();
    let mut index = 0;
    while index < values.len() {
        match values[index].as_str() {
            "--help" | "-h" => {
                println!("{}", usage());
                std::process::exit(0);
            }
            "--json" => json = true,
            "--repo" | "--rev" | "--mode" | "--output" => {
                let option = values[index].clone();
                index += 1;
                let Some(value) = values.get(index) else {
                    return Err(format!("S1_COVERAGE_USAGE missing value for {option}"));
                };
                match option.as_str() {
                    "--repo" => repo = PathBuf::from(value),
                    "--rev" => revision.clone_from(value),
                    "--mode" => {
                        mode = Mode::parse(value).ok_or_else(|| {
                            format!("S1_COVERAGE_USAGE invalid mode={value}; expected tree|worktree|both")
                        })?;
                    }
                    "--output" => output = Some(PathBuf::from(value)),
                    _ => unreachable!(),
                }
            }
            value if value.starts_with("--repo=") => repo = PathBuf::from(&value[7..]),
            value if value.starts_with("--rev=") => revision = value[6..].to_owned(),
            value if value.starts_with("--mode=") => {
                mode = Mode::parse(&value[7..]).ok_or_else(|| {
                    format!("S1_COVERAGE_USAGE invalid mode={}; expected tree|worktree|both", &value[7..])
                })?;
            }
            value if value.starts_with("--output=") => output = Some(PathBuf::from(&value[9..])),
            value => return Err(format!("S1_COVERAGE_USAGE unknown argument={value}")),
        }
        index += 1;
    }
    Ok(Args {
        repo,
        revision,
        mode,
        json,
        output,
    })
}

async fn read_tree_file(cx: &Cx, repo: &Path, revision: &str, path: &str) -> Result<String, String> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(repo.to_string_lossy().as_ref())
        .arg("show")
        .arg(format!("{revision}:{path}"));
    let output = run_output(cx, command)
        .await
        .map_err(|error| format!("S1_COVERAGE_TREE_READ path={path} error={error}"))?;
    if !output.status.success() {
        return Err(format!(
            "S1_COVERAGE_TREE_READ path={path} status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("S1_COVERAGE_TREE_UTF8 path={path} error={error}"))
}

fn read_worktree_file(repo: &Path, path: &str) -> Result<String, String> {
    fs::read_to_string(repo.join(path))
        .map_err(|error| format!("S1_COVERAGE_WORKTREE_READ path={path} error={error}"))
}

fn make_input(contracts: Vec<SourceText>, s1_toml: String, beads: Vec<BeadRecord>) -> CoverageInput {
    CoverageInput {
        contracts,
        s1_toml,
        beads,
    }
}

async fn load_tree(cx: &Cx, repo: &Path, revision: &str) -> Result<CoverageInput, String> {
    let mut contracts = Vec::with_capacity(CONTRACT_PATHS.len());
    for path in CONTRACT_PATHS {
        cx.checkpoint()
            .map_err(|_| "S1_COVERAGE_ERROR reason=cancelled".to_owned())?;
        contracts.push(SourceText {
            path: path.to_owned(),
            text: read_tree_file(cx, repo, revision, path).await?,
        });
    }
    let s1_toml = read_tree_file(cx, repo, revision, S1_TOML_PATH).await?;
    let beads_text = read_tree_file(cx, repo, revision, BEADS_PATH).await?;
    let beads = parse_beads_jsonl(&beads_text).map_err(|error| error.to_string())?;
    Ok(make_input(contracts, s1_toml, beads))
}

fn load_worktree(repo: &Path) -> Result<CoverageInput, String> {
    let contracts = CONTRACT_PATHS
        .into_iter()
        .map(|path| {
            read_worktree_file(repo, path).map(|text| SourceText {
                path: path.to_owned(),
                text,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let s1_toml = read_worktree_file(repo, S1_TOML_PATH)?;
    let beads_text = read_worktree_file(repo, BEADS_PATH)?;
    let beads = parse_beads_jsonl(&beads_text).map_err(|error| error.to_string())?;
    Ok(make_input(contracts, s1_toml, beads))
}

async fn report_for(cx: &Cx, args: &Args, mode: Mode) -> Result<CoverageReport, String> {
    let input = match mode {
        Mode::Tree => load_tree(cx, &args.repo, &args.revision).await?,
        Mode::Worktree => load_worktree(&args.repo)?,
        Mode::Both => unreachable!(),
    };
    compute(&input, mode.as_str(), &args.revision).map_err(|error| error.to_string())
}

fn render_json(report: &CoverageReport) -> Result<String, String> {
    serde_json::to_string_pretty(report).map_err(|error| format!("S1_COVERAGE_RENDER_JSON error={error}"))
}

async fn execute(cx: &Cx, args: &Args) -> Result<String, String> {
    match args.mode {
        Mode::Tree | Mode::Worktree => {
            let report = report_for(cx, args, args.mode).await?;
            if args.json {
                render_json(&report)
            } else {
                Ok(render_markdown(&report))
            }
        }
        Mode::Both => {
            let tree = report_for(cx, args, Mode::Tree).await?;
            let worktree = report_for(cx, args, Mode::Worktree).await?;
            if args.json {
                serde_json::to_string_pretty(&serde_json::json!({
                    "schema": "s1-coverage/v1-both",
                    "mode": "both",
                    "tree": tree,
                    "worktree": worktree,
                }))
                .map_err(|error| format!("S1_COVERAGE_RENDER_JSON error={error}"))
            } else {
                Ok(format!(
                    "## TREE ({})\n\n{}\n## WORKTREE\n\n{}",
                    args.revision,
                    render_markdown(&tree),
                    render_markdown(&worktree)
                ))
            }
        }
    }
}

fn main() -> ExitCode {
    let args = match parse_args() {
        Ok(args) => args,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(2);
        }
    };
    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("S1_COVERAGE_ERROR reason=runtime_build detail={error}");
            return ExitCode::from(2);
        }
    };
    let result = runtime.block_on(async {
        let cx = Cx::current()
            .ok_or_else(|| "S1_COVERAGE_ERROR reason=no_runtime_context".to_owned())?;
        execute(&cx, &args).await
    });
    match result {
        Ok(rendered) => {
            if let Some(path) = args.output {
                if let Err(error) = fs::write(&path, rendered) {
                    eprintln!("S1_COVERAGE_ERROR reason=write_output path={} detail={error}", path.display());
                    return ExitCode::from(2);
                }
            } else {
                println!("{rendered}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("S1_COVERAGE_ERROR {error}");
            ExitCode::from(2)
        }
    }
}
