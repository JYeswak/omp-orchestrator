#![forbid(unsafe_code)]

//! Fleet-wide pane SET compare (session:index), not busy labels, not %N.
//! Empty oracle is ERROR. Empty product vs live oracle is DISAGREEMENT (ntm#254).

use oracle_compare::{compare_sets, set_delta, OracleCompareRules, OracleCompareVerdict, SetArm};
use serde_json::Value;
use std::collections::BTreeSet;

pub fn parse_tmux_keys(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .map(|s| s.to_string())
        .collect()
}

#[allow(clippy::result_unit_err)]
pub fn parse_ntm_keys(session: &str, json: &str) -> Result<BTreeSet<String>, ()> {
    let v: Value = serde_json::from_str(json).map_err(|_| ())?;
    if v.get("success") != Some(&Value::Bool(true)) {
        return Err(());
    }
    // Original python: s=d.get('session',''). Prefer the JSON session so both arms
    // deserialize the same identity (C14). Fall back to the list-sessions name only
    // when the field is missing, which live ntm always populates.
    let sess = v
        .get("session")
        .and_then(|x| x.as_str())
        .filter(|s| !s.is_empty())
        .unwrap_or(session);
    let mut out = BTreeSet::new();
    let ags = v.get("agents").ok_or(())?;
    let arr = ags.as_array().ok_or(())?;
    for a in arr {
        let idx = a.get("pane_idx").or_else(|| a.get("pane")).and_then(|x| {
            x.as_u64()
                .or_else(|| x.as_str().and_then(|s| s.parse().ok()))
        });
        if let Some(i) = idx {
            out.insert(format!("{sess}:{i}"));
        }
    }
    Ok(out)
}

pub fn diff_sets(
    oracle: BTreeSet<String>,
    product: Result<BTreeSet<String>, ()>,
    rules: &OracleCompareRules,
) -> (OracleCompareVerdict, Vec<String>, Vec<String>) {
    let product_arm = match product {
        Ok(s) => SetArm::Value(s),
        Err(()) => SetArm::Unreadable,
    };
    let o2 = oracle.clone();
    let p2 = match &product_arm {
        SetArm::Value(s) => s.clone(),
        SetArm::Unreadable => BTreeSet::new(),
    };
    let v = compare_sets(SetArm::Value(oracle), product_arm, rules);
    let (only_o, only_p) = set_delta(&o2, &p2);
    (
        v,
        only_o.into_iter().map(|s| s.to_string()).collect(),
        only_p.into_iter().map(|s| s.to_string()).collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ntm_uses_session_index_not_percent_id() {
        let j = r#"{"success":true,"agents":[{"pane":"0","pane_idx":0},{"pane":"2"}]}"#;
        let s = parse_ntm_keys("control-plane", j).unwrap();
        assert!(s.contains("control-plane:0"));
        assert!(s.contains("control-plane:2"));
        assert!(!s.iter().any(|x| x.contains('%')));
    }

    #[test]
    fn unparseable_ntm_is_error() {
        assert!(parse_ntm_keys("s", "nope").is_err());
        assert!(parse_ntm_keys("s", r#"{"success":false,"agents":[]}"#).is_err());
    }
}
