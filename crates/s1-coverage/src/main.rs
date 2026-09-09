#![forbid(unsafe_code)]

use asupersync::process::Command;
use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use s1_coverage::{
    compare_reports, compute_with_manifest, parse_beads_jsonl, render_markdown, BeadRecord,
    CoverageComparison, CoverageInput, CoverageReport, InputManifest, SourceText, CONTRACT_PATHS,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use subprocess_contract::run_output;

const S1_TOML_PATH: &str = "docs/plan/flow/boxes/S1.toml";
const BEADS_PATH: &str = ".beads/issues.jsonl";
const PROVENANCE_PATHS: [&str; 3] = [
    "docs/plan/flow/S1-COVERAGE.md",
    "docs/plan/flow/maturity0",
    ".git/s1_cov.py",
];

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
    head: String,
    mode: Mode,
    compare: Option<(String, String)>,
    json: bool,
    output: Option<PathBuf>,
}

fn usage() -> &'static str {
    "usage: s1-coverage [--repo PATH] [--head REV] [--compare BASE HEAD] [--mode tree|worktree|both] [--json] [--output PATH]\n\nReads six S1 contracts and reports TREE/INDEX/WORKTREE/WORKTREE_ONLY provenance plus relation-based growth and closure."
}

fn parse_args() -> Result<Args, String> {
    let mut repo = PathBuf::from(".");
    let mut head = String::from("HEAD");
    let mut mode = Mode::Tree;
    let mut compare = None;
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
            "--repo" | "--head" | "--rev" | "--mode" | "--output" | "--compare" => {
                let option = values[index].clone();
                index += 1;
                let Some(value) = values.get(index) else {
                    return Err(format!("S1_COVERAGE_USAGE missing value for {option}"));
                };
                match option.as_str() {
                    "--repo" => repo = PathBuf::from(value),
                    "--head" | "--rev" => head.clone_from(value),
                    "--mode" => {
                        mode = Mode::parse(value).ok_or_else(|| {
                            format!("S1_COVERAGE_USAGE invalid mode={value}; expected tree|worktree|both")
                        })?;
                    }
                    "--output" => output = Some(PathBuf::from(value)),
                    "--compare" => {
                        index += 1;
                        let Some(compare_head) = values.get(index) else {
                            return Err("S1_COVERAGE_USAGE --compare requires BASE HEAD".to_owned());
                        };
                        compare = Some((value.clone(), compare_head.clone()));
                    }
                    _ => unreachable!(),
                }
            }
            value if value.starts_with("--repo=") => repo = PathBuf::from(&value[7..]),
            value if value.starts_with("--head=") => head = value[7..].to_owned(),
            value if value.starts_with("--rev=") => head = value[6..].to_owned(),
            value if value.starts_with("--mode=") => {
                mode = Mode::parse(&value[7..]).ok_or_else(|| {
                    format!("S1_COVERAGE_USAGE invalid mode={}; expected tree|worktree|both", &value[7..])
                })?;
            }
            value if value.starts_with("--output=") => output = Some(PathBuf::from(&value[9..])),
            value if value.starts_with("--compare=") => {
                let pair = &value[10..];
                let Some((base, compare_head)) = pair.split_once("..") else {
                    return Err("S1_COVERAGE_USAGE --compare=BASE..HEAD expected".to_owned());
                };
                compare = Some((base.to_owned(), compare_head.to_owned()));
            }
            value => return Err(format!("S1_COVERAGE_USAGE unknown argument={value}")),
        }
        index += 1;
    }
    Ok(Args {
        repo,
        head,
        mode,
        compare,
        json,
        output,
    })
}

