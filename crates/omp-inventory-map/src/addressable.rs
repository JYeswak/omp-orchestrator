//! ADDRESSABLE: one documented command runs the gate, and `--help` names it.
//!
//! Bead `omp-orchestrator-plan-04-7wn9.1`. A help payload that returns
//! `CONFIG_ERROR` or omits the run command (`doctor`) is not addressable.

use serde_json::{Value, json};

/// Documented command that runs the inventory gate.
pub const RUN_COMMAND: &str = "doctor";

/// Subcommands `--help` must name.
pub const SUBCOMMANDS: &[&str] = &[
    "doctor", "health", "audit", "version", "types", "help",
];

/// Flags `--help` must name.
pub const FLAGS: &[&str] = &[
    "--help", "--json", "--repo", "--omp", "--cargo", "--find",
];

/// Failures of the ADDRESSABLE property.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AddressableError {
    ConfigErrorHelp,
    OmitsRunCommand {
        run_command: &'static str,
    },
    OmitsSurface {
        token: &'static str,
    },
}

impl std::fmt::Display for AddressableError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConfigErrorHelp => formatter.write_str(
                "ADDRESSABLE_REFUSED --help returned CONFIG_ERROR; a gate nobody can invoke does not exist",
            ),
            Self::OmitsRunCommand { run_command } => write!(
                formatter,
                "ADDRESSABLE_OMITS_RUN_COMMAND --help does not name {run_command}"
            ),
            Self::OmitsSurface { token } => write!(
                formatter,
                "ADDRESSABLE_OMITS_SURFACE --help does not name {token}"
            ),
        }
    }
}

impl std::error::Error for AddressableError {}

pub fn help_data() -> Value {
    let usage = format!(
        "omp-inventory-map [{}] [{}]",
        SUBCOMMANDS.join("|"),
        FLAGS.iter()
            .map(|flag| if *flag == "--help" {
                "--help".to_owned()
            } else if *flag == "--json" {
                "--json".to_owned()
            } else {
                format!("{flag} PATH")
            })
            .collect::<Vec<_>>()
            .join("] [")
    );
    json!({
        "usage": usage,
        "run_command": RUN_COMMAND,
        "subcommands": SUBCOMMANDS,
        "flags": FLAGS,
        "notes": "doctor is the documented command that runs the inventory gate. types runs the type-collision gate.",
    })
}

/// Refuse CONFIG_ERROR help and any surface token missing from the payload.
pub fn check_addressable(help_text: &str) -> Result<(), AddressableError> {
    if help_text.contains("CONFIG_ERROR") {
        return Err(AddressableError::ConfigErrorHelp);
    }
    if !help_text.contains(RUN_COMMAND) {
        return Err(AddressableError::OmitsRunCommand {
            run_command: RUN_COMMAND,
        });
    }
    for token in SUBCOMMANDS.iter().chain(FLAGS.iter()) {
        if !help_text.contains(token) {
            return Err(AddressableError::OmitsSurface { token });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_good_help_names_doctor_and_every_flag() {
        let text = serde_json::to_string_pretty(&help_data()).expect("json");
        check_addressable(&text).expect("canonical help is ADDRESSABLE");
        assert!(text.contains("doctor"));
    }

    #[test]
    fn known_bad_config_error_help_is_not_addressable() {
        let help = r#"{"status":"ERROR","error":"CONFIG_ERROR unknown argument --help"}"#;
        assert_eq!(
            check_addressable(help),
            Err(AddressableError::ConfigErrorHelp)
        );
    }

    #[test]
    fn known_bad_help_that_omits_doctor_is_not_addressable() {
        let help = r#"usage: omp-inventory-map [health] [--json]"#;
        assert_eq!(
            check_addressable(help),
            Err(AddressableError::OmitsRunCommand {
                run_command: "doctor"
            })
        );
    }
}
