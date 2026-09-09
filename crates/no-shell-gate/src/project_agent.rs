//! Commit-time validation for the project-local OMP grader definition.
//!
//! OMP's upstream loader parses agent frontmatter but does not enforce this
//! repository's grader contract. This module is the small, local policy layer:
//! it reads the semantic frontmatter, rejects forbidden tools and missing
//! output properties, and fails closed when the file cannot be read.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::Path;
use std::str;

pub const OMP_GRADER_PATH: &str = ".omp/agents/omp-grader.md";
pub const REQUIRED_OUTPUT_PROPERTIES: &[&str; 9] = &[
    "verdict",
    "verdict_class",
    "worker",
    "proof_exit",
    "proof_test_result",
    "tree_pin",
    "reexecuted",
    "controls",
    "no_claim",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProjectAgentError {
    MissingFile { path: String },
    UnreadableFile { path: String, detail: String },
    InvalidFrontmatter { path: String, detail: String },
    MissingTools { path: String },
    ForbiddenTool { path: String, tool: String },
    MissingOutputProperty { path: String, property: String },
}

impl fmt::Display for ProjectAgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFile { path } => {
                write!(f, "PROJECT_AGENT_REFUSED path={path} reason=MISSING_FILE")
            }
            Self::UnreadableFile { path, detail } => write!(
                f,
                "PROJECT_AGENT_REFUSED path={path} reason=UNREADABLE_FILE detail={detail}"
            ),
            Self::InvalidFrontmatter { path, detail } => write!(
                f,
                "PROJECT_AGENT_REFUSED path={path} reason=INVALID_FRONTMATTER detail={detail}"
            ),
            Self::MissingTools { path } => {
                write!(f, "PROJECT_AGENT_REFUSED path={path} reason=MISSING_TOOLS")
            }
            Self::ForbiddenTool { path, tool } => write!(
                f,
                "PROJECT_AGENT_REFUSED path={path} tool={tool} reason=FORBIDDEN_TOOL"
            ),
            Self::MissingOutputProperty { path, property } => write!(
                f,
                "PROJECT_AGENT_REFUSED path={path} property={property} reason=MISSING_REQUIRED_OUTPUT_PROPERTY"
            ),
        }
    }
}

impl std::error::Error for ProjectAgentError {}

/// Validate the committed project-local grader file from a repository root.
pub fn validate_grader_agent_file(repo_root: &Path) -> Result<(), ProjectAgentError> {
    let path = repo_root.join(OMP_GRADER_PATH);
    let bytes = fs::read(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ProjectAgentError::MissingFile {
                path: OMP_GRADER_PATH.to_owned(),
            }
        } else {
            ProjectAgentError::UnreadableFile {
                path: OMP_GRADER_PATH.to_owned(),
                detail: error.to_string(),
            }
        }
    })?;
    validate_grader_agent_bytes(Path::new(OMP_GRADER_PATH), &bytes)
}

