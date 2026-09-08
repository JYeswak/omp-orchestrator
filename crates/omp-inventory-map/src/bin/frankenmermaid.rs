use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

const DIAGRAM_1: &str = "diagram-1-crate-dependencies.mmd";
const DIAGRAM_2: &str = "diagram-2-omp-surface.mmd";

#[derive(Debug)]
enum GeneratorError {
    Usage(String),
    InputMissing { kind: &'static str, path: PathBuf },
    InputEmpty { kind: &'static str },
    InputInvalid { kind: &'static str, detail: String },
    OutputMissing(PathBuf),
    DiagramDrift(PathBuf),
    Io { path: PathBuf, detail: String },
}

impl std::fmt::Display for GeneratorError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Usage(detail) => write!(formatter, "DIAGRAM_USAGE {detail}"),
            Self::InputMissing { kind, path } => {
                write!(
                    formatter,
                    "DIAGRAM_INPUT_MISSING kind={kind} path={}",
                    path.display()
                )
            }
            Self::InputEmpty { kind } => write!(formatter, "DIAGRAM_INPUT_EMPTY kind={kind}"),
            Self::InputInvalid { kind, detail } => {
                write!(
                    formatter,
                    "DIAGRAM_INPUT_INVALID kind={kind} detail={detail}"
                )
            }
            Self::OutputMissing(path) => {
                write!(formatter, "DIAGRAM_OUTPUT_MISSING path={}", path.display())
            }
            Self::DiagramDrift(path) => write!(formatter, "DIAGRAM_DRIFT path={}", path.display()),
            Self::Io { path, detail } => {
                write!(
                    formatter,
                    "DIAGRAM_IO_ERROR path={} detail={detail}",
                    path.display()
                )
            }
        }
    }
}

fn read_input(path: &Path, kind: &'static str) -> Result<String, GeneratorError> {
    fs::read_to_string(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            GeneratorError::InputMissing {
                kind,
                path: path.to_path_buf(),
            }
        } else {
            GeneratorError::Io {
                path: path.to_path_buf(),
                detail: error.to_string(),
            }
        }
    })
}

fn json_object<'a>(
    value: &'a Value,
    kind: &'static str,
) -> Result<&'a serde_json::Map<String, Value>, GeneratorError> {
    value
        .as_object()
        .ok_or_else(|| GeneratorError::InputInvalid {
            kind,
            detail: "expected JSON object".to_owned(),
        })
}

fn required_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    key: &str,
    kind: &'static str,
) -> Result<&'a str, GeneratorError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| GeneratorError::InputInvalid {
            kind,
            detail: format!("missing non-empty {key}"),
        })
}

fn render_dependency_diagram(input: &str) -> Result<(String, usize, usize), GeneratorError> {
    let root: Value =
        serde_json::from_str(input).map_err(|error| GeneratorError::InputInvalid {
            kind: "cargo_metadata",
            detail: error.to_string(),
        })?;
    let root = json_object(&root, "cargo_metadata")?;
    let packages = root
        .get("packages")
        .and_then(Value::as_array)
        .ok_or_else(|| GeneratorError::InputInvalid {
            kind: "cargo_metadata",
            detail: "missing packages array".to_owned(),
        })?;
    if packages.is_empty() {
        return Err(GeneratorError::InputEmpty {
            kind: "cargo_metadata packages",
        });
    }

    let mut names = BTreeSet::new();
    let mut edges = Vec::new();
    for package in packages {
        let package = json_object(package, "cargo_metadata package")?;
        let name = required_string(package, "name", "cargo_metadata package")?.to_owned();
        if !names.insert(name.clone()) {
            return Err(GeneratorError::InputInvalid {
                kind: "cargo_metadata",
                detail: format!("duplicate package {name}"),
            });
        }
        let dependencies = package
            .get("dependencies")
            .and_then(Value::as_array)
            .ok_or_else(|| GeneratorError::InputInvalid {
                kind: "cargo_metadata package",
                detail: format!("{name} has no dependencies array"),
            })?;
        for dependency in dependencies {
            let dependency = json_object(dependency, "cargo_metadata dependency")?;
            if dependency.get("path").and_then(Value::as_str).is_none() {
                continue;
            }
            let target =
                required_string(dependency, "name", "cargo_metadata dependency")?.to_owned();
            edges.push((name.clone(), target));
        }
    }

    let mut node_ids = BTreeMap::new();
    for (index, name) in names.iter().enumerate() {
        node_ids.insert(name.clone(), format!("n{index}"));
    }
    edges.sort();
    for (from, to) in &edges {
        if !node_ids.contains_key(to) {
            return Err(GeneratorError::InputInvalid {
                kind: "cargo_metadata",
                detail: format!("path dependency {from} -> {to} is not a workspace package"),
            });
        }
    }

    let mut output = format!(
        "%% GENERATED BY frankenmermaid; source=cargo metadata; packages={} path_dependency_edges={}\ngraph TD\n",
        names.len(),
        edges.len()
    );
    for (name, node) in &node_ids {
        output.push_str(&format!("    {node}[{name}]\n"));
    }
    for (from, to) in &edges {
        output.push_str(&format!("    {} --> {}\n", node_ids[from], node_ids[to]));
    }
    Ok((output, names.len(), edges.len()))
}

