#![forbid(unsafe_code)]

use oracle_compare::{spawn_timeout, OracleCompareRules, OracleCompareVerdict};
use oracle_pane_state_differential::{diff_sets, parse_ntm_keys, parse_tmux_keys};
use std::collections::BTreeSet;
use std::io::{self, Read};
use std::process::{Command, ExitCode};
use std::time::Duration;

fn main() -> ExitCode {
    let mut json_out = false;
    let mut selftest = false;
    let mut eval_sets = false;
    let mut mutation = false;
    let mut disabled: Vec<String> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--json" => json_out = true,
            "--selftest" => selftest = true,
            "--eval-sets" => eval_sets = true,
            "--mutation" => mutation = true,
            "--disable-rule" => {
                if let Some(v) = args.next() {
                    disabled.push(v);
                }
            }
            "-h" | "--help" => {
                eprintln!("oracle-pane-state-differential [--json] | --selftest");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("oracle-pane-state-differential: unknown flag {other}");
                return ExitCode::from(2);
            }
        }
    }
    let mut rules = OracleCompareRules::default();
    if !disabled.is_empty() && !mutation {
        eprintln!("usage error: --disable-rule requires --mutation");
        return ExitCode::from(2);
    }
    for name in &disabled {
        if !rules.disable(name) {
            eprintln!("usage error: unknown rule {name}");
            return ExitCode::from(2);
        }
    }
    if selftest {
        return run_selftest(&rules);
    }
    if eval_sets {
        let mut buf = String::new();
        let _ = io::stdin().read_to_string(&mut buf);
        // two blocks separated by ---
        let (o, p) = buf.split_once("\n---\n").unwrap_or((buf.as_str(), ""));
        let oracle = parse_tmux_keys(o);
        let product = if p.trim() == "UNREADABLE" {
            Err(())
        } else {
            Ok(parse_tmux_keys(p))
        };
        let (v, only_o, only_p) = diff_sets(oracle, product, &rules);
        emit(json_out, &v, &only_o, &only_p);
        return ExitCode::from(match v {
            OracleCompareVerdict::Agree { .. } => 0,
            OracleCompareVerdict::Disagree { .. } => 1,
            OracleCompareVerdict::Unmeasurable { .. } => 3,
        } as u8);
    }
    run_live(json_out, &rules)
}

fn emit(json_out: bool, v: &OracleCompareVerdict, only_o: &[String], only_p: &[String]) {
    let oracle_n = match v {
        OracleCompareVerdict::Agree { n } => *n,
        OracleCompareVerdict::Disagree { oracle_n, .. } => *oracle_n,
        OracleCompareVerdict::Unmeasurable { .. } => 0,
    };
    let product_n = match v {
        OracleCompareVerdict::Agree { n } => *n,
        OracleCompareVerdict::Disagree { product_n, .. } => *product_n,
        OracleCompareVerdict::Unmeasurable { .. } => 0,
    };
    let only_oracle = only_o.join(" ");
    let only_product = only_p.join(" ");
    match v {
        OracleCompareVerdict::Agree { n } => {
            if json_out {
                println!(
                    r#"{{"schema_version":"1.0.0","oracle":"tmux list-panes -a","product":"ntm --robot-activity","oracle_panes":{n},"product_panes":{n},"only_in_oracle":"","only_in_product":"","code":"PASS","exit_code":0}}"#
                );
            } else {
                println!("PASS\tarms agree\t{n} pane(s)\toracle=tmux product=ntm");
            }
        }
        OracleCompareVerdict::Disagree {
            oracle_n,
            product_n,
        } => {
            if json_out {
                println!(
                    r#"{{"schema_version":"1.0.0","oracle":"tmux list-panes -a","product":"ntm --robot-activity","oracle_panes":{oracle_n},"product_panes":{product_n},"only_in_oracle":"{only_oracle}","only_in_product":"{only_product}","code":"DISAGREEMENT","exit_code":1}}"#
                );
            } else {
                println!("DISAGREEMENT\toracle {oracle_n} pane(s) / product {product_n} pane(s)");
                if !only_o.is_empty() {
                    println!("  ONLY IN ORACLE (product is blind to these): {only_oracle}");
                }
                if !only_p.is_empty() {
                    println!(
                        "  ONLY IN PRODUCT (product reports panes tmux does not have): {only_product}"
                    );
                }
                println!("  A disagreement is a FINDING about the product, never about tmux.");
            }
        }
        OracleCompareVerdict::Unmeasurable { why } if *why == "empty_oracle" => {
            let line = "ORACLE_EMPTY: tmux reported zero panes; a rig that inspected nothing cannot report green";
            println!("{line}");
            eprintln!("{line}");
        }
        OracleCompareVerdict::Unmeasurable { .. } => {
            let line = "PRODUCT_UNASKABLE: could not pose the pane-set question; a rig that could not ask must not report the silence as a finding";
            println!("{line}");
            eprintln!("{line}");
        }
    }
    let _ = (oracle_n, product_n);
}