async fn git_capture(cx: &Cx, repo: &Path, args: &[String]) -> Result<String, String> {
    let mut command = Command::new("git");
    command.arg("-C").arg(repo.to_string_lossy().to_string());
    for arg in args {
        command.arg(arg);
    }
    let output = run_output(cx, command)
        .await
        .map_err(|error| format!("S1_COVERAGE_GIT_ERROR error={error}"))?;
    if !output.status.success() {
        return Err(format!(
            "S1_COVERAGE_GIT_ERROR status={} stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    String::from_utf8(output.stdout)
        .map_err(|error| format!("S1_COVERAGE_GIT_UTF8 error={error}"))
}

fn lines(text: String) -> Vec<String> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn manifest_args(kind: &str, revision: &str) -> Vec<String> {
    let mut args = match kind {
        "tree" => vec!["ls-tree".to_owned(), "-r".to_owned(), "--name-only".to_owned(), revision.to_owned()],
        "index" => vec!["ls-files".to_owned(), "--cached".to_owned()],
        "worktree" => vec!["diff".to_owned(), "--name-only".to_owned(), revision.to_owned()],
        "worktree_only" => vec![
            "ls-files".to_owned(),
            "--others".to_owned(),
            "--exclude-standard".to_owned(),
        ],
        _ => unreachable!(),
    };
    args.push("--".to_owned());
    args.extend(PROVENANCE_PATHS.iter().map(ToString::to_string));
    args
}

fn git_unavailable(error: &str) -> bool {
    let lower = error.to_ascii_lowercase();
    lower.contains("not a git repository")
        || lower.contains("invalid object name")
        || lower.contains("not a valid object name")
}

async fn build_manifest(cx: &Cx, repo: &Path, revision: &str) -> Result<InputManifest, String> {
    let tree = match git_capture(cx, repo, &manifest_args("tree", revision)).await {
        Ok(output) => lines(output),
        Err(error) if revision == "HEAD" && git_unavailable(&error) => {
            let paths = PROVENANCE_PATHS.iter().map(ToString::to_string).collect::<Vec<_>>();
            return Ok(InputManifest::new(revision, paths.clone(), paths, Vec::new(), Vec::new()));
        }
        Err(error) => return Err(error),
    };
    let index = lines(git_capture(cx, repo, &manifest_args("index", revision)).await?);
    let worktree = lines(git_capture(cx, repo, &manifest_args("worktree", revision)).await?);
    let worktree_only = lines(git_capture(cx, repo, &manifest_args("worktree_only", revision)).await?);
    Ok(InputManifest::new(revision, tree, index, worktree, worktree_only))
}

async fn read_tree_file(cx: &Cx, repo: &Path, revision: &str, path: &str) -> Result<String, String> {
    let args = vec!["show".to_owned(), format!("{revision}:{path}")];
    match git_capture(cx, repo, &args).await {
        Ok(text) => Ok(text),
        Err(error) if revision == "HEAD" && git_unavailable(&error) => {
            read_worktree_file(repo, path)
                .map_err(|fallback| format!("S1_COVERAGE_TREE_READ path={path} error={fallback}"))
        }
        Err(error) => Err(format!("S1_COVERAGE_TREE_READ path={path} error={error}")),
    }
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

async fn load_tree(
    cx: &Cx,
    repo: &Path,
    revision: &str,
) -> Result<(CoverageInput, InputManifest), String> {
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
    let manifest = build_manifest(cx, repo, revision).await?;
    Ok((make_input(contracts, s1_toml, beads), manifest))
}

async fn load_worktree(cx: &Cx, repo: &Path, revision: &str) -> Result<(CoverageInput, InputManifest), String> {
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
    let manifest = build_manifest(cx, repo, revision).await?;
    Ok((make_input(contracts, s1_toml, beads), manifest))
}

async fn report_for(cx: &Cx, args: &Args, mode: Mode, revision: &str) -> Result<CoverageReport, String> {
    let (input, manifest) = match mode {
        Mode::Tree => load_tree(cx, &args.repo, revision).await?,
        Mode::Worktree => load_worktree(cx, &args.repo, revision).await?,
        Mode::Both => unreachable!(),
    };
    compute_with_manifest(&input, mode.as_str(), revision, manifest)
        .map_err(|error| error.to_string())
}

fn render_report(report: &CoverageReport, json: bool) -> Result<String, String> {
    if json {
        serde_json::to_string_pretty(report)
            .map_err(|error| format!("S1_COVERAGE_RENDER_JSON error={error}"))
    } else {
        Ok(render_markdown(report))
    }
}

fn render_comparison(comparison: &CoverageComparison, json: bool) -> Result<String, String> {
    if json {
        serde_json::to_string_pretty(comparison)
            .map_err(|error| format!("S1_COVERAGE_RENDER_JSON error={error}"))
    } else {
        Ok(format!(
            "S1_COVERAGE_COMPARISON base={} head={} growth={} closure={} decision={}\n",
            comparison.base_revision,
            comparison.head_revision,
            comparison.growth,
            comparison.closure,
            comparison.decision.as_str()
        ))
    }
}

async fn execute(cx: &Cx, args: &Args) -> Result<String, String> {
    if let Some((base_revision, head_revision)) = &args.compare {
        let base = report_for(cx, args, Mode::Tree, base_revision).await?;
        let head = report_for(cx, args, Mode::Tree, head_revision).await?;
        return render_comparison(&compare_reports(&base, &head), args.json);
    }
    match args.mode {
        Mode::Tree | Mode::Worktree => {
            render_report(&report_for(cx, args, args.mode, &args.head).await?, args.json)
        }
        Mode::Both => {
            let tree = report_for(cx, args, Mode::Tree, &args.head).await?;
            let worktree = report_for(cx, args, Mode::Worktree, &args.head).await?;
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
                    args.head,
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