fn count_classification(rows: &[Value], classification: &str) -> usize {
    rows.iter()
        .filter(|row| row.get("classification").and_then(Value::as_str) == Some(classification))
        .count()
}

fn render_surface_diagram(input: &str) -> Result<(String, usize, usize), GeneratorError> {
    let root: Value =
        serde_json::from_str(input).map_err(|error| GeneratorError::InputInvalid {
            kind: "inventory_map",
            detail: error.to_string(),
        })?;
    let root = json_object(&root, "inventory_map")?;
    let data = root
        .get("data")
        .ok_or_else(|| GeneratorError::InputInvalid {
            kind: "inventory_map",
            detail: "missing data object".to_owned(),
        })?;
    let data = json_object(data, "inventory_map data")?;
    let counts = data
        .get("counts")
        .and_then(Value::as_object)
        .ok_or_else(|| GeneratorError::InputInvalid {
            kind: "inventory_map",
            detail: "missing data.counts object".to_owned(),
        })?;
    let rows =
        data.get("rows")
            .and_then(Value::as_array)
            .ok_or_else(|| GeneratorError::InputInvalid {
                kind: "inventory_map",
                detail: "missing data.rows array".to_owned(),
            })?;
    if rows.is_empty() {
        return Err(GeneratorError::InputEmpty {
            kind: "inventory_map rows",
        });
    }
    let workspace_crates = counts
        .get("workspace_crates")
        .and_then(Value::as_u64)
        .ok_or_else(|| GeneratorError::InputInvalid {
            kind: "inventory_map counts",
            detail: "missing workspace_crates".to_owned(),
        })?;
    let count = |key: &str| -> Result<u64, GeneratorError> {
        counts
            .get(key)
            .and_then(Value::as_u64)
            .ok_or_else(|| GeneratorError::InputInvalid {
                kind: "inventory_map counts",
                detail: format!("missing {key}"),
            })
    };
    let cli_commands = count("cli_commands")?;
    let type_roots = count("type_roots")?;
    let declarations = count("declarations")?;
    let rpc_handlers = count("rpc_handlers")?;
    let slash_commands = count("slash_commands")?;
    let capability_not_used = count_classification(rows, "CAPABILITY_NOT_USED");
    let direct = count_classification(rows, "MAPPED_BY_DIRECT_PROBE");
    let scraped = count_classification(rows, "SCRAPED_OR_OBSERVED_ALTERNATIVE");

    let mut output = format!(
        "%% GENERATED BY frankenmermaid; source=inventory map data.counts; rows={} workspace_crates={}\ngraph LR\n",
        rows.len(),
        workspace_crates
    );
    output.push_str(&format!(
        "    inv[\"crate:omp-inventory-map<br/>(1 of {workspace_crates} crates)\"]\n"
    ));
    output.push_str("    inv -->|consumes| t_cli[\"type_root:cli\"]\n");
    output.push_str("    inv -->|consumes| t_cmd[\"type_root:commands\"]\n");
    output.push_str("    inv -->|consumes| t_rpc[\"type_root:jsonrpc\"]\n");
    output.push_str("    inv -->|consumes| t_slash[\"type_root:slash-commands\"]\n");
    output.push_str("    inv -->|consumes| h_get[\"rpc_handler:get_available_commands\"]\n");
    output.push_str("    inv -->|consumes| s_probe[\"slash_command:UNKNOWN_PROBE\"]\n");
    output.push_str("    inv -->|consumes| tr_mode[\"transport:--mode=&lt;value&gt;\"]\n");
    output.push_str("    subgraph UNTOUCHED[\"surface not yet consumed\"]\n");
    output.push_str(&format!(
        "        mass[\"{} census rows<br/>{} cli_commands · {} type_roots · {} declarations<br/>{} rpc_handlers · {} slash_commands<br/>{} CAPABILITY_NOT_USED · {} MAPPED_BY_DIRECT_PROBE · {} SCRAPED_OR_OBSERVED_ALTERNATIVE\"]\n",
        rows.len(), cli_commands, type_roots, declarations, rpc_handlers, slash_commands,
        capability_not_used, direct, scraped
    ));
    output.push_str("    end\n");
    output.push_str("    subgraph SILENT[\"workspace crates with no consumed surface\"]\n        others[\"derived from the same counts\"]\n    end\n");
    output.push_str("    others -.->|no edge exists| UNTOUCHED\n");
    output.push_str("    style inv fill:#2d5016,color:#fff\n    style UNTOUCHED fill:#4a1010,color:#fff\n    style SILENT fill:#4a1010,color:#fff\n");
    Ok((output, rows.len(), workspace_crates as usize))
}

