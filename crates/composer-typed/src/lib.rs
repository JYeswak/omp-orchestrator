#![forbid(unsafe_code)]

//! Composer occupancy discriminator, port of `bin/composer-typed.py`.
//!
//! exit 0 = typed operator text (pane is NOT free)
//! exit 1 = bare prompt OR a greyed autosuggestion (pane IS free)
//!
//! This is NOT a liveness classifier and does not rank on ntm labels.

use regex::Regex;
use std::sync::OnceLock;

pub const MARKERS: &[&str] = &["❯", "›"];
pub const STRIP: &str = " \t\r\n │╰╮╯╭─\u{00a0}";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rule {
    DimSuggestionIsNotTyped,
    BrightBodyIsTyped,
}

impl Rule {
    pub const ALL: &'static [Rule] = &[
        Rule::DimSuggestionIsNotTyped,
        Rule::BrightBodyIsTyped,
    ];
    pub fn as_str(self) -> &'static str {
        match self {
            Rule::DimSuggestionIsNotTyped => "dim_suggestion_is_not_typed",
            Rule::BrightBodyIsTyped => "bright_body_is_typed",
        }
    }
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|r| r.as_str() == name)
    }
}

#[derive(Clone, Debug)]
pub struct Rules {
    pub dim_suggestion_is_not_typed: bool,
    pub bright_body_is_typed: bool,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            dim_suggestion_is_not_typed: true,
            bright_body_is_typed: true,
        }
    }
}

impl Rules {
    pub fn disable(&mut self, name: &str) -> bool {
        let Some(rule) = Rule::parse(name) else {
            return false;
        };
        match rule {
            Rule::DimSuggestionIsNotTyped => self.dim_suggestion_is_not_typed = false,
            Rule::BrightBodyIsTyped => self.bright_body_is_typed = false,
        }
        true
    }
}

fn sgr() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new("\u{1b}\\[([0-9;]*)m").expect("SGR"))
}

fn dim_fg(codes: &str) -> bool {
    let parts: Vec<&str> = codes.split(';').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return false;
    }
    if parts == ["2"] {
        return true;
    }
    if parts.len() >= 3 && parts[0] == "38" && parts[1] == "5" {
        if let Ok(n) = parts[2].parse::<i32>() {
            return (232..=250).contains(&n) || (244..=248).contains(&n);
        }
    }
    if parts.len() >= 5 && parts[0] == "38" && parts[1] == "2" {
        if let (Ok(r), Ok(g), Ok(b)) = (
            parts[2].parse::<i32>(),
            parts[3].parse::<i32>(),
            parts[4].parse::<i32>(),
        ) {
            let mx = r.max(g).max(b);
            let mn = r.min(g).min(b);
            return mx <= 205 && (mx - mn) <= 20;
        }
    }
    false
}

fn sets_fg(codes: &str) -> bool {
    let parts: Vec<&str> = codes.split(';').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return true;
    }
    if parts[0] == "0" || parts[0] == "39" {
        return true;
    }
    if parts[0] == "38" || parts[0] == "2" {
        return true;
    }
    if parts[0].len() == 2 && parts[0].starts_with('3') {
        return true;
    }
    false
}

fn next_dim(cur: bool, c: &str) -> bool {
    if !sets_fg(c) {
        return cur;
    }
    if c.is_empty() || c == "0" || c == "39" {
        return false;
    }
    dim_fg(c)
}

fn fg_dim_state(codes_seq: &[String]) -> bool {
    let mut st = false;
    for c in codes_seq {
        st = next_dim(st, c);
    }
    st
}

fn marker_idx(line: &str) -> Option<usize> {
    MARKERS
        .iter()
        .filter_map(|m| line.find(m).map(|j| j + m.len()))
        .min()
}

fn strip_empty(s: &str) -> bool {
    !s.trim_matches(|c: char| STRIP.contains(c)).is_empty()
}

