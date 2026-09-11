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

/// Strip a YAML `#` comment without changing the source line.
///
/// YAML opens a comment only at line start or after whitespace, and both quote
/// kinds protect `#`. Routes the `#`-comment call sites that `toml_code_only`
/// would over-strip (`key: value#fragment` keeps its tail here).
pub fn yaml_code_only(line: &str) -> Cow<'_, str> {
    Cow::Borrowed(strip_yaml_comment(line))
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

/// Return prose (markdown/TOML/plain) with QUOTED SPECIMENS blanked.
///
/// S1 criterion R8, self-referential half. `code_only` blanks Rust comments; this is its
/// prose sibling, and prose is where the class actually bit us. Blanks, preserving line
/// structure so line numbers survive:
///
/// * fenced blocks — ```` ``` ```` … ```` ``` ````
/// * inline code spans — `` `like this` ``
///
/// WHY THIS EXISTS, measured 2026-09-07 across one session. A citation-hygiene scan over a
/// corpus that CONTAINS its own defect reports finds its OWN specimens, and every instance
/// was a false positive on a document that was already correct:
///
/// * `CONTRACT.md`'s only `mirror:beads_rust/...` match is inside the sentence REFUSING that
///   form — a specimen in the rule against it.
/// * `planning_to_exhaustion.md`'s two unprefixed `plf.N` ids are inside the PX-D5 row that
///   REPORTS the dropped leading `j`.
/// * a contract's own doc-name census counted itself, reporting 24/9 where the true figures
///   were 23/8, because the file names both candidates while arguing about them.
///
/// The remedy is to strip specimens before matching — NEVER to edit the documents that
/// record the rule, which is how a correct document gets "fixed" into a wrong one.
///
/// OVER-STRIPPING IS THE SAFE DIRECTION: it can only report LESS, and the failure this
/// prevents is a false POSITIVE against a compliant file.
pub fn prose_specimen_stripped(source: &str) -> Cow<'_, str> {
    if !source.contains('`') {
        return Cow::Borrowed(source);
    }
    let mut out = String::with_capacity(source.len());
    let mut fenced = false;
    for line in source.lines() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            blank_line_into(&mut out, line);
            continue;
        }
        if fenced {
            blank_line_into(&mut out, line);
            continue;
        }
        // Inline spans. An UNCLOSED backtick blanks to end of line: a half-open span is
        // ambiguous, and the safe direction is to strip.
        let mut buf = String::with_capacity(line.len());
        let mut inside = false;
        for ch in line.chars() {
            if ch == '`' {
                inside = !inside;
                buf.push(' ');
            } else if inside {
                buf.push(' ');
            } else {
                buf.push(ch);
            }
        }
        out.push_str(&buf);
        out.push('\n');
    }
    if !source.ends_with('\n') {
        out.pop();
    }
    Cow::Owned(out)
}

fn blank_line_into(out: &mut String, line: &str) {
    for _ in line.chars() {
        out.push(' ');
    }
    out.push('\n');
}

/// One scan hit, attributed to the file it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanHit {
    /// Path the hit was found in.
    pub path: String,
    /// The matched text, for a reader to judge.
    pub matched: String,
}

/// What a census scan actually measured, split so a self-hit cannot be cited as a defect.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScanVerdict {
    /// Hits in files that DECLARE the rule — never citable as defects.
    pub self_referential: Vec<ScanHit>,
    /// Hits in files that do not declare the rule.
    pub citable: Vec<ScanHit>,
}

impl ScanVerdict {
    /// Both denominators, always. A bare count of either half is the defect this splits.
    #[must_use]
    pub fn census(&self) -> String {
        format!(
            "SCAN_CENSUS hits={} self_referential={} citable={}",
            self.self_referential.len() + self.citable.len(),
            self.self_referential.len(),
            self.citable.len()
        )
    }
}

/// Split a scan's hits into self-referential and citable.
///
/// `rule_files` are the paths that DEFINE the rule being scanned for — a checker's own
/// source, the contract that states the convention, the finding that reports the defect. A hit
/// in one of those is the scanner seeing its own specimen.
///
/// ANTI-VACUITY: an empty hit set is an `Err`, never an empty pass. A scan that covered
/// nothing reports identically to one that covered everything and found nothing, and this
/// repository has paid for that conflation repeatedly.
///
/// A `rule_files` entry matching NO hit is also an `Err`: a stale exclusion silently widens
/// the citable set on the next edit, and an allowlist row that matches nothing is the same
/// never-fires shape as the gate it guards.
pub fn classify_scan(hits: &[ScanHit], rule_files: &[&str]) -> Result<ScanVerdict, String> {
    if hits.is_empty() {
        return Err(
            "SCAN_EMPTY reason=zero_hits — an empty scan set is an ERROR, never a pass".to_owned(),
        );
    }
    let mut verdict = ScanVerdict {
        self_referential: Vec::new(),
        citable: Vec::new(),
    };
    let mut used = vec![false; rule_files.len()];
    for hit in hits {
        match rule_files
            .iter()
            .position(|rule| hit.path == *rule || hit.path.ends_with(rule))
        {
            Some(index) => {
                used[index] = true;
                verdict.self_referential.push(hit.clone());
            }
            None => verdict.citable.push(hit.clone()),
        }
    }
    let stale: Vec<&str> = rule_files
        .iter()
        .zip(&used)
        .filter(|(_, hit)| !**hit)
        .map(|(rule, _)| *rule)
        .collect();
    if !stale.is_empty() {
        return Err(format!(
            "SCAN_STALE_RULE_FILE reason=declared_rule_file_matched_no_hit rows={stale:?} — a \
             stale exclusion silently widens the citable set on the next edit"
        ));
    }
    Ok(verdict)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn yaml_comment_needs_whitespace_and_respects_quotes() {
        assert_eq!(yaml_code_only("# full line"), "");
        assert_eq!(yaml_code_only("key: value # tail"), "key: value ");
        assert_eq!(
            yaml_code_only("key: value#fragment"),
            "key: value#fragment"
        );
        assert_eq!(
            yaml_code_only("key: \"quoted # kept\""),
            "key: \"quoted # kept\""
        );
        assert_eq!(
            yaml_code_only("key: 'single # kept'"),
            "key: 'single # kept'"
        );
    }

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