fn diagram_paths(output_dir: &Path) -> (PathBuf, PathBuf) {
    (output_dir.join(DIAGRAM_1), output_dir.join(DIAGRAM_2))
}

fn check_exact(path: &Path, existing: &str, generated: &str) -> Result<(), GeneratorError> {
    if existing == generated {
        Ok(())
    } else {
        Err(GeneratorError::DiagramDrift(path.to_path_buf()))
    }
}
fn compare_or_write(path: &Path, generated: &str, write_mode: bool) -> Result<(), GeneratorError> {
    if write_mode {
        fs::write(path, generated).map_err(|error| GeneratorError::Io {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })
    } else {
        let existing = fs::read_to_string(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                GeneratorError::OutputMissing(path.to_path_buf())
            } else {
                GeneratorError::Io {
                    path: path.to_path_buf(),
                    detail: error.to_string(),
                }
            }
        })?;
        check_exact(path, &existing, generated)
    }
}

fn run(args: impl IntoIterator<Item = String>) -> Result<(), GeneratorError> {
    let mut metadata = None;
    let mut inventory = None;
    let mut output_dir = None;
    let mut write_mode = None;
    let mut values = args.into_iter();
    while let Some(value) = values.next() {
        match value.as_str() {
            "--metadata" => metadata = values.next().map(PathBuf::from),
            "--inventory" => inventory = values.next().map(PathBuf::from),
            "--output-dir" => output_dir = values.next().map(PathBuf::from),
            "--write" => write_mode = Some(true),
            "--check" => write_mode = Some(false),
            "--help" | "-h" => {
                return Err(GeneratorError::Usage(
                    "--metadata PATH --inventory PATH --output-dir DIR --write|--check".to_owned(),
                ));
            }
            other => return Err(GeneratorError::Usage(format!("unknown argument {other}"))),
        }
    }
    let metadata =
        metadata.ok_or_else(|| GeneratorError::Usage("--metadata is required".to_owned()))?;
    let inventory =
        inventory.ok_or_else(|| GeneratorError::Usage("--inventory is required".to_owned()))?;
    let output_dir =
        output_dir.ok_or_else(|| GeneratorError::Usage("--output-dir is required".to_owned()))?;
    let write_mode = write_mode.ok_or_else(|| {
        GeneratorError::Usage("choose exactly one of --write or --check".to_owned())
    })?;

    let metadata_text = read_input(&metadata, "cargo_metadata")?;
    let inventory_text = read_input(&inventory, "inventory_map")?;
    let (diagram_1, packages, path_edges) = render_dependency_diagram(&metadata_text)?;
    let (diagram_2, rows, workspace_crates) = render_surface_diagram(&inventory_text)?;
    if write_mode {
        fs::create_dir_all(&output_dir).map_err(|error| GeneratorError::Io {
            path: output_dir.clone(),
            detail: error.to_string(),
        })?;
    }
    let (path_1, path_2) = diagram_paths(&output_dir);
    compare_or_write(&path_1, &diagram_1, write_mode)?;
    compare_or_write(&path_2, &diagram_2, write_mode)?;
    println!(
        "FRANKENMERMAID PASS mode={} diagram1_packages={} path_dependency_edges={} diagram2_rows={} diagram2_workspace_crates={} outputs={} {}",
        if write_mode { "write" } else { "check" },
        packages,
        path_edges,
        rows,
        workspace_crates,
        path_1.display(),
        path_2.display()
    );
    Ok(())
}

