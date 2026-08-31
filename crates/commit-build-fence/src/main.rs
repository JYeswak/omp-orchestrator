#![forbid(unsafe_code)]

//! Pre-commit entry point for the in-flight build fence.
//!
//! With no subcommand, this is the hook operation: check the explicit
//! registration store and exit 1 on an active matching build or 2 when the
//! store cannot be trusted. `--no-verify` remains an intentional bypass.

use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use commit_build_fence::{
    check, BuildRegistration, FenceVerdict, RegistrationStore, StoreError, DEFAULT_TTL_SECS,
};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

const USAGE: &str = "usage: commit-build-fence [check|init|register|release] [options]";

#[derive(Debug)]
enum Outcome {
    Allow,
    Refused(String),
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("COMMIT_FENCE_ERROR reason=runtime_build detail={error}");
            return ExitCode::from(2);
        }
    };
    let result = runtime.block_on(async move {
        let cx = Cx::current()
            .ok_or_else(|| "COMMIT_FENCE_ERROR reason=no_runtime_context".to_owned())?;
        run(&cx, &args).await
    });
    match result {
        Ok(Outcome::Allow) => ExitCode::SUCCESS,
        Ok(Outcome::Refused(detail)) => {
            eprintln!("{detail}");
            ExitCode::from(1)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(2)
        }
    }
}

async fn run(cx: &Cx, args: &[String]) -> Result<Outcome, String> {
    cx.checkpoint()
        .map_err(|_| "COMMIT_FENCE_ERROR reason=cancelled".to_owned())?;
    let command = args.first().map(String::as_str).unwrap_or("check");
    let outcome = match command {
        "check" => run_check(args).map_err(|error| error.to_string()),
        "init" => run_init(args).map_err(|error| error.to_string()),
        "register" => run_register(args).map_err(|error| error.to_string()),
        "release" => run_release(args).map_err(|error| error.to_string()),
        "--help" | "-h" => {
            println!("{USAGE}");
            Ok(Outcome::Allow)
        }
        other => Err(format!(
            "COMMIT_FENCE_ERROR reason=unknown_command command={other}\n{USAGE}"
        )),
    }?;
    cx.checkpoint()
        .map_err(|_| "COMMIT_FENCE_ERROR reason=cancelled".to_owned())?;
    Ok(outcome)
}

fn run_check(args: &[String]) -> Result<Outcome, StoreError> {
    let repo = repo_path(args)?;
    let store = store_path(args, &repo)?;
    let current_head = head_for(args, &repo, &store)?;
    let now = option(args, "--now")?
        .map(|value| parse_u64("--now", &value))
        .transpose()
        .map_err(|detail| StoreError::Invalid {
            path: store.clone(),
            detail,
        })?
        .unwrap_or_else(now_unix);
    match check(&store, &repo.display().to_string(), &current_head, now)? {
        FenceVerdict::Clear => Ok(Outcome::Allow),
        FenceVerdict::Refused {
            registration,
            current_head,
        } => Ok(Outcome::Refused(format!(
            "COMMIT_FENCE_REFUSED repo={} build_id={} registered_head={} current_head={} holder={} expires_at_unix={}",
            registration.repo,
            registration.build_id,
            registration.head,
            current_head,
            registration.holder,
            registration.expires_at_unix
        ))),
    }
}

fn run_init(args: &[String]) -> Result<Outcome, StoreError> {
    let repo = repo_path(args)?;
    let store = store_path(args, &repo)?;
    RegistrationStore::init(&store)?;
    println!("COMMIT_FENCE_STORE_READY path={}", store.display());
    Ok(Outcome::Allow)
}

fn run_register(args: &[String]) -> Result<Outcome, StoreError> {
    let repo = repo_path(args)?;
    let store_path = store_path(args, &repo)?;
    let build_id = required_option(args, "--build-id")?;
    let holder = option(args, "--holder")?
        .or_else(|| env::var("OMP_BUILD_HOLDER").ok())
        .unwrap_or_else(|| format!("pid:{}", std::process::id()));
    let head = head_for(args, &repo, &store_path)?;
    let started_at_unix = option(args, "--now")?
        .map(|value| parse_u64("--now", &value))
        .transpose()
        .map_err(|detail| StoreError::Invalid {
            path: store_path.clone(),
            detail,
        })?
        .unwrap_or_else(now_unix);
    let ttl_secs = option(args, "--ttl-secs")?
        .map(|value| parse_u64("--ttl-secs", &value))
        .transpose()
        .map_err(|detail| StoreError::Invalid {
            path: store_path.clone(),
            detail,
        })?
        .unwrap_or(DEFAULT_TTL_SECS);
    let expires_at_unix =
        started_at_unix
            .checked_add(ttl_secs)
            .ok_or_else(|| StoreError::Invalid {
                path: store_path.clone(),
                detail: "registration TTL overflows unix time".to_owned(),
            })?;
    let registration = BuildRegistration {
        build_id,
        repo: repo.display().to_string(),
        head,
        holder,
        started_at_unix,
        expires_at_unix,
    };
    let mut store = RegistrationStore::load(&store_path)?;
    store.register(registration.clone())?;
    store.save_atomic(&store_path)?;
    println!(
        "COMMIT_FENCE_REGISTERED repo={} build_id={} head={} holder={} expires_at_unix={}",
        registration.repo,
        registration.build_id,
        registration.head,
        registration.holder,
        registration.expires_at_unix
    );
    Ok(Outcome::Allow)
}

