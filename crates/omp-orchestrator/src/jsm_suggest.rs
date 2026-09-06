#![forbid(unsafe_code)]

//! Parser for `jsm suggest --json` (12.68).
//!
//! The historical silent-success: a grep that did not match jsm's envelope
//! reported zero skills. This parser keys on `suggestions[].skill_name`.

use serde_json::Value;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsmSuggest {
    pub success: bool,
    pub skill_names: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    InvalidJson,
    MissingSuggestions,
    SuggestionNotObject,
    MissingSkillName,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidJson => formatter.write_str("JSM_SUGGEST_INVALID_JSON"),
            Self::MissingSuggestions => formatter.write_str("JSM_SUGGEST_MISSING_SUGGESTIONS"),
            Self::SuggestionNotObject => formatter.write_str("JSM_SUGGEST_ROW_NOT_OBJECT"),
            Self::MissingSkillName => formatter.write_str("JSM_SUGGEST_MISSING_SKILL_NAME"),
        }
    }
}

pub fn parse_jsm_suggest(text: &str) -> Result<JsmSuggest, ParseError> {
    let value: Value = serde_json::from_str(text.trim()).map_err(|_| ParseError::InvalidJson)?;
    let success = value.get("success").and_then(Value::as_bool).unwrap_or(false);
    let rows = value
        .get("suggestions")
        .and_then(Value::as_array)
        .ok_or(ParseError::MissingSuggestions)?;
    let mut skill_names = Vec::new();
    for row in rows {
        let object = row.as_object().ok_or(ParseError::SuggestionNotObject)?;
        let name = object
            .get("skill_name")
            .and_then(Value::as_str)
            .ok_or(ParseError::MissingSkillName)?;
        if !name.is_empty() {
            skill_names.push(name.to_owned());
        }
    }
    Ok(JsmSuggest {
        success,
        skill_names,
    })
}

pub fn skill_count(parsed: &JsmSuggest) -> usize {
    parsed.skill_names.len()
}

pub fn count_named(parsed: &JsmSuggest, name: &str) -> usize {
    parsed
        .skill_names
        .iter()
        .filter(|skill| skill.as_str() == name)
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KNOWN_PRESENT: &str = r#"{
        "success": true,
        "suggestions": [
            {"rank": 1, "skill_name": "ntm", "reason": "Context match"},
            {"rank": 2, "skill_name": "asupersync-mega-skill", "reason": "Context match"}
        ]
    }"#;

    #[test]
    fn positive_control_returns_nonzero_on_known_present_skill() {
        let parsed = parse_jsm_suggest(KNOWN_PRESENT).expect("fixture parses");
        assert!(parsed.success);
        assert_eq!(skill_count(&parsed), 2);
        assert!(
            count_named(&parsed, "ntm") > 0,
            "known-present skill ntm must be non-zero"
        );
    }

    #[test]
    fn empty_suggestions_is_zero_not_a_pass_for_named_skill() {
        let parsed = parse_jsm_suggest(r#"{"success":true,"suggestions":[]}"#).unwrap();
        assert_eq!(skill_count(&parsed), 0);
        assert_eq!(count_named(&parsed, "ntm"), 0);
    }

    #[test]
    fn wrong_shape_is_typed_error() {
        assert_eq!(
            parse_jsm_suggest("not json").unwrap_err(),
            ParseError::InvalidJson
        );
        assert_eq!(
            parse_jsm_suggest("{}").unwrap_err(),
            ParseError::MissingSuggestions
        );
    }
}
