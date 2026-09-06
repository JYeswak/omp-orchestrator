#![forbid(unsafe_code)]

//! Structure-aware matching primitives for gate and census crates.
//!
//! Comments and fenced prose are not code evidence. These helpers preserve line
//! structure while making that distinction explicit at every caller.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;

/// A heading-anchored documentation section outside fenced or inline code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub heading: String,
    pub body: String,
}

/// A manifest dependency name.
pub type Name = String;

/// One raw matcher expression found in source code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTextMatch {
    pub line: usize,
    pub expression: String,
}

fn blank_range(out: &mut String, chars: &[char], start: usize, end: usize) {
    for &character in &chars[start..end] {
        out.push(if character == '\n' || character == '\r' {
            character
        } else {
            ' '
        });
    }
}

fn raw_string_end(chars: &[char], start: usize) -> Option<usize> {
    if chars.get(start) != Some(&'r') {
        return None;
    }
    let mut index = start + 1;
    while chars.get(index) == Some(&'#') {
        index += 1;
    }
    if chars.get(index) != Some(&'"') {
        return None;
    }
    let hashes = index - start - 1;
    index += 1;
    while index < chars.len() {
        if chars[index] == '"'
            && chars
                .get(index + 1..index + 1 + hashes)
                .is_some_and(|tail| tail.iter().all(|character| *character == '#'))
        {
            return Some(index + hashes + 1);
        }
        index += 1;
    }
    Some(chars.len())
}

/// Return source with comments blanked and string literals preserved.
///
/// The returned representation preserves line structure. `Cow::Borrowed` is
/// returned when no comments needed masking.
pub fn code_only(source: &str) -> Cow<'_, str> {
    let chars: Vec<char> = source.chars().collect();
    let mut out = String::with_capacity(source.len());
    let mut changed = false;
    let mut index = 0usize;
    while index < chars.len() {
        if chars[index] == '/' && chars.get(index + 1) == Some(&'/') {
            let start = index;
            index += 2;
            while index < chars.len() && chars[index] != '\n' {
                index += 1;
            }
            blank_range(&mut out, &chars, start, index);
            changed = true;
            continue;
        }
        if chars[index] == '/' && chars.get(index + 1) == Some(&'*') {
            let start = index;
            index += 2;
            let mut depth = 1usize;
            while index < chars.len() && depth > 0 {
                if chars[index] == '/' && chars.get(index + 1) == Some(&'*') {
                    depth += 1;
                    index += 2;
                } else if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
                    depth -= 1;
                    index += 2;
                } else {
                    index += 1;
                }
            }
            blank_range(&mut out, &chars, start, index);
            changed = true;
            continue;
        }
        if let Some(end) = raw_string_end(&chars, index) {
            out.extend(chars[index..end].iter().copied());
            index = end;
            continue;
        }
        if chars[index] == '"' || chars[index] == '\'' {
            let quote = chars[index];
            out.push(quote);
            index += 1;
            while index < chars.len() {
                let character = chars[index];
                out.push(character);
                index += 1;
                if character == '\\' && index < chars.len() {
                    out.push(chars[index]);
                    index += 1;
                    continue;
                }
                if character == quote {
                    break;
                }
            }
            continue;
        }
        out.push(chars[index]);
        index += 1;
    }
    if changed {
        Cow::Owned(out)
    } else {
        Cow::Borrowed(source)
    }
}

/// Match a complete identifier after comment masking.
///
/// The boundary rule is the source-level equivalent of regex \b: a needle that
/// is only a prefix of a longer identifier does not count.
pub fn has_identifier(source: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    let code = code_only(source);
    code.match_indices(needle).any(|(start, _)| {
        let end = start + needle.len();
        let left_ok = start == 0
            || !code.as_bytes()[start - 1].is_ascii_alphanumeric()
                && code.as_bytes()[start - 1] != b'_';
        let right_ok = end == code.len()
            || !code.as_bytes()[end].is_ascii_alphanumeric() && code.as_bytes()[end] != b'_';
        left_ok && right_ok
    })
}