/// Validate grader bytes read from either the worktree or the git index.
pub fn validate_grader_agent_bytes(path: &Path, bytes: &[u8]) -> Result<(), ProjectAgentError> {
    let path = path.to_string_lossy().into_owned();
    let text = str::from_utf8(bytes).map_err(|error| ProjectAgentError::UnreadableFile {
        path: path.clone(),
        detail: format!("invalid UTF-8: {error}"),
    })?;
    let parsed =
        parse_frontmatter(text).map_err(|detail| ProjectAgentError::InvalidFrontmatter {
            path: path.clone(),
            detail,
        })?;

    if parsed.tools.is_empty() {
        return Err(ProjectAgentError::MissingTools { path });
    }
    for tool in parsed.tools {
        if tool == "write" || tool == "edit" {
            return Err(ProjectAgentError::ForbiddenTool { path, tool });
        }
    }
    for property in REQUIRED_OUTPUT_PROPERTIES {
        if !parsed.output_properties.contains(*property) {
            return Err(ProjectAgentError::MissingOutputProperty {
                path,
                property: (*property).to_owned(),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
struct ParsedFrontmatter {
    tools: Vec<String>,
    output_properties: BTreeSet<String>,
}

fn parse_frontmatter(text: &str) -> Result<ParsedFrontmatter, String> {
    let mut lines = text.lines();
    let first = lines
        .next()
        .unwrap_or_default()
        .trim_start_matches('\u{feff}')
        .trim();
    if first != "---" {
        return Err("missing opening --- delimiter".to_owned());
    }

    let mut body = Vec::new();
    let mut closed = false;
    for line in lines {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if strip_yaml_comment(line).trim() == "---" {
            closed = true;
            break;
        }
        body.push(line);
    }
    if !closed {
        return Err("missing closing --- delimiter".to_owned());
    }

    let mut parsed = ParsedFrontmatter::default();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut tools_indent = None;
    for raw in body {
        let line = strip_yaml_comment(raw).trim_end();
        if line.trim().is_empty() {
            continue;
        }
        let indent = line.bytes().take_while(|byte| *byte == b' ').count();
        let content = line[indent..].trim_end();

        if is_sequence_item(content) {
            if stack.len() == 1
                && stack[0].1 == "tools"
                && tools_indent.is_some_and(|tools| indent > tools)
            {
                let value = scalar(content[1..].trim());
                if !value.is_empty() {
                    parsed.tools.push(value);
                }
            }
            continue;
        }

        let Some(colon) = content.find(':') else {
            return Err(format!("mapping entry has no colon: {content}"));
        };
        let key = content[..colon].trim();
        if key.is_empty() {
            return Err("mapping entry has an empty key".to_owned());
        }
        while stack.last().is_some_and(|(level, _)| *level >= indent) {
            stack.pop();
        }

        if stack.len() == 2 && stack[0].1 == "output" && stack[1].1 == "properties" {
            parsed.output_properties.insert(scalar(key));
        }

        let value = content[colon + 1..].trim();
        if stack.is_empty() && key == "tools" {
            tools_indent = Some(indent);
            parsed.tools.extend(inline_list(value));
        }
        stack.push((indent, scalar(key)));
    }
    Ok(parsed)
}

fn is_sequence_item(content: &str) -> bool {
    content == "-" || content.starts_with("- ") || content.starts_with("-\t")
}

fn inline_list(value: &str) -> Vec<String> {
    let value = value.trim();
    if !(value.starts_with('[') && value.ends_with(']')) {
        return Vec::new();
    }
    value[1..value.len() - 1]
        .split(',')
        .map(scalar)
        .filter(|item| !item.is_empty())
        .collect()
}

fn scalar(value: &str) -> String {
    let value = value.trim();
    if value.len() >= 2 {
        let first = value.as_bytes()[0] as char;
        let last = value.as_bytes()[value.len() - 1] as char;
        if (first == '"' && last == '"') || (first == '\'' && last == '\'') {
            return value[1..value.len() - 1].to_owned();
        }
    }
    value.to_owned()
}

fn strip_yaml_comment(line: &str) -> &str {
    let mut quote = None;
    for (index, character) in line.char_indices() {
        match (quote, character) {
            (None, '"') | (None, '\'') => quote = Some(character),
            (Some(open), character) if character == open => quote = None,
            (None, '#')
                if index == 0
                    || line[..index]
                        .chars()
                        .next_back()
                        .is_some_and(|previous| previous.is_whitespace()) =>
            {
                return &line[..index];
            }
            _ => {}
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comments_and_order_do_not_change_semantic_result() {
        let source = r#"---
output: # output can precede tools
  properties:
    no_claim: {type: string}
    controls: {type: string}
    reexecuted: {type: string}
    tree_pin: {type: string}
    proof_test_result: {type: string}
    proof_exit: {type: string}
    worker: {type: string}
    verdict_class: {type: string}
    verdict: {type: string}
tools:
  - yield # safe
  - bash
  - read
  - glob
name: omp-grader
---
body mentions write and edit but is outside frontmatter
"#;
        assert!(validate_grader_agent_bytes(Path::new(OMP_GRADER_PATH), source.as_bytes()).is_ok());
    }

    #[test]
    fn inline_tools_are_parsed_semantically() {
        let source = r#"---
tools: [read, grep, glob, bash, yield]
output:
  properties:
    verdict:
    verdict_class:
    worker:
    proof_exit:
    proof_test_result:
    tree_pin:
    reexecuted:
    controls:
    no_claim:
---
"#;
        assert!(validate_grader_agent_bytes(Path::new(OMP_GRADER_PATH), source.as_bytes()).is_ok());
    }
}
