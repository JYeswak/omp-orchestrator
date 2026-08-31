#![forbid(unsafe_code)]

use std::{
    env,
    fs::{self, File, OpenOptions, TryLockError},
    io::{self, Write},
    path::{Path, PathBuf},
    process::{Command, ExitCode, Stdio},
};

const EXIT_BUSY: u8 = 75;
const EXIT_NOT_FREE: u8 = 76;
const EXIT_CONFIG: u8 = 78;

#[derive(Debug)]
struct Config {
    state_dir: PathBuf,
    session: String,
    pane: String,
    owner: String,
    ready_probe: PathBuf,
    command: Vec<String>,
}

#[derive(Debug)]
enum AcquireError {
    Busy,
    Io(io::Error),
}

fn valid_component(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn parse_args(args: impl IntoIterator<Item = String>) -> Result<Config, String> {
    let mut args = args.into_iter();
    let mut state_dir = None;
    let mut session = None;
    let mut pane = None;
    let mut owner = None;
    let mut ready_probe = None;
    let mut command = Vec::new();

    while let Some(arg) = args.next() {
        if arg == "--" {
            command.extend(args);
            break;
        }
        let slot = match arg.as_str() {
            "--state-dir" => &mut state_dir,
            "--session" => &mut session,
            "--pane" => &mut pane,
            "--owner" => &mut owner,
            "--ready-probe" => &mut ready_probe,
            _ => return Err(format!("unknown argument: {arg}")),
        };
        *slot = Some(
            args.next()
                .ok_or_else(|| format!("missing value for {arg}"))?,
        );
    }

    let config = Config {
        state_dir: PathBuf::from(state_dir.ok_or("missing --state-dir")?),
        session: session.ok_or("missing --session")?,
        pane: pane.ok_or("missing --pane")?,
        owner: owner.ok_or("missing --owner")?,
        ready_probe: PathBuf::from(ready_probe.ok_or("missing --ready-probe")?),
        command,
    };
    if !config.state_dir.is_absolute() {
        return Err("--state-dir must be absolute".into());
    }
    if !config.ready_probe.is_absolute() {
        return Err("--ready-probe must be absolute".into());
    }
    for (name, value) in [
        ("session", config.session.as_str()),
        ("pane", config.pane.as_str()),
        ("owner", config.owner.as_str()),
    ] {
        if !valid_component(value) {
            return Err(format!("invalid {name}: {value:?}"));
        }
    }
    if config.command.is_empty() {
        return Err("missing command after --".into());
    }
    Ok(config)
}

fn lock_path(state_dir: &Path, session: &str, pane: &str) -> PathBuf {
    state_dir
        .join("pane-dispatch-fence")
        .join(format!("{session}.{pane}.lock"))
}

fn acquire(path: &Path, owner: &str) -> Result<File, AcquireError> {
    let parent = path.parent().ok_or_else(|| {
        AcquireError::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "lock path has no parent",
        ))
    })?;
    fs::create_dir_all(parent).map_err(AcquireError::Io)?;
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(path)
        .map_err(AcquireError::Io)?;
    match file.try_lock() {
        Ok(()) => {}
        Err(TryLockError::WouldBlock) => return Err(AcquireError::Busy),
        Err(TryLockError::Error(error)) => return Err(AcquireError::Io(error)),
    }
    file.set_len(0).map_err(AcquireError::Io)?;
    writeln!(file, "owner={owner} pid={}", std::process::id()).map_err(AcquireError::Io)?;
    file.flush().map_err(AcquireError::Io)?;
    Ok(file)
}

fn run(config: Config) -> u8 {
    let path = lock_path(&config.state_dir, &config.session, &config.pane);
    let lock = match acquire(&path, &config.owner) {
        Ok(lock) => lock,
        Err(AcquireError::Busy) => {
            eprintln!(
                "PANE_DISPATCH_FENCE_BUSY session={} pane={} owner={}",
                config.session, config.pane, config.owner
            );
            return EXIT_BUSY;
        }
        Err(AcquireError::Io(error)) => {
            eprintln!(
                "PANE_DISPATCH_FENCE_ERROR path={} error={error}",
                path.display()
            );
            return EXIT_CONFIG;
        }
    };

    match Command::new(&config.ready_probe)
        .arg(&config.session)
        .arg(format!("--pane={}", config.pane))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(_) => {
            eprintln!(
                "PANE_DISPATCH_FENCE_NOT_FREE session={} pane={} owner={}",
                config.session, config.pane, config.owner
            );
            return EXIT_NOT_FREE;
        }
        Err(error) => {
            eprintln!("PANE_DISPATCH_FENCE_PROBE_ERROR error={error}");
            return EXIT_CONFIG;
        }
    }

    let status = Command::new(&config.command[0])
        .args(&config.command[1..])
        .status();
    drop(lock);
    match status {
        Ok(status) => status
            .code()
            .and_then(|code| u8::try_from(code).ok())
            .unwrap_or(1),
        Err(error) => {
            eprintln!("PANE_DISPATCH_FENCE_CHILD_ERROR error={error}");
            EXIT_CONFIG
        }
    }
}

