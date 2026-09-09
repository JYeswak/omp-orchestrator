#![forbid(unsafe_code)]

use asupersync::runtime::RuntimeBuilder;
use asupersync::Cx;
use bead_availability::{
    classify, collect_live, collect_ready_live, measure_family, parse_graph_json,
    parse_issues_jsonl, parse_r7_parent_child_jsonl, R7_DEPENDENCY_SOURCE, R7_S0_EPIC,
    ScanError, FAMILY_NEEDLES, NO_CLAIM,
};
use std::env;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

#[used]
static BUILD_ID_MARKER: &[u8] = concat!("build_id=", env!("OMP_BUILD_ID")).as_bytes();

fn usage() -> &'static str {
    "usage: bead-availability [--json] [--br PATH] [--readiness|--version]\n\
            bead-availability --graph PATH.json\n\
            bead-availability --definition-quality [--issues PATH]\n\
            exit --definition-quality: 0 at-or-above floor, 1 below-floor, 2 vacuous/unreadable\n\
            readiness: 0 = no projection residual, 1 = residual, 2 = instrument error, 3 = anti-vacuity"
}


fn main() -> ExitCode {
    let mut json = false;
    let mut br_program = String::from("br");
    let mut graph: Option<String> = None;
    let mut definition_quality = false;
    let mut readiness = false;
    let mut issues_path: Option<String> = None;
    let args = env::args().skip(1).collect::<Vec<_>>();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--json" => json = true,
            "--definition-quality" => definition_quality = true,
            "--readiness" => readiness = true,
            "--version" => {
                println!("bead-availability 0.1.0 build_id={}", env!("OMP_BUILD_ID"));
                return ExitCode::SUCCESS;
            }
            "--help" | "-h" => {
                println!("{}", usage());
                return ExitCode::SUCCESS;
            }
            value if value == "--graph" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    eprintln!("BEAD_AVAILABILITY_USAGE missing value for --graph");
                    return ExitCode::from(2);
                };
                graph = Some(path.clone());
            }
            value if value.starts_with("--graph=") => {
                graph = Some(value[8..].to_owned());
            }
            value if value == "--issues" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    eprintln!("BEAD_AVAILABILITY_USAGE missing value for --issues");
                    return ExitCode::from(2);
                };
                issues_path = Some(path.clone());
            }
            value if value.starts_with("--issues=") => {
                issues_path = Some(value[9..].to_owned());
            }
            value if value == "--br" => {
                index += 1;
                let Some(path) = args.get(index) else {
                    eprintln!("BEAD_AVAILABILITY_USAGE missing value for --br");
                    return ExitCode::from(2);
                };
                br_program.clone_from(path);
            }
            value if value.starts_with("--br=") => {
                br_program = value[5..].to_owned();
            }
            value => {
                eprintln!("BEAD_AVAILABILITY_USAGE unknown argument={value}");
                return ExitCode::from(2);
            }
        }
        index += 1;
    }

    if definition_quality {
        return run_definition_quality(issues_path.as_deref().unwrap_or(".beads/issues.jsonl"), json);
    }
    if readiness {
        return run_readiness(&br_program, json);
    }
    if let Some(path) = graph {
        return run_graph(Path::new(&path));
    }


    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("BEAD_AVAILABILITY_ERROR reason=runtime_build detail={error}");
            return ExitCode::from(2);
        }
    };
    let result = runtime.block_on(async move {
        let cx = Cx::current()
            .ok_or_else(|| "BEAD_AVAILABILITY_ERROR reason=no_runtime_context".to_owned())?;
        collect_live(&cx, &br_program).await
    });
    match result {
        Ok(report) => {
            if json {
                match serde_json::to_string_pretty(&report) {
                    Ok(output) => println!("{output}"),
                    Err(error) => {
                        eprintln!("BEAD_AVAILABILITY_ERROR reason=render_json detail={error}");
                        return ExitCode::from(2);
                    }
                }
            } else {
                print!("{}", report.render_text());
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("BEAD_AVAILABILITY_ERROR {error}");
            ExitCode::from(2)
        }
    }
}
fn readiness_error_exit(error: &str) -> ExitCode {
    if error.starts_with("EMPTY_") {
        ExitCode::from(3)
    } else {
        ExitCode::from(2)
    }
}
fn readiness_report_exit(residual_count: usize) -> ExitCode {
    if residual_count == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}

fn load_r7_membership() -> Result<bead_availability::ParentChildMembership, String> {
    let text = fs::read_to_string(R7_DEPENDENCY_SOURCE).map_err(|error| {
        format!("R7_SOURCE_READ_FAILED path={R7_DEPENDENCY_SOURCE} detail={error}")
    })?;
    parse_r7_parent_child_jsonl(&text, R7_S0_EPIC).map_err(|error| error.to_string())
}

