use std::path::PathBuf;
use std::process::{Command, ExitCode, Stdio};
use std::time::Duration;

const ORACLE_TIMEOUT: Duration = Duration::from_secs(120);
const DIAGNOSTIC_VERBS: [&str; 4] = ["status", "why", "capabilities", "robot-docs"];

pub fn handle(binary: &str, args: &[String]) -> Option<ExitCode> {
    let first = args.first().map(String::as_str)?;
    if first == DIAGNOSTIC_VERBS[0] {
        Some(status(binary, args))
    } else if first == DIAGNOSTIC_VERBS[1] {
        Some(why(binary, args))
    } else if first == DIAGNOSTIC_VERBS[2] {
        Some(capabilities(binary))
    } else if first == DIAGNOSTIC_VERBS[3] {
        Some(robot_docs(binary, args))
    } else {
        None
    }
}

fn oracle_output() -> Result<String, String> {
    let oracle = std::env::var_os("DISPATCH_STALL_ORACLE")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("CONTROL_PLANE_REPO").map(|root| PathBuf::from(root).join("bin/dispatch-stall-profile.sh")))
        .unwrap_or_else(|| PathBuf::from("dispatch-stall-profile"));
    let mut command = Command::new("/bin/bash");
    command
        .arg(oracle)
        .arg("--check")
        .stdin(Stdio::null());

    // Use the shared bounded runner: it drains both pipes concurrently, kills the
    // entire process group on deadline, and keeps timeout distinct from output.
    match subprocess_contract::bounded_output(&mut command, ORACLE_TIMEOUT) {
        subprocess_contract::BoundedOutcome::Completed(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
            if stdout.trim().is_empty() {
                return Err(format!(
                    "oracle produced no output (rc={:?}{})",
                    output.status.code(),
                    if stderr.is_empty() {
                        String::new()
                    } else {
                        format!(", stderr={stderr}")
                    }
                ));
            }
            Ok(stdout)
        }
        subprocess_contract::BoundedOutcome::TimedOut => Err(format!(
            "oracle timeout after {}s (process group signalled; no output verdict)",
            ORACLE_TIMEOUT.as_secs()
        )),
        subprocess_contract::BoundedOutcome::Unspawned(error) => {
            Err(format!("spawn oracle: {error}"))
        }
    }
}
#[derive(Default)]
struct Fields {
    verdict: String,
    load: String,
    stage_bound: String,
    queue: String,
    free_panes: String,
    verdict_age: String,
    published: String,
    red: String,
    live: String,
}

fn lock_path(binary: &str) -> String {
    match binary {
        "fast-dispatch" => std::env::var("FD_LOCK_FILE")
            .unwrap_or_else(|_| format!("{}/.local/state/flywheel/fast-dispatch.lock", home())),
        "controller-tick" => std::env::var("CT_LOCK_FILE").unwrap_or_else(|_| {
            format!("{}/.local/state/flywheel/controller-tick.run.lock", home())
        }),
        "fleet-monitor" => std::env::var("FLEET_LOCK_FILE")
            .unwrap_or_else(|_| format!("{}/.local/state/flywheel/fleet-monitor.run.lock", home())),
        "loop-driver" => std::env::var("LOOP_DRIVER_LOCK")
            .unwrap_or_else(|_| "/tmp/control-plane-loop-driver.lock".into()),
        _ => String::new(),
    }
}

fn home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| ".".into())
}