/// Return source with comments and literal bodies blanked while preserving bytes and lines.
///
/// This is for scanners that report byte offsets. Use `code_only` when string
/// literals are themselves the evidence being measured.
pub fn code_and_literals(source: &str) -> String {
    let mut bytes = source.as_bytes().to_vec();
    let mut index = 0usize;
    let mut block_depth = 0usize;
    let mut quote = None;
    let mut escaped = false;
    while index < bytes.len() {
        if block_depth > 0 {
            if bytes.get(index..index + 2) == Some(b"/*") {
                bytes[index] = b' ';
                bytes[index + 1] = b' ';
                block_depth += 1;
                index += 2;
            } else if bytes.get(index..index + 2) == Some(b"*/") {
                bytes[index] = b' ';
                bytes[index + 1] = b' ';
                block_depth -= 1;
                index += 2;
            } else {
                if bytes[index] != b'\n' {
                    bytes[index] = b' ';
                }
                index += 1;
            }
            continue;
        }
        if let Some(end_quote) = quote {
            if bytes[index] == b'\n' {
                quote = None;
                escaped = false;
                index += 1;
            } else {
                let closes = bytes[index] == end_quote && !escaped;
                bytes[index] = b' ';
                escaped = bytes[index] == b'\\' && !escaped;
                index += 1;
                if closes {
                    quote = None;
                    escaped = false;
                }
            }
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"//") {
            while index < bytes.len() && bytes[index] != b'\n' {
                bytes[index] = b' ';
                index += 1;
            }
        } else if bytes.get(index..index + 2) == Some(b"/*") {
            bytes[index] = b' ';
            bytes[index + 1] = b' ';
            block_depth = 1;
            index += 2;
        } else if bytes[index] == b'"' || bytes[index] == b'\'' {
            quote = Some(bytes[index]);
            bytes[index] = b' ';
            escaped = false;
            index += 1;
        } else {
            index += 1;
        }
    }
    String::from_utf8(bytes).unwrap_or_else(|_| source.to_owned())
}

fn toggle_inline_code(mut state: bool, line: &str) -> bool {
    let mut escaped = false;
    for character in line.chars() {
        if character == '`' && !escaped {
            state = !state;
        }
        escaped = character == '\\' && !escaped;
        if character != '\\' {
            escaped = false;
        }
    }
    state
}

/// Extract `## heading` sections outside fenced and inline code.
pub fn doc_sections(source: &str) -> Vec<Section> {
    let mut sections = Vec::new();
    let mut fenced = false;
    let mut inline_code = false;
    let mut current: Option<Section> = None;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            fenced = !fenced;
        }
        let heading = !fenced && !inline_code && line.starts_with("## ");
        if heading {
            if let Some(section) = current.take() {
                sections.push(section);
            }
            current = Some(Section {
                heading: line[3..].trim().to_owned(),
                body: String::new(),
            });
        } else if let Some(section) = current.as_mut() {
            if !section.body.is_empty() {
                section.body.push('\n');
            }
            section.body.push_str(line);
        }
        inline_code = toggle_inline_code(inline_code, line);
    }
    if let Some(section) = current {
        sections.push(section);
    }
    sections
}

fn strip_toml_comment(line: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;
    for (index, character) in line.char_indices() {
        if character == '"' && !escaped {
            quoted = !quoted;
        }
        if character == '#' && !quoted {
            return &line[..index];
        }
        escaped = character == '\\' && !escaped;
        if character != '\\' {
            escaped = false;
        }
    }
    line
}

/// Strip a TOML comment without changing the source line.
pub fn toml_code_only(line: &str) -> Cow<'_, str> {
    Cow::Borrowed(strip_toml_comment(line))
}

