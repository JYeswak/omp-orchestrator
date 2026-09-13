//! Fake worker-side transport for retire legs (bead 4ftow): one checked-in
//! Rust binary staged under BOTH the `rch` and `ssh` names in a sandbox-only
//! PATH, dispatching on its own argv[0] file name. No shell runs at any
//! point; no network is touched.
//!
//! Contract, deliberately narrow (it emulates only what `retire_export`
//! invokes; anything else exits 2 loudly so a new dependency is visible):
//! - `rch status --json` prints `$FAKE_RCH_STATUS` verbatim (the control
//!   snapshot the guard reads).
//! - `ssh <target> find <base> -mindepth 1 -maxdepth 1 -name <glob>` lists
//!   matching entries under the box root as `'<p>\t<y>\t<s>\n'` rows, with
//!   `y` in {d, l} (symlink_metadata: symlink reads as a link even when its
//!   target is gone) and `s` always 0 (retire takes honest bytes from du).
//! - `ssh <target> test -e -- <p>` exits 0/1 on presence.
//! - `ssh <target> rm -rf -- <p>` removes a file, symlink, or tree; missing
//!   reads as success, like real `rm -rf`.
//! - `ssh <target> pgrep -a -f <pat>` prints `$FAKE_PGREP_LINES` (default
//!   empty); exit 0 iff non-empty, mirroring real pgrep's no-match exit 1.
//! - `ssh <target> realpath -e -- <p>` prints the canonical path; a missing
//!   target exits 1.
//! - `ssh <target> du -sk -- <p>` prints walked bytes rounded UP to KiB.
//!
//! Box-absolute paths map under `$FAKE_BOX_ROOT` by stripping the leading
//! `/`. Both env vars are mandatory: an unconfigured fake refuses rather
//! than emulating an empty box (which would read as "nothing to dispose").

#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

fn box_root() -> PathBuf {
    std::env::var_os("FAKE_BOX_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("fake-box-transport: FAKE_BOX_ROOT is not set");
            std::process::exit(2);
        })
}

fn on_box(path: &str) -> PathBuf {
    box_root().join(path.trim_start_matches('/'))
}

fn glob_match(pattern: &str, name: &str) -> bool {
    // Minimal `*` glob: enough for `.rch-target*` and exact names. Anything
    // fancier is out of scope and refuses loudly at the call site, never
    // silently matches everything.
    if !pattern.contains('*') {
        return pattern == name;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.iter().any(|part| part.contains(['?', '[', ']'])) {
        eprintln!("fake-box-transport: unsupported glob {pattern:?}");
        std::process::exit(2);
    }
    let mut rest = name;
    if !parts[0].is_empty() {
        if let Some(tail) = rest.strip_prefix(parts[0]) {
            rest = tail;
        } else {
            return false;
        }
    }
    for part in &parts[1..parts.len() - 1] {
        if let Some(index) = rest.find(*part) {
            rest = &rest[index + part.len()..];
        } else {
            return false;
        }
    }
    let last = parts.last().unwrap_or(&"");
    if last.is_empty() {
        true
    } else {
        rest.ends_with(last)
    }
}

fn cmd_find(base: &str, pattern: &str) {
    let dir = on_box(base);
    let entries = std::fs::read_dir(&dir).unwrap_or_else(|error| {
        eprintln!("fake-box-transport: find cannot list {base}: {error}");
        std::process::exit(1);
    });
    let mut names: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if glob_match(pattern, &name) {
            let kind = match entry.file_type() {
                Ok(kind) if kind.is_symlink() => "l",
                Ok(kind) if kind.is_dir() => "d",
                _ => "f",
            };
            names.push(format!("{base}/{name}\t{kind}\t0"));
        }
    }
    names.sort();
    // No surrounding quotes: the `-printf "'%p...'"` quotes in the real
    // command are shell quoting consumed remotely, not output text.
    for line in names {
        println!("{line}");
    }
}

fn walk_bytes(path: &Path) -> u64 {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(next) = stack.pop() {
        let Ok(meta) = std::fs::symlink_metadata(&next) else {
            continue;
        };
        if meta.is_dir() && !meta.is_symlink() {
            if let Ok(entries) = std::fs::read_dir(&next) {
                stack.extend(entries.flatten().map(|entry| entry.path()));
            }
        } else if meta.is_file() {
            total = total.saturating_add(meta.len());
        }
    }
    total
}

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    let me = Path::new(&argv[0])
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if me == "rch" {
        match std::env::var("FAKE_RCH_STATUS") {
            Ok(status) => print!("{status}"),
            Err(_) => {
                eprintln!("fake-box-transport: FAKE_RCH_STATUS is not set");
                std::process::exit(2);
            }
        }
        return;
    }
    if me != "ssh" {
        eprintln!("fake-box-transport: staged under an unknown name {me:?}; want rch or ssh");
        std::process::exit(2);
    }
    // ssh [-o ...] <target> <command...>: the command starts at the first
    // arg that is not a flag pair and not the user@host target.
    let mut index = 1;
    while index < argv.len() {
        if argv[index] == "-o" {
            index += 2;
        } else {
            break;
        }
    }
    if index < argv.len() && argv[index].contains('@') {
        index += 1;
    }
    let command: &[String] = &argv[index..];
    let Some(verb) = command.first().map(String::as_str) else {
        eprintln!("fake-box-transport: ssh with no remote command");
        std::process::exit(2);
    };
    match verb {
        "find" => {
            // find <base> -mindepth 1 -maxdepth 1 -name <glob> [-printf ...]
            let base = command.get(1).cloned().unwrap_or_default();
            let pattern = command
                .windows(2)
                .find(|pair| pair[0] == "-name")
                .map(|pair| pair[1].clone())
                .unwrap_or_else(|| {
                    eprintln!("fake-box-transport: find without -name");
                    std::process::exit(2);
                });
            cmd_find(&base, &pattern);
        }
        "test" => {
            let path = command.last().cloned().unwrap_or_default();
            std::process::exit(i32::from(!on_box(&path).exists()));
        }
        "rm" => {
            let path = command.last().cloned().unwrap_or_default();
            let target = on_box(&path);
            let result = match std::fs::symlink_metadata(&target) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(error),
                Ok(meta) => {
                    if meta.is_dir() && !meta.is_symlink() {
                        std::fs::remove_dir_all(&target)
                    } else {
                        std::fs::remove_file(&target)
                    }
                }
            };
            if let Err(error) = result {
                eprintln!("fake-box-transport: rm failed: {error}");
                std::process::exit(1);
            }
        }
        "pgrep" => {
            let lines = std::env::var("FAKE_PGREP_LINES").unwrap_or_default();
            if lines.trim().is_empty() {
                std::process::exit(1);
            }
            print!("{lines}");
            if !lines.ends_with('\n') {
                println!();
            }
        }
        "realpath" => {
            let path = command.last().cloned().unwrap_or_default();
            match on_box(&path).canonicalize() {
                Ok(resolved) => println!("{}", resolved.display()),
                Err(_) => std::process::exit(1),
            }
        }
        "du" => {
            let path = command.last().cloned().unwrap_or_default();
            let kb = walk_bytes(&on_box(&path)).div_ceil(1024);
            println!("{kb}\t{path}");
        }
        other => {
            eprintln!("fake-box-transport: unsupported remote command {other:?}");
            std::process::exit(2);
        }
    }
}
