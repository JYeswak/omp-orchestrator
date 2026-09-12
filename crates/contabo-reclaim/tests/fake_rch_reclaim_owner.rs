#![forbid(unsafe_code)]

//! Test-only fake owner executable. NEVER installed and never on any real
//! PATH: the reclaimer suite copies it under a sandbox name and points a
//! sandbox-only PATH at it. Pure Rust, no shell: behavior comes from argv
//! plus three env inputs, and any misconfiguration refuses nonzero rather
//! than emitting bytes that could read as a valid owner answer.
//!
//! Contract: `fake-rch-reclaim-owner <report|apply|status|cancel> <id>`
//! appends `verb\nid\n` to `$FAKE_ARGV_LOG` (when set), prints exactly
//! `$FAKE_OWNER_RESPONSE` (when set), and exits `$FAKE_OWNER_EXIT` (default 0).

fn fail(message: &str) -> std::process::ExitCode {
    eprintln!("fake-rch-reclaim-owner: {message}");
    std::process::ExitCode::from(99)
}

fn main() -> std::process::ExitCode {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let [verb, request_id] = argv.as_slice() else {
        return fail("expected exactly [verb, request-id]");
    };
    if verb != "report" && verb != "apply" && verb != "status" && verb != "cancel" {
        return fail("unknown verb; expected report|apply|status|cancel");
    }
    if let Some(log) = std::env::var_os("FAKE_ARGV_LOG") {
        let line = format!("{verb}\n{request_id}\n");
        if std::fs::write(log, line).is_err() {
            return fail("argv log unwritable");
        }
    }
    let body = match std::env::var("FAKE_OWNER_RESPONSE") {
        Ok(body) => body,
        Err(_) => return fail("FAKE_OWNER_RESPONSE unset"),
    };
    let exit: u8 = std::env::var("FAKE_OWNER_EXIT")
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    print!("{body}");
    std::process::ExitCode::from(exit)
}