fn run_release(args: &[String]) -> Result<Outcome, StoreError> {
    let repo = repo_path(args)?;
    let store_path = store_path(args, &repo)?;
    let build_id = required_option(args, "--build-id")?;
    let holder = option(args, "--holder")?
        .or_else(|| env::var("OMP_BUILD_HOLDER").ok())
        .unwrap_or_else(|| format!("pid:{}", std::process::id()));
    let released_at_unix = option(args, "--now")?
        .map(|value| parse_u64("--now", &value))
        .transpose()
        .map_err(|detail| StoreError::Invalid {
            path: store_path.clone(),
            detail,
        })?
        .unwrap_or_else(now_unix);
    let mut store = RegistrationStore::load(&store_path)?;
    let event = store.release(
        &build_id,
        &repo.display().to_string(),
        &holder,
        released_at_unix,
    )?;
    store.save_atomic(&store_path)?;
    println!(
        "COMMIT_FENCE_RELEASED repo={} build_id={} holder={} released_at_unix={}",
        event.repo, event.build_id, event.holder, event.released_at_unix
    );
    Ok(Outcome::Allow)
}

fn repo_path(args: &[String]) -> Result<PathBuf, StoreError> {
    let raw = option(args, "--repo")?
        .or_else(|| env::var("OMP_REPO").ok())
        .map(PathBuf::from)
        .or_else(|| env::current_dir().ok())
        .ok_or_else(|| StoreError::Invalid {
            path: PathBuf::from("."),
            detail: "repository path is unavailable".to_owned(),
        })?;
    fs::canonicalize(&raw).map_err(|error| StoreError::Io {
        path: raw,
        operation: "canonicalize repository".to_owned(),
        detail: error.to_string(),
    })
}

fn store_path(args: &[String], repo: &Path) -> Result<PathBuf, StoreError> {
    if let Some(path) = option(args, "--store")? {
        return Ok(PathBuf::from(path));
    }
    if let Some(path) = env::var_os("OMP_BUILD_REGISTRATION") {
        return Ok(PathBuf::from(path));
    }
    Ok(git_dir(repo)?.join("omp-build-registration.json"))
}

fn head_for(args: &[String], repo: &Path, store_path: &Path) -> Result<String, StoreError> {
    if let Some(head) = option(args, "--head")?.or_else(|| env::var("OMP_HEAD").ok()) {
        return Ok(head);
    }
    read_head(repo).map_err(|detail| StoreError::Io {
        path: repo.join(".git/HEAD"),
        operation: "read HEAD".to_owned(),
        detail: format!("{detail} (store={})", store_path.display()),
    })
}
fn git_dir(repo: &Path) -> Result<PathBuf, StoreError> {
    let dot_git = repo.join(".git");
    if dot_git.is_dir() {
        return Ok(dot_git);
    }
    let text = fs::read_to_string(&dot_git).map_err(|error| StoreError::Io {
        path: dot_git.clone(),
        operation: "read git directory".to_owned(),
        detail: error.to_string(),
    })?;
    let value = text
        .lines()
        .find_map(|line| line.strip_prefix("gitdir: "))
        .ok_or_else(|| StoreError::Invalid {
            path: dot_git,
            detail: "git directory file has no gitdir entry".to_owned(),
        })?;
    let path = PathBuf::from(value);
    Ok(if path.is_absolute() {
        path
    } else {
        repo.join(path)
    })
}

fn read_head(repo: &Path) -> Result<String, String> {
    let git_dir = git_dir(repo).map_err(|error| error.to_string())?;
    let head =
        fs::read_to_string(git_dir.join("HEAD")).map_err(|error| format!("read HEAD: {error}"))?;
    let head = head.trim();
    if let Some(reference) = head.strip_prefix("ref: ") {
        let ref_path = git_dir.join(reference);
        if let Ok(value) = fs::read_to_string(&ref_path) {
            return Ok(value.trim().to_owned());
        }
        let packed = fs::read_to_string(git_dir.join("packed-refs"))
            .map_err(|error| format!("read packed refs: {error}"))?;
        if let Some(value) = packed.lines().find_map(|line| {
            let (sha, name) = line.split_once(' ')?;
            (name == reference).then_some(sha.to_owned())
        }) {
            return Ok(value);
        }
        Err(format!("reference {reference} is unresolved"))
    } else if head.is_empty() {
        Err("HEAD is empty".to_owned())
    } else {
        Ok(head.to_owned())
    }
}

fn option(args: &[String], flag: &str) -> Result<Option<String>, StoreError> {
    for (index, argument) in args.iter().enumerate() {
        if argument == flag {
            return args
                .get(index + 1)
                .cloned()
                .ok_or_else(|| StoreError::Invalid {
                    path: PathBuf::from("."),
                    detail: format!("{flag} requires a value"),
                })
                .map(Some);
        }
        if let Some(value) = argument.strip_prefix(&format!("{flag}=")) {
            return Ok(Some(value.to_owned()));
        }
    }
    Ok(None)
}

fn required_option(args: &[String], flag: &str) -> Result<String, StoreError> {
    option(args, flag)?.ok_or_else(|| StoreError::Invalid {
        path: PathBuf::from("."),
        detail: format!("{flag} is required"),
    })
}

fn parse_u64(flag: &str, value: &str) -> Result<u64, String> {
    value
        .parse()
        .map_err(|error| format!("{flag} must be an unsigned integer: {error}"))
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}