fn run_readiness(br_program: &str, json: bool) -> ExitCode {
    let runtime = match RuntimeBuilder::current_thread().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            let detail = format!("runtime_build detail={error}");
            if json {
                println!("{{\"schema\":\"bead-availability/readiness-v1\",\"status\":\"ERROR\",\"error\":{}}}", serde_json::to_string(&detail).unwrap_or_else(|_| "\"runtime_build\"".to_owned()));
            }
            eprintln!("BEAD_AVAILABILITY_READINESS_ERROR {detail}");
            return ExitCode::from(2);
        }
    };
    let result = runtime.block_on(async {
        let cx = Cx::current()
            .ok_or_else(|| "NO_RUNTIME_CONTEXT: readiness collector has no Cx".to_owned())?;
        collect_ready_live(&cx, br_program).await
    });
    match result {
        Ok(report) => {
            let r7 = match load_r7_membership() {
                Ok(value) => value,
                Err(error) => {
                    if json {
                        let value = serde_json::json!({
                            "schema": "bead-availability/readiness-v1",
                            "status": "ERROR",
                            "error": error,
                        });
                        println!("{}", serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{\"status\":\"ERROR\"}".to_owned()));
                    }
                    eprintln!("BEAD_AVAILABILITY_READINESS_ERROR {error}");
                    return readiness_error_exit(&error);
                }
            };
            let residual_count = report.residual_count.saturating_add(r7.residual_count());
            if json {
                let value = serde_json::json!({
                    "schema": report.schema,
                    "status": if residual_count == 0 { "OK" } else { "RESIDUAL" },
                    "data": report,
                    "r7": r7,
                });
                match serde_json::to_string_pretty(&value) {
                    Ok(output) => println!("{output}"),
                    Err(error) => {
                        eprintln!("BEAD_AVAILABILITY_READINESS_ERROR render_json detail={error}");
                        return ExitCode::from(2);
                    }
                }
            } else {
                print!("{}", report.render_text());
                println!(
                    "R7 source={} epic={} status={} children={} non_terminal={}",
                    r7.source,
                    r7.epic_id,
                    r7.epic_status.as_str(),
                    r7.child_ids.len(),
                    r7.non_terminal_ids.len()
                );
            }
            readiness_report_exit(residual_count)
        }
        Err(error) => {
            if json {
                let value = serde_json::json!({
                    "schema": "bead-availability/readiness-v1",
                    "status": "ERROR",
                    "error": error,
                });
                println!("{}", serde_json::to_string_pretty(&value).unwrap_or_else(|_| "{\"status\":\"ERROR\"}".to_owned()));
            }
            eprintln!("BEAD_AVAILABILITY_READINESS_ERROR {error}");
            readiness_error_exit(&error)
        }
    }
}

fn run_graph(path: &Path) -> ExitCode {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "INVERSION_GRAPH_UNREADABLE path={} error={error}",
                path.display()
            );
            return ExitCode::from(3);
        }
    };
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(value) => value,
        Err(error) => {
            eprintln!("INVERSION_GRAPH_MALFORMED json={error}");
            return ExitCode::from(3);
        }
    };
    let (issues, edges) = match parse_graph_json(&value) {
        Ok(graph) => graph,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::from(error.exit_code());
        }
    };
    match classify(&issues, &edges) {
        Ok(classification) => {
            eprint!("{}", classification.report());
            ExitCode::from(classification.gate_exit())
        }
        Err(ScanError::EmptyIssueSet) => {
            eprintln!("{}", ScanError::EmptyIssueSet);
            ExitCode::from(2)
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::from(error.exit_code())
        }
    }
}

fn run_definition_quality(path: &str, json: bool) -> ExitCode {
    // Exit 0 = at-or-above floor; 1 = below-floor; 2 = vacuous/unreadable.
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("DEFINITION_QUALITY_UNREADABLE path={path} error={error}");
            return ExitCode::from(2);
        }
    };
    match parse_issues_jsonl(&text, FAMILY_NEEDLES) {
        Ok(beads) => match measure_family(&beads) {
            Ok(report) => {
                let success = !report.below_floor();
                if json {
                    println!("{}", report.to_json(success));
                } else {
                    eprintln!(
                        "DEFINITION_QUALITY beads={} specific_milli={} floor_milli={} cluster={} suffix_bytes={} specimen={} {}",
                        report.bead_count,
                        report.specific_milli,
                        report.floor_milli,
                        report.cluster_size,
                        report.cluster_suffix_bytes,
                        report.named_specimen_hit,
                        NO_CLAIM
                    );
                }
                if !success {
                    eprintln!("DEFINITION_QUALITY_BELOW_FLOOR");
                }
                ExitCode::from(report.exit_code())
            }
            Err(vacuity) => {
                eprintln!("{vacuity}");
                ExitCode::from(vacuity.exit_code())
            }
        },
        Err(vacuity) => {
            eprintln!("{vacuity}");
            ExitCode::from(vacuity.exit_code())
        }
    }
}
#[cfg(test)]
mod tests {
    use super::{readiness_error_exit, readiness_report_exit};
    use std::process::ExitCode;

    #[test]
    fn readiness_exit_codes_keep_domain_and_anti_vacuity_distinct() {
        assert_eq!(readiness_error_exit("EMPTY_READY_SURFACE: br ready returned no candidates"), ExitCode::from(3));
        assert_eq!(readiness_error_exit("MALFORMED_DEPENDENCY: line=2 missing=depends_on_id"), ExitCode::from(2));
        assert_eq!(readiness_error_exit("EMPTY_DEPENDENCY_SOURCE: dependency source contained no edges"), ExitCode::from(3));
        assert_eq!(readiness_error_exit("MALFORMED_ISSUES: br blocked response"), ExitCode::from(2));
        assert_eq!(readiness_error_exit("RUN_FAILED command=br detail=timeout"), ExitCode::from(2));
        assert_eq!(readiness_report_exit(0), ExitCode::SUCCESS);
        assert_eq!(readiness_report_exit(1), ExitCode::from(1));
    }
}