fn main() -> ExitCode {
    match run(env::args().skip(1)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("FRANKENMERMAID ERROR {error}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const METADATA: &str = r#"{"packages":[
        {"name":"a","dependencies":[{"name":"b","path":"../b"}]},
        {"name":"b","dependencies":[]},
        {"name":"c","dependencies":[{"name":"b","path":"../b"}]}
    ]}"#;
    const INVENTORY: &str = r#"{"data":{"counts":{"workspace_crates":3,"cli_commands":1,"type_roots":2,"declarations":3,"rpc_handlers":4,"slash_commands":5},"rows":[
        {"classification":"CAPABILITY_NOT_USED"},
        {"classification":"MAPPED_BY_DIRECT_PROBE"},
        {"classification":"SCRAPED_OR_OBSERVED_ALTERNATIVE"}
    ]}}"#;

    #[test]
    fn metadata_edges_match_the_input_path_dependency_count() {
        let (diagram, packages, edges) = render_dependency_diagram(METADATA).expect("metadata");
        assert_eq!(packages, 3);
        assert_eq!(edges, 2);
        assert_eq!(diagram.matches(" --> ").count(), 2);
        assert!(diagram.contains("path_dependency_edges=2"));
    }

    #[test]
    fn hand_edited_edge_is_a_typed_drift() {
        let (diagram, _, _) = render_dependency_diagram(METADATA).expect("metadata");
        let edited = diagram.replacen("n0 --> n1", "n0 --> n2", 1);
        assert_ne!(edited, diagram);
        let error = check_exact(Path::new(DIAGRAM_1), &edited, &diagram)
            .expect_err("a hand-edited edge must be RED");
        assert!(
            matches!(error, GeneratorError::DiagramDrift(path) if path == Path::new(DIAGRAM_1))
        );
    }

    #[test]
    fn inventory_counts_are_rendered_from_the_input() {
        let (diagram, rows, crates) = render_surface_diagram(INVENTORY).expect("inventory");
        assert_eq!(rows, 3);
        assert_eq!(crates, 3);
        assert!(diagram.contains("3 census rows"));
        assert!(diagram.contains("1 CAPABILITY_NOT_USED"));
        assert!(diagram.contains("2 type_roots"));
    }

    #[test]
    fn empty_or_missing_inputs_refuse_with_named_messages() {
        let empty_metadata = render_dependency_diagram(r#"{"packages":[]}"#)
            .expect_err("empty metadata must refuse");
        assert!(matches!(empty_metadata, GeneratorError::InputEmpty { .. }));
        assert!(empty_metadata.to_string().contains("DIAGRAM_INPUT_EMPTY"));

        let empty_inventory = render_surface_diagram(r#"{"data":{"counts":{},"rows":[]}}"#)
            .expect_err("empty inventory must refuse");
        assert!(matches!(empty_inventory, GeneratorError::InputEmpty { .. }));
        assert!(empty_inventory.to_string().contains("DIAGRAM_INPUT_EMPTY"));

        let invalid_inventory =
            render_surface_diagram(r#"{}"#).expect_err("missing inventory data must refuse");
        assert!(matches!(
            invalid_inventory,
            GeneratorError::InputInvalid { .. }
        ));
        assert!(
            invalid_inventory
                .to_string()
                .contains("DIAGRAM_INPUT_INVALID")
        );
    }
}