fn typed_ansi(line: &str, rules: &Rules) -> bool {
    let Some(idx) = marker_idx(line) else {
        return false;
    };
    let pre: Vec<String> = sgr()
        .captures_iter(&line[..idx])
        .map(|c| c[1].to_string())
        .collect();
    let mut cur_dim = fg_dim_state(&pre);
    let rest = &line[idx..];
    let mut pos = 0usize;
    for mo in sgr().find_iter(rest) {
        let chunk = &rest[pos..mo.start()];
        if strip_empty(chunk) && !cur_dim && rules.bright_body_is_typed {
            return true;
        }
        let codes = sgr()
            .captures(mo.as_str())
            .map(|c| c[1].to_string())
            .unwrap_or_default();
        cur_dim = next_dim(cur_dim, &codes);
        if !rules.dim_suggestion_is_not_typed {
            cur_dim = false;
        }
        pos = mo.end();
    }
    let tail = &rest[pos..];
    strip_empty(tail) && !cur_dim && rules.bright_body_is_typed
}

fn typed_plain(line: &str, rules: &Rules) -> bool {
    let Some(idx) = marker_idx(line) else {
        return false;
    };
    strip_empty(&line[idx..]) && rules.bright_body_is_typed
}

/// True iff any line holds typed operator text.
///
/// # THE PERMISSIVE DIRECTION IS DIFFERENT FOR EACH CONSUMER CLASS. READ THIS BEFORE TRUSTING A
/// NAME.
///
/// This function answers ONE question \u2014 *does the composer hold typed text?* \u2014 and `false` is its
/// negative answer. Whether `false` is the SAFE answer depends entirely on what the caller is
/// asking, and the two classes want opposite things from the same value:
///
/// | consumer class | the question | `false` means | permissive direction |
/// |---|---|---|---|
/// | OCCUPANCY (this crate, `main.rs`) | is there typed text? | no text found | `false` is simply NEGATIVE, and correct |
/// | DISPATCH (a gate deciding "may I send here?") | is this pane safe to send to? | pane looks free | `false` is ADMITTING, and unsafe when the input was unreadable |
///
/// So `is_typed("") == false` is RIGHT for occupancy and DANGEROUS for dispatch, and the binary's
/// exit mapping (`main.rs:3`, `0=TYPED, 1=FREE`) turns it into rc=1 = FREE = ADMIT.
///
/// ⛔ THE EMPTY CASE IS THE CALLER'S TO GUARD, AND THIS CRATE WILL NOT GUARD IT FOR YOU. An empty
/// or all-whitespace capture is not evidence the composer is free \u2014 it is the absence of evidence,
/// and a dispatch gate must treat it as OCCUPIED. `fast-dispatch` does exactly that at the caller
/// (`composer_occupied` refuses on an all-whitespace capture BEFORE calling this function), which
/// is the correct shape: the occupancy kernel stays honest about what it saw, and the dispatch
/// consumer supplies its own fail-closed direction.
///
/// # Why there is no flag for this
///
/// There was one. `Rules::fail_closed_on_empty` was declared, defaulted `true`, disableable by
/// name, and named by a test \u2014 and READ BY NOTHING, while the empty case below was hard-coded. It
/// was deleted rather than wired, because wiring it would have flipped this function's answer on
/// empty input and changed what `omp-orchestrator`'s in-process consumer observes
/// (`resident.rs:3837`), which is a different subject with its own grade. A flag that cannot
/// change an answer while a test carries its name is a check that cannot fire reading as
/// protection \u2014 and the name was inverted for dispatch callers on top of that.
pub fn is_typed(data: &str, rules: &Rules) -> bool {
    if data.is_empty() {
        return false;
    }
    let has_ansi = data.contains("\u{1b}[");
    for line in data.split('\n') {
        let hit = if has_ansi {
            typed_ansi(line, rules)
        } else {
            typed_plain(line, rules)
        };
        if hit {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_named_rule_is_disableable() {
        for rule in Rule::ALL {
            let mut g = Rules::default();
            assert!(g.disable(rule.as_str()), "{}", rule.as_str());
        }
    }

    #[test]
    fn crate_does_not_widen_admission() {
        let lib = include_str!("lib.rs");
        let token = format!("{}_{}", "ADMISSION", "FRESH");
        assert!(!lib.contains(&token));
    }
}
