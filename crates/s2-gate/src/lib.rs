#![forbid(unsafe_code)]

//! L5-METRIC-READBACK-OK ∈ {0,1}. Floor 1 before S2.
//! inception.json absent or missing required keys ⇒ metric 0 ⇒ S2_REFUSE.

use serde_json::Value;
use std::fmt;
use std::path::Path;

pub const FLOOR: u8 = 1;

pub const REQUIRED_KEYS: [&str; 7] = [
    "schema_version",
    "project_id",
    "repo_identity",
    "control_files",
    "host_capabilities",
    "required_tools",
    "trust_status",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum S2Refuse {
    MetricZero,
    MetricNotBit { got: u8 },
}

impl fmt::Display for S2Refuse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MetricZero => write!(
                f,
                "S2_REFUSE L5-METRIC-READBACK-OK=0 floor={FLOOR} — S2 must not start on 0"
            ),
            Self::MetricNotBit { got } => {
                write!(f, "S2_REFUSE L5-METRIC-READBACK-OK={got} not in {{0,1}}")
            }
        }
    }
}

/// Admit S2 only when the readback metric is 1.
pub fn admit(metric: u8) -> Result<(), S2Refuse> {
    match metric {
        0 => Err(S2Refuse::MetricZero),
        1 => Ok(()),
        got => Err(S2Refuse::MetricNotBit { got }),
    }
}

/// 1 iff inception.json exists and carries every required key; else 0.
pub fn readback_ok(inception: &Path) -> u8 {
    let Ok(bytes) = std::fs::read(inception) else {
        return 0;
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return 0;
    };
    let Some(object) = value.as_object() else {
        return 0;
    };
    if REQUIRED_KEYS.iter().all(|key| object.contains_key(*key)) {
        1
    } else {
        0
    }
}