/// Read dependency keys from Cargo manifest dependency tables.
pub fn manifest_deps(path: &Path) -> io::Result<BTreeSet<Name>> {
    let text = fs::read_to_string(path)?;
    let mut section = String::new();
    let mut deps = BTreeSet::new();
    for raw in text.lines() {
        let toml_line = toml_code_only(raw);
        let line = toml_line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            section = line.trim_matches(['[', ']']).trim().to_owned();
            continue;
        }
        if !matches!(
            section.as_str(),
            "dependencies" | "dev-dependencies" | "build-dependencies" | "workspace.dependencies"
        ) {
            continue;
        }
        let Some((name, _)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim().trim_matches('"');
        if !name.is_empty()
            && name.chars().all(|character| {
                character.is_ascii_alphanumeric() || character == '_' || character == '-'
            })
        {
            deps.insert(name.to_owned());
        }
    }
    Ok(deps)
}

/// Find raw source/document matcher calls that must use structure-aware input.
pub fn raw_text_matches(source: &str) -> Vec<RawTextMatch> {
    let code = code_only(source);
    let mut findings = Vec::new();
    let mut allow_next = false;
    for (index, (raw, line)) in source.lines().zip(code.lines()).enumerate() {
        if raw.contains("structure-keyed:") {
            allow_next = true;
            continue;
        }
        if allow_next {
            allow_next = false;
            continue;
        }
        if line.contains("text_structure::")
            || line.trim_start().starts_with("assert!")
            || line.trim_start().starts_with("panic!")
        {
            continue;
        }
        let raw_match = line.contains(".contains(\"")
            || line.contains(".find(\"")
            || line.contains(".matches(\"")
            || line.contains("grep -c")
            || line.contains("grep -rl");
        if raw_match {
            findings.push(RawTextMatch {
                line: index + 1,
                expression: line.trim().to_owned(),
            });
        }
    }
    findings
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn comments_and_literals_are_distinct() {
        let source = "fn real() {} // fake comment\n/* fake block */ fn next() {}\nlet s = \"literal needle\";";
        let code = code_only(source);
        assert!(code.contains("fn real"));
        assert!(!code.contains("fake comment"));
        assert!(code.contains("literal needle"));
    }

    #[test]
    fn code_and_literals_masks_bodies_and_preserves_bytes() {
        let source = "fn real() { call(); } // fake\nlet s = \"needle\";\n";
        let masked = code_and_literals(source);
        assert_eq!(masked.len(), source.len());
        assert!(masked.contains("fn real() { call(); }"));
        assert!(!masked.contains("fake"));
        assert!(!masked.contains("needle"));
    }

    #[test]
    fn known_good_code_remains_visible() {
        assert!(code_only("fn real() { call_needle(); }").contains("call_needle"));
    }

    #[test]
    fn raw_matcher_known_bad_comment_is_invisible_and_code_is_visible() {
        assert!(raw_text_matches("// let n = body.contains(\"needle\");\n").is_empty());
        assert_eq!(
            raw_text_matches("let n = body.contains(\"needle\");\n").len(),
            1
        );
        assert!(
            raw_text_matches(
                "// structure-keyed: shared matcher\nlet n = body.contains(\"needle\");\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn doc_sections_ignore_fenced_and_inline_headings() {
        let source = "## Real\nbody\n```\n## Fake\n```\n`## Inline Fake`\n## Next\nvalue";
        let sections = doc_sections(source);
        assert_eq!(sections.len(), 2);
        assert_eq!(sections[0].heading, "Real");
        assert_eq!(sections[1].heading, "Next");
    }

    #[test]
    fn manifest_dependencies_are_structural() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        writeln!(
            file,
            "[dependencies]\nreal = \"1\"\n# fake = \"2\"\n[package]\nnope = \"3\""
        )
        .unwrap();
        let deps = manifest_deps(file.path()).unwrap();
        assert_eq!(deps, BTreeSet::from(["real".to_owned()]));
    }

    #[test]
    fn identifier_boundary_rejects_prefix_matches() {
        assert!(!has_identifier("pub enum LeaseRefusal {}", "Lease"));
        assert!(has_identifier("pub enum Lease {}", "Lease"));
        assert!(!has_identifier("// pub enum Lease {}", "Lease"));
    }
    #[test]
    fn empty_inputs_are_safe() {
        assert!(code_only("").is_empty());
        assert!(doc_sections("").is_empty());
        assert!(raw_text_matches("").is_empty());
    }
}
