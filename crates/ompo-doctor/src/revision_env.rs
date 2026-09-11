/// Shared revision-resolution decision table for build provenance.
///
/// Used from TWO places that must never disagree: `build.rs` (via `include!`,
/// which pastes this file mid-module, so these are outer docs on the item
/// below) and the unit legs in this file (via `mod revision_env`).
/// Whitespace-only and empty values count as ABSENT: an env var containing
/// spaces is not a revision, and treating it as one would stamp garbage.
#[must_use]
pub fn clean_env_value(value: Option<String>) -> Option<String> {
    value
        .map(|raw| raw.trim().to_owned())
        .filter(|trimmed| !trimmed.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_and_missing_env_counts_as_absent() {
        assert_eq!(clean_env_value(None), None);
        assert_eq!(clean_env_value(Some(String::new())), None);
        assert_eq!(clean_env_value(Some("   ".to_owned())), None);
        assert_eq!(clean_env_value(Some("\t\n ".to_owned())), None);
    }

    #[test]
    fn present_value_survives_trimmed() {
        assert_eq!(
            clean_env_value(Some("  abc123  ".to_owned())),
            Some("abc123".to_owned())
        );
        assert_eq!(
            clean_env_value(Some("nogit-1".to_owned())),
            Some("nogit-1".to_owned())
        );
    }
}
