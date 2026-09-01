#![forbid(unsafe_code)]

//! Agent-pane census vs ntm. Existence only — never busy/error labels.
//! Oracle is `tmux list-panes` + `ps` children; ntm is the subject.

use oracle_compare::{compare_counts, CountArm, OracleCompareRules, OracleCompareVerdict};
use regex::Regex;
use std::sync::OnceLock;

pub const AGENT_CMD_RE: &str = r"(^|/)(claude|codex|grok|gemini|aider|cursor-agent)( |$)";

fn agent_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(AGENT_CMD_RE).expect("AGENT_CMD_RE"))
}

pub fn is_agent_command(cmd: &str) -> bool {
    agent_re().is_match(cmd)
}

pub fn census(
    oracle_n: u64,
    subject: Result<u64, ()>,
    session_visible: bool,
    rules: &OracleCompareRules,
) -> OracleCompareVerdict {
    // MATCH THE SHELL. When the session is visible and the oracle found zero AGENT
    // panes, run_diff PASSES without comparing the subject count (bin/pane-oracle-diff.sh
    // ~141-147). Zero agents is the true answer for a shells-only session; ntm
    // overcount on this path is NOT a finding in the original. Changing that would
    // change which pane states disagree.
    if oracle_n == 0 && session_visible {
        return OracleCompareVerdict::Agree { n: 0 };
    }
    let product = match subject {
        Ok(n) => CountArm::Value(n),
        Err(()) => CountArm::Unreadable,
    };
    compare_counts(CountArm::Value(oracle_n), product, session_visible, rules)
}

#[allow(clippy::result_unit_err)]
pub fn parse_subject_json(text: &str) -> Result<u64, ()> {
    let v: serde_json::Value = serde_json::from_str(text).map_err(|_| ())?;
    if v.get("success") != Some(&serde_json::Value::Bool(true)) {
        return Err(());
    }
    let ags = v.get("agents").ok_or(())?;
    let arr = ags.as_array().ok_or(())?;
    Ok(arr.len() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oracle_compare::{OracleCompareRules, OracleCompareVerdict};

    #[test]
    fn allow_list_matches_measured_commands() {
        assert!(is_agent_command(
            "claude --dangerously-skip-permissions --model claude-opus-4-8"
        ));
        assert!(is_agent_command(&format!(
            "node {}/.local/bin/codex --dangerously-bypass-approvals",
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|home| home.display().to_string())
                .unwrap_or_default()
        )));
        assert!(is_agent_command("grok --always-approve"));
        assert!(!is_agent_command("npm exec comfyui-mcp"));
        assert!(!is_agent_command("-zsh"));
        assert!(!is_agent_command("node /some/other/server.js"));
    }

    #[test]
    fn unparseable_subject_is_error() {
        assert!(parse_subject_json("nope").is_err());
        assert!(parse_subject_json(r#"{"success":false,"agents":[]}"#).is_err());
        assert_eq!(
            parse_subject_json(r#"{"success":true,"agents":[{},{}]}"#).unwrap(),
            2
        );
    }

    #[test]
    fn shells_only_passes_even_if_subject_overcounts() {
        let r = OracleCompareRules::default();
        assert_eq!(
            census(0, Ok(5), true, &r),
            OracleCompareVerdict::Agree { n: 0 },
            "shell original PASSes shells-only without comparing subject; do not invent a finding"
        );
        assert!(matches!(
            census(0, Err(()), false, &r),
            OracleCompareVerdict::Unmeasurable { .. }
        ));
    }
}
