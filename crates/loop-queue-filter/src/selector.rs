#![forbid(unsafe_code)]

//! Graph-selector admission (09.10 / bcrn.4).
//!
//! `bv` is resolved from `PATH` only. A missing binary is
//! `SELECTOR_UNAVAILABLE program=bv` — never a panic, never recency
//! ranking, never a hardcoded host path.

use std::fmt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// The graph selector program name. Looked up on PATH; never an absolute host path.
pub const PROGRAM: &str = "bv";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SelectorError {
    Unavailable { program: &'static str },
}

impl fmt::Display for SelectorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable { program } => {
                write!(formatter, "SELECTOR_UNAVAILABLE program={program}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectorReceipt {
    pub program: &'static str,
    pub resolved: PathBuf,
}

impl SelectorReceipt {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"status\":\"AVAILABLE\",\"program\":\"{}\",\"resolved\":\"{}\",\"selected_by\":\"{}\"}}",
            self.program,
            self.resolved.display(),
            self.program
        )
    }
}

fn is_executable(path: &Path) -> bool {
    match path.metadata() {
        Ok(meta) => meta.is_file() && meta.permissions().mode() & 0o111 != 0,
        Err(_) => false,
    }
}

/// Resolve `program` on a PATH string. Absolute host paths are not consulted.
pub fn resolve_on_path(program: &str, path: &str) -> Option<PathBuf> {
    if program.is_empty() || program.contains('/') {
        return None;
    }
    for dir in path.split(':') {
        if dir.is_empty() {
            continue;
        }
        let candidate = Path::new(dir).join(program);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Admit graph selection. Recency ranking is not a fallback.
pub fn select_graph(path: &str) -> Result<SelectorReceipt, SelectorError> {
    match resolve_on_path(PROGRAM, path) {
        Some(resolved) => Ok(SelectorReceipt {
            program: PROGRAM,
            resolved,
        }),
        None => Err(SelectorError::Unavailable { program: PROGRAM }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_path_is_unavailable_not_success() {
        let err = select_graph("").expect_err("empty PATH");
        assert_eq!(
            err.to_string(),
            "SELECTOR_UNAVAILABLE program=bv",
            "{err}"
        );
    }

    #[test]
    fn slash_in_program_name_is_rejected() {
        assert!(resolve_on_path("bv/evil", "/usr/bin").is_none());
    }
}