fn holder_snapshot(binary: &str) -> (String, String, String) {
    let path = lock_path(binary);
    if path.is_empty() {
        return ("none".into(), "none".into(), "none".into());
    }
    let marker = format!("{path}.d/pid");
    let candidate = if std::path::Path::new(&marker).exists() {
        marker.clone()
    } else {
        path.clone()
    };
    let marked_pid = if binary == "fast-dispatch" {
        std::fs::read_to_string(&marker)
            .ok()
            .and_then(|value| value.trim().parse::<u32>().ok())
            .map(|pid| pid.to_string())
    } else {
        None
    };
    let pid = marked_pid.or_else(|| {
        let output = Command::new("/usr/sbin/lsof")
            .args(["-t", &candidate])
            .output()
            .ok()?;
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .find(|line| line.trim().parse::<u32>().is_ok())
            .map(|line| line.trim().to_owned())
    });
    let Some(pid) = pid else {
        return ("none".into(), "none".into(), "none".into());
    };
    let Ok(ps) = Command::new("/bin/ps")
        .args(["-o", "etime=", "-o", "command=", "-p", &pid])
        .output()
    else {
        return (pid, "unknown".into(), "unknown".into());
    };
    let detail = String::from_utf8_lossy(&ps.stdout).trim().to_owned();
    if detail.is_empty() {
        return ("none".into(), "none".into(), "none".into());
    }
    let mut parts = detail.splitn(2, char::is_whitespace);
    let elapsed = parts.next().unwrap_or("unknown").to_owned();
    let argv = parts.next().unwrap_or("unknown").trim().to_owned();
    (
        pid,
        elapsed,
        if argv.is_empty() {
            "unknown".into()
        } else {
            argv
        },
    )
}

fn parse_fields(output: &str) -> Fields {
    let mut fields = Fields::default();
    let line = output.lines().next().unwrap_or_default();
    for token in line.split_whitespace() {
        let Some((key, value)) = token.split_once('=') else {
            continue;
        };
        match key {
            "verdict" => fields.verdict = value.to_owned(),
            "load" => fields.load = value.to_owned(),
            "stage_bound" => fields.stage_bound = value.trim_end_matches('s').to_owned(),
            "queue" => fields.queue = value.to_owned(),
            "free_panes" => fields.free_panes = value.to_owned(),
            "verdict_age" => fields.verdict_age = value.trim_end_matches('s').to_owned(),
            "published" => fields.published = value.to_owned(),
            "red" => fields.red = value.to_owned(),
            "live" => fields.live = value.to_owned(),
            _ => {}
        }
    }
    fields
}