fn run_live(json_out: bool, rules: &OracleCompareRules) -> ExitCode {
    let tmux = std::env::var("TMUX_BIN").unwrap_or_else(|_| "/opt/homebrew/bin/tmux".into());
    if !std::path::Path::new(&tmux).is_file() {
        eprintln!("ORACLE_UNAVAILABLE: {tmux} is not executable");
        return ExitCode::from(3);
    }
    let mut cmd = Command::new(&tmux);
    cmd.args(["list-panes", "-a", "-F", "#{session_name}:#{pane_index}"]);
    let Some(out) = spawn_timeout(cmd, Duration::from_secs(15)) else {
        eprintln!("ORACLE_UNAVAILABLE: {tmux} is not executable");
        return ExitCode::from(3);
    };
    let oracle = parse_tmux_keys(&String::from_utf8_lossy(&out.stdout));
    if oracle.is_empty() {
        let (v, o, p) = diff_sets(oracle, Ok(BTreeSet::new()), rules);
        emit(json_out, &v, &o, &p);
        return ExitCode::from(3);
    }
    let mut sess_cmd = Command::new(&tmux);
    sess_cmd.args(["list-sessions", "-F", "#{session_name}"]);
    let Some(sessions) = spawn_timeout(sess_cmd, Duration::from_secs(15)) else {
        eprintln!("PRODUCT_UNASKABLE: could not pose the pane-set question to ntm");
        return ExitCode::from(3);
    };
    let mut product = BTreeSet::new();
    for s in String::from_utf8_lossy(&sessions.stdout).lines() {
        let s = s.trim();
        if s.is_empty() {
            continue;
        }
        // MATCH THE SHELL INNER QUERY (franken-harvest ntm-robot-activity-guard):
        // `ntm --robot-activity=$SESSION --robot-format=json` WITHOUT --all.
        // --all would add the user pane to the product arm and shrink only_in_oracle
        // (control-plane:0 is a documented DEFINITIONAL gap). Do not change which
        // pane states disagree. A single unreadable session contributes no keys
        // (original python sys.exit(0)), it does not fail the fleet-wide product arm.
        let mut ntm = Command::new("ntm");
        ntm.arg(format!("--robot-activity={s}"))
            .arg("--robot-format=json");
        let Some(out) = spawn_timeout(ntm, Duration::from_secs(30)) else {
            continue;
        };
        if let Ok(set) = parse_ntm_keys(s, &String::from_utf8_lossy(&out.stdout)) {
            product.extend(set);
        }
    }
    let (v, only_o, only_p) = diff_sets(oracle, Ok(product), rules);
    emit(json_out, &v, &only_o, &only_p);
    ExitCode::from(match v {
        OracleCompareVerdict::Agree { .. } => 0,
        OracleCompareVerdict::Disagree { .. } => 1,
        OracleCompareVerdict::Unmeasurable { .. } => 3,
    } as u8)
}

fn run_selftest(rules: &OracleCompareRules) -> ExitCode {
    let mut fails = 0;
    println!("[selftest] leg 1 — identical sets must AGREE:");
    let a: BTreeSet<_> = ["s:1", "s:2", "s:3"]
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let (v, _, _) = diff_sets(a.clone(), Ok(a.clone()), rules);
    if matches!(v, OracleCompareVerdict::Agree { n: 3 }) {
        println!("[selftest] PASS — agreement detected");
    } else {
        println!("[selftest] FAIL — identical sets reported as disagreeing");
        fails += 1;
    }
    println!("[selftest] leg 2 — a pane MISSING from the product arm must be a FINDING:");
    let b: BTreeSet<_> = ["s:1", "s:2"].into_iter().map(|s| s.to_string()).collect();
    let (v, _, _) = diff_sets(a, Ok(b), rules);
    if matches!(v, OracleCompareVerdict::Disagree { .. }) {
        println!("[selftest] PASS — missing pane surfaced");
    } else {
        println!("[selftest] FAIL — a pane the product cannot see was reported as agreement");
        fails += 1;
    }
    println!("[selftest] leg 3 — an EMPTY product arm against a live oracle must NOT be green:");
    let o: BTreeSet<_> = ["s:1", "s:2"].into_iter().map(|s| s.to_string()).collect();
    let (v, _, _) = diff_sets(o, Ok(BTreeSet::new()), rules);
    if matches!(v, OracleCompareVerdict::Disagree { product_n: 0, .. }) {
        println!("[selftest] PASS — empty product arm refused, not passed");
    } else {
        println!("[selftest] FAIL — empty product arm treated as agreement");
        fails += 1;
    }
    println!("[selftest] leg 4 — an empty ORACLE must ERROR, never PASS (vacuous-green refusal):");
    let (v, _, _) = diff_sets(BTreeSet::new(), Ok(BTreeSet::new()), rules);
    if matches!(
        v,
        OracleCompareVerdict::Unmeasurable {
            why: "empty_oracle"
        }
    ) {
        println!("[selftest] PASS — empty oracle is an error, not a pass");
    } else {
        println!("[selftest] FAIL — vacuous run reported a verdict");
        fails += 1;
    }
    if fails == 0 {
        println!("SELFTEST PASS: agreement, missing-pane, empty-product, and vacuous-oracle legs all fire.");
        ExitCode::SUCCESS
    } else {
        println!("SELFTEST FAIL: {fails} leg(s) failed.");
        ExitCode::from(1)
    }
}
