#![forbid(unsafe_code)]

use no_shell_gate::project_agent::{
    validate_grader_agent_bytes, validate_grader_agent_file, ProjectAgentError, OMP_GRADER_PATH,
    REQUIRED_OUTPUT_PROPERTIES,
};
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

const CURRENT_SOURCE: &str = include_str!("../../../.omp/agents/omp-grader.md");

#[test]
fn current_project_grader_passes_the_real_file_gate() {
    validate_grader_agent_file(&repo_root()).unwrap_or_else(|error| panic!("{error}"));
}

#[test]
fn forbidden_tools_are_refused_with_exact_path_and_tool() {
    let source = CURRENT_SOURCE;
    for tool in ["write", "edit"] {
        let mutant = source.replacen("  - bash\n", &format!("  - bash\n  - {tool}\n"), 1);
        let error = validate_grader_agent_bytes(Path::new(OMP_GRADER_PATH), mutant.as_bytes())
            .expect_err("forbidden grader tool must refuse");
        assert_eq!(
            error.to_string(),
            format!(
                "PROJECT_AGENT_REFUSED path={OMP_GRADER_PATH} tool={tool} reason=FORBIDDEN_TOOL"
            )
        );
    }
}

#[test]
fn every_required_output_property_has_an_exact_missing_refusal() {
    let source = CURRENT_SOURCE;
    for property in REQUIRED_OUTPUT_PROPERTIES {
        let marker = format!("    {property}:");
        let mut removed = false;
        let mutant = source
            .lines()
            .filter_map(|line| {
                if !removed && line.starts_with(&marker) {
                    removed = true;
                    None
                } else {
                    Some(line)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert!(removed, "test fixture did not remove {property}");
        let error = validate_grader_agent_bytes(Path::new(OMP_GRADER_PATH), mutant.as_bytes())
            .expect_err("missing required output property must refuse");
        assert_eq!(
            error.to_string(),
            format!(
                "PROJECT_AGENT_REFUSED path={OMP_GRADER_PATH} property={property} \
                 reason=MISSING_REQUIRED_OUTPUT_PROPERTY"
            )
        );
    }
}

#[test]
fn missing_and_invalid_utf8_files_are_typed_anti_vacuity_refusals() {
    let missing = validate_grader_agent_file(&repo_root().join("definitely-missing-root"))
        .expect_err("missing grader file must refuse");
    assert_eq!(
        missing.to_string(),
        format!("PROJECT_AGENT_REFUSED path={OMP_GRADER_PATH} reason=MISSING_FILE")
    );

    let unreadable = validate_grader_agent_bytes(Path::new(OMP_GRADER_PATH), &[0xff])
        .expect_err("invalid UTF-8 grader file must refuse");
    assert!(matches!(
        unreadable,
        ProjectAgentError::UnreadableFile { .. }
    ));
    assert!(unreadable.to_string().contains("reason=UNREADABLE_FILE"));
}

#[test]
fn comments_and_mapping_order_do_not_change_the_verdict() {
    let source = r#"---
output: # comments are not content
  properties:
    no_claim:
    controls:
    reexecuted:
    tree_pin:
    proof_test_result:
    proof_exit:
    worker:
    verdict_class:
    verdict:
tools:
  - yield # safe tool
  - bash
  - read
  - glob
name: omp-grader
---
The body may mention write or edit without changing frontmatter.
"#;
    assert_eq!(
        validate_grader_agent_bytes(Path::new(OMP_GRADER_PATH), source.as_bytes()),
        Ok(())
    );
}
