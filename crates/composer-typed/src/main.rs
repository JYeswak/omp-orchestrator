#![forbid(unsafe_code)]

//! Live composer-typed binary. Verdict is the exit code (0=TYPED, 1=FREE), matching
//! `bin/composer-typed.py`. stdin is the capture; callers check `$?`.

use composer_typed::{is_typed, Rules};
use std::io::{self, Read};
use std::process::ExitCode;

fn main() -> ExitCode {
    let mut mutation = false;
    let mut disabled: Vec<String> = Vec::new();
    let mut selftest = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--selftest" => selftest = true,
            "--mutation" => mutation = true,
            "--disable-rule" => match args.next() {
                Some(v) => disabled.push(v),
                None => {
                    eprintln!("usage error: --disable-rule requires a name");
                    return ExitCode::from(2);
                }
            },
            "-h" | "--help" => {
                eprintln!("usage: composer-typed [--selftest]  # stdin = capture-pane -p -e");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("unknown flag: {other}");
                return ExitCode::from(2);
            }
        }
    }
    if !disabled.is_empty() && !mutation {
        eprintln!("usage error: --disable-rule requires --mutation");
        return ExitCode::from(2);
    }
    let mut rules = Rules::default();
    for name in &disabled {
        if !rules.disable(name) {
            eprintln!("usage error: unknown rule {name}");
            return ExitCode::from(2);
        }
    }
    if selftest {
        return run_selftest(&rules);
    }
    let mut buf = String::new();
    let _ = io::stdin().read_to_string(&mut buf);
    if is_typed(&buf, &rules) {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn run_selftest(rules: &Rules) -> ExitCode {
    let mut fail = 0;
    let esc = "\u{1b}";
    let dim = format!("{esc}[2m");
    let off = format!("{esc}[0m");
    let def = format!("{esc}[39m");
    let agent = "  Opus 5 (1M context) | control-plane";
    let cases = [
        ("bare prompt", format!("{agent}\n{def}❯ {off}"), false),
        (
            "greyed autosuggestion",
            format!("{agent}\n{def}❯ {dim}fix kyzn - repin{off}"),
            false,
        ),
        (
            "typed operator text",
            format!("{agent}\n{def}❯ bought credits - resume the fleet{off}"),
            true,
        ),
        ("plain typed", "❯ hello".to_string(), true),
        ("plain bare", "❯ ".to_string(), false),
        ("empty", String::new(), false),
    ];
    for (label, body, want) in cases {
        let got = is_typed(&body, rules);
        if got == want {
            println!("  [ ok ] {label:<42} typed={got}");
        } else {
            println!("  [FAIL] {label:<42} typed={got} want={want}");
            fail += 1;
        }
    }
    if fail == 0 {
        println!("=== SELFTEST: 0 failure(s) ===");
        ExitCode::SUCCESS
    } else {
        println!("=== SELFTEST: {fail} failure(s) ===");
        ExitCode::from(2)
    }
}