fn status(binary: &str, args: &[String]) -> ExitCode {
    let json = args.iter().any(|arg| arg == "--json");
    match oracle_output() {
        Ok(output) => {
            let f = parse_fields(&output);
            let (holder_pid, holder_elapsed, holder_argv) = holder_snapshot(binary);
            if json {
                println!(
                    "{{\"schema\":\"dispatch.status.v1\",\"binary\":\"{}\",\"verdict\":\"{}\",\"admitted\":{},\"load\":{},\"stage_bound_s\":{},\"queue_depth\":{},\"free_panes\":{},\"verdict_age_s\":{},\"published_overall\":\"{}\",\"published_red\":\"{}\",\"live_gate\":\"{}\",\"lock_path\":\"{}\",\"holder_pid\":\"{}\",\"holder_elapsed\":\"{}\",\"holder_argv\":\"{}\"}}",
                    escape(binary),
                    escape(&f.verdict),
                    if f.verdict == "DISPATCHABLE" { "true" } else { "false" },
                    number(&f.load),
                    number(&f.stage_bound),
                    number(&f.queue),
                    number(&f.free_panes),
                    number(&f.verdict_age),
                    escape(&f.published),
                    escape(&f.red),
                    escape(&f.live),
                    escape(&lock_path(binary)),
                    escape(&holder_pid),
                    escape(&holder_elapsed),
                    escape(&holder_argv),
                );
            } else {
                println!(
                    "DISPATCH-STATUS binary={binary} admitted={} verdict={} load={} stage_bound={}s queue={} free_panes={} verdict_age={}s published={} red={} live={} lock={} holder_pid={} holder_elapsed={} holder_argv={}",
                    if f.verdict == "DISPATCHABLE" { "true" } else { "false" },
                    f.verdict, f.load, f.stage_bound, f.queue, f.free_panes, f.verdict_age,
                    f.published, f.red, f.live, lock_path(binary), holder_pid, holder_elapsed, holder_argv
                );
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("dispatch status failed: {error}; retry with `{binary} status --json`");
            ExitCode::from(3)
        }
    }
}

fn why(binary: &str, args: &[String]) -> ExitCode {
    let json = args.iter().any(|arg| arg == "--json");
    match oracle_output() {
        Ok(output) => {
            let fields = parse_fields(&output);
            let (holder_pid, holder_elapsed, holder_argv) = holder_snapshot(binary);
            if json {
                println!(
                    "{{\"schema\":\"dispatch.why.v1\",\"binary\":\"{}\",\"verdict\":\"{}\",\"published_red\":\"{}\",\"live_gate\":\"{}\",\"lock_path\":\"{}\",\"holder_pid\":\"{}\",\"holder_elapsed\":\"{}\",\"holder_argv\":\"{}\",\"evidence\":\"{}\"}}",
                    escape(binary), escape(&fields.verdict), escape(&fields.red), escape(&fields.live),
                    escape(&lock_path(binary)), escape(&holder_pid), escape(&holder_elapsed),
                    escape(&holder_argv), escape(&output),
                );
            } else {
                println!("DISPATCH-WHY binary={binary} verdict={} published_red={} live_gate={} holder_pid={holder_pid} holder_elapsed={holder_elapsed} holder_argv={holder_argv}", fields.verdict, fields.red, fields.live);
                print!("{output}");
                if !output.ends_with('\n') {
                    println!();
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("dispatch why failed: {error}; retry with `{binary} status --json`");
            ExitCode::from(3)
        }
    }
}
fn capabilities(binary: &str) -> ExitCode {
    let verbs = DIAGNOSTIC_VERBS
        .iter()
        .map(|name| match *name {
            "status" | "why" | "capabilities" => {
                format!("{{\"name\":\"{name}\",\"flags\":[\"--json\"]}}")
            }
            "robot-docs" => format!("{{\"name\":\"{name}\",\"args\":[\"guide\"]}}"),
            _ => unreachable!("all diagnostic verbs are described above"),
        })
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"schema\":\"dispatch.capabilities.v1\",\"binary\":\"{}\",\"read_only\":true,\"verbs\":[{}],\"exit_codes\":{{\"0\":\"success\",\"2\":\"usage error\",\"3\":\"diagnostic oracle unavailable or timed out\"}},\"oracle\":\"bin/dispatch-stall-profile.sh\"}}",
        escape(binary),
        verbs
    );
    ExitCode::SUCCESS
}

fn robot_docs(binary: &str, args: &[String]) -> ExitCode {
    if args.get(1).map(String::as_str) != Some("guide") && args.len() > 1 {
        eprintln!("usage error: `{binary} robot-docs guide`");
        return ExitCode::from(2);
    }
    println!(
        "{binary} diagnostic guide\n\n  {binary} status --json   current dispatch verdict, queue, panes, age, and gate\n  {binary} why              evidence chain and the remedy for the current refusal\n  {binary} capabilities --json   machine-readable verbs and exit codes\n\nRead-only diagnostics. `status` and `why` use bin/dispatch-stall-profile.sh as their differential oracle.\n"
    );
    ExitCode::SUCCESS
}

fn escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn number(value: &str) -> String {
    if value.parse::<f64>().is_ok() {
        value.to_owned()
    } else {
        "null".to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_fields, Fields};

    #[test]
    fn parses_stall_profile_contract() {
        let fields = parse_fields("DISPATCH-STALL verdict=REPAIRED_BUT_UNPUBLISHED load=2 stage_bound=620s queue=7 free_panes=1 verdict_age=1800s published=RED red=docs-staleness live=PASS\n");
        assert_eq!(fields.verdict, "REPAIRED_BUT_UNPUBLISHED");
        assert_eq!(fields.stage_bound, "620");
        assert_eq!(fields.queue, "7");
        assert_eq!(fields.free_panes, "1");
        assert_eq!(fields.verdict_age, "1800");
        assert_eq!(fields.live, "PASS");
    }

    #[test]
    fn malformed_numbers_are_not_emitted_as_json_numbers() {
        let fields = Fields {
            queue: "wat".into(),
            ..Fields::default()
        };
        assert!(super::number(&fields.queue) == "null");
    }
}