fn selftest(state_dir: &Path) -> ExitCode {
    let session = format!("selftest-{}", std::process::id());
    let path = lock_path(state_dir, &session, "0");
    let first = match acquire(&path, "selftest-a") {
        Ok(lock) => lock,
        Err(error) => {
            eprintln!("selftest: FAIL — first acquisition failed: {error:?}");
            return ExitCode::FAILURE;
        }
    };
    if !matches!(acquire(&path, "selftest-b"), Err(AcquireError::Busy)) {
        eprintln!("selftest: FAIL — concurrent owner entered the same session:pane");
        return ExitCode::FAILURE;
    }
    drop(first);
    if acquire(&path, "selftest-c").is_err() {
        eprintln!("selftest: FAIL — process-release did not reopen the pane");
        return ExitCode::FAILURE;
    }
    println!("selftest: PASS — concurrent owner refused; process-release reopens pane");
    ExitCode::SUCCESS
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.first().is_some_and(|arg| arg == "--selftest") {
        let state_dir = args
            .windows(2)
            .find(|pair| pair[0] == "--state-dir")
            .map(|pair| PathBuf::from(&pair[1]));
        return match state_dir {
            Some(path) if path.is_absolute() => selftest(&path),
            _ => {
                eprintln!("selftest: FAIL — absolute --state-dir is required");
                ExitCode::from(EXIT_CONFIG)
            }
        };
    }
    match parse_args(args) {
        Ok(config) => ExitCode::from(run(config)),
        Err(error) => {
            eprintln!("pane-dispatch-fence: {error}");
            ExitCode::from(EXIT_CONFIG)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, MutexGuard};

    static TEST_SERIAL: Mutex<()> = Mutex::new(());

    fn test_guard() -> MutexGuard<'static, ()> {
        TEST_SERIAL
            .lock()
            .expect("test mutex should not be poisoned")
    }

    fn test_root(name: &str) -> PathBuf {
        env::temp_dir().join(format!("pane-dispatch-fence-{name}-{}", std::process::id()))
    }

    #[test]
    fn same_pane_is_exclusive_and_drop_releases_it() {
        let _guard = test_guard();
        let path = lock_path(&test_root("exclusive"), "control-plane", "2");
        let first = acquire(&path, "controller").expect("first owner should acquire");
        assert!(matches!(acquire(&path, "arc"), Err(AcquireError::Busy)));
        drop(first);
        acquire(&path, "arc").expect("dropping the process handle should release the pane");
    }

    #[test]
    fn sibling_panes_do_not_contend() {
        let _guard = test_guard();
        let root = test_root("siblings");
        let _first = acquire(&lock_path(&root, "control-plane", "1"), "controller")
            .expect("pane 1 should acquire");
        acquire(&lock_path(&root, "control-plane", "2"), "loop")
            .expect("pane 2 should remain independent");
    }

    #[test]
    fn parser_rejects_path_injection() {
        let _guard = test_guard();
        let args = [
            "--state-dir",
            "/tmp",
            "--session",
            "../other",
            "--pane",
            "2",
            "--owner",
            "test",
            "--ready-probe",
            "/usr/bin/true",
            "--",
            "true",
        ]
        .into_iter()
        .map(str::to_owned);
        assert!(parse_args(args).is_err());
    }

    fn run_config(name: &str, ready_probe: &str) -> Config {
        Config {
            state_dir: test_root(name),
            session: format!("run-{name}"),
            pane: "1".into(),
            owner: "test".into(),
            ready_probe: PathBuf::from(ready_probe),
            command: vec!["/usr/bin/true".into()],
        }
    }

    #[test]
    fn readiness_is_rechecked_while_lock_is_held() {
        let _guard = test_guard();
        assert_eq!(run(run_config("not-free", "/usr/bin/false")), EXIT_NOT_FREE);
    }

    #[test]
    fn ready_probe_allows_child() {
        let _guard = test_guard();
        assert_eq!(run(run_config("free", "/usr/bin/true")), 0);
    }
}
