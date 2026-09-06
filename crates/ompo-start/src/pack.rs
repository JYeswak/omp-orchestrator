#![forbid(unsafe_code)]

//! Context pack receipt for `ntm send -t tick_zero`.
//!
//! DP-6: a send returning success/submitted is not a read. The spawn path
//! retains a pack receipt object. Send success without that object fails.

use crate::spawn::sha256_hex;
use std::fmt;

pub const TICK_ZERO_TARGET: &str = "tick_zero";

/// Typed pack receipt. Cannot be forged from an exit code.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackReceipt {
    target: String,
    pane_id: String,
    pack_sha256: String,
}

impl PackReceipt {
    pub fn new(pane_id: impl Into<String>, pack_bytes: &[u8]) -> Self {
        Self {
            target: TICK_ZERO_TARGET.to_owned(),
            pane_id: pane_id.into(),
            pack_sha256: sha256_hex(pack_bytes),
        }
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn pane_id(&self) -> &str {
        &self.pane_id
    }

    pub fn pack_sha256(&self) -> &str {
        &self.pack_sha256
    }
}

/// What a send returned. `success` without `receipt` is the DP-6 failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendAttempt {
    pub exit_code: i32,
    pub success: bool,
    pub receipt: Option<PackReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackError {
    SendWithoutReceipt { exit_code: i32, success: bool },
}

impl fmt::Display for PackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SendWithoutReceipt { exit_code, success } => write!(
                f,
                "PACK_SEND_WITHOUT_RECEIPT exit={exit_code} success={success} — DP-6: send is not a read"
            ),
        }
    }
}

impl std::error::Error for PackError {}

/// Retain the pack receipt. Exit 0 / success true with no receipt object fails.
pub fn retain_pack_receipt(attempt: SendAttempt) -> Result<PackReceipt, PackError> {
    match attempt.receipt {
        Some(receipt) if receipt.target() == TICK_ZERO_TARGET => Ok(receipt),
        Some(_) | None => Err(PackError::SendWithoutReceipt {
            exit_code: attempt.exit_code,
            success: attempt.success,
        }),
    }
}
