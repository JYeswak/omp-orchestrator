#![forbid(unsafe_code)]

//! Live binary. Verdicts go to STDOUT at column 0 in both directions.
//! stderr is usage errors only. The live path always exits 0 (the oracle
//! reports; it does not gate). `--disable-rule` is a mutation-harness
//! affordance and is refused without `--mutation`.

use std::process::ExitCode;
use verify_dispatch::{run_live, VerifyDispatchConfig, VerifyDispatchRules};

fn main() -> ExitCode {
    let mut mutation = false;
    let mut disabled: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--mutation" => mutation = true,
            "--disable-rule" => match args.next() {
                Some(v) => disabled.push(v),
                None => {
                    eprintln!("usage error: --disable-rule requires a name");
                    return ExitCode::from(2);
                }
            },
            // The oracle ignores argv. Unknown flags must not become a silent
            // behaviour change on the live path (controller-tick passes none).
            _ => {}
        }
    }
    if !disabled.is_empty() && !mutation {
        eprintln!("usage error: --disable-rule requires --mutation");
        return ExitCode::from(2);
    }

    let mut cfg = match VerifyDispatchConfig::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::from(2);
        }
    };
    for name in &disabled {
        if !cfg.rules.disable(name) {
            eprintln!(
                "usage error: unknown rule {name}; known: {}",
                VerifyDispatchRules::known_names_csv()
            );
            return ExitCode::from(2);
        }
    }

    let out = run_live(&cfg);
    print!("{}", out.stdout);
    ExitCode::from(out.code as u8)
}
