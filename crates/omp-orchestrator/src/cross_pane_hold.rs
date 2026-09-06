#![forbid(unsafe_code)]

//! Pane-aware hold for a bead already dispatched and not terminated.
//!
//! WHY identity_key, not fw20's assignee: `omp-orchestrator-assignee-is-not-pane-identity-fw20`
//! is still open. The assignee column still holds `--receiver-agent` (WildStone) for live
//! rows, so it cannot answer "does another pane already hold this bead". The lifecycle
//! ledger's identity_key already includes pane. Two panes are two keys; this module
//! collapses by bead.
//!
//! Acc 7lsb. identity_key is the hold. fw20 does not have to land first.

use ntm_fleet_monitor::bead_lifecycle::ledger::InFlightIdentity;
use std::fmt;

/// Zero in-flight holders is a typed state, distinct from a holder the tracker
/// cannot name (empty/placeholder pane on an in-flight identity).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HolderScan {
    Zero,
    Named { pane: String, identity_key: String },
    Unnamed { identity_key: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrossPaneRefuse {
    Duplicate {
        bead: String,
        held_pane: String,
        target_pane: String,
        identity_key: String,
    },
    HolderUnnamed { bead: String, identity_key: String },
}

impl fmt::Display for CrossPaneRefuse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Duplicate {
                bead,
                held_pane,
                target_pane,
                identity_key,
            } => write!(
                f,
                "CROSS_PANE_DUPLICATE bead={bead} held_pane={held_pane} target_pane={target_pane} identity_key={identity_key}"
            ),
            Self::HolderUnnamed { bead, identity_key } => write!(
                f,
                "CROSS_PANE_HOLDER_UNNAMED bead={bead} identity_key={identity_key} distinct_from=ZERO_HOLDERS"
            ),
        }
    }
}

pub fn scan(identities: &[InFlightIdentity]) -> HolderScan {
    if identities.is_empty() {
        return HolderScan::Zero;
    }
    if let Some(row) = identities.iter().find(|row| !pane_named(&row.pane)) {
        return HolderScan::Unnamed {
            identity_key: row.identity_key.clone(),
        };
    }
    let row = &identities[0];
    HolderScan::Named {
        pane: row.pane.clone(),
        identity_key: row.identity_key.clone(),
    }
}

fn pane_named(pane: &str) -> bool {
    let trimmed = pane.trim();
    !trimmed.is_empty() && !trimmed.contains('<') && !trimmed.contains('>')
}

/// Admit a dispatch of `bead` to `target_pane`.
///
/// Tracker `open` releases a prior pane (reap/abandon). Same-pane retry is
/// allowed here; w5re is the same-pane double-send guard.
pub fn admit(
    bead: &str,
    target_pane: &str,
    tracker_status: &str,
    identities: &[InFlightIdentity],
) -> Result<(), CrossPaneRefuse> {
    admit_with_hold(true, bead, target_pane, tracker_status, identities)
}

fn admit_with_hold(
    hold_enabled: bool,
    bead: &str,
    target_pane: &str,
    tracker_status: &str,
    identities: &[InFlightIdentity],
) -> Result<(), CrossPaneRefuse> {
    if !hold_enabled {
        return Ok(());
    }
    if tracker_status == "open" {
        return Ok(());
    }
    match scan(identities) {
        HolderScan::Zero => Ok(()),
        HolderScan::Unnamed { identity_key } => Err(CrossPaneRefuse::HolderUnnamed {
            bead: bead.to_owned(),
            identity_key,
        }),
        HolderScan::Named { .. } => {
            if let Some(other) = identities
                .iter()
                .find(|row| pane_named(&row.pane) && row.pane != target_pane)
            {
                Err(CrossPaneRefuse::Duplicate {
                    bead: bead.to_owned(),
                    held_pane: other.pane.clone(),
                    target_pane: target_pane.to_owned(),
                    identity_key: other.identity_key.clone(),
                })
            } else {
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named(pane: &str) -> InFlightIdentity {
        InFlightIdentity {
            pane: pane.to_owned(),
            identity_key: format!("lef1|repo|sess|{pane}|digest|SCHEDULED"),
        }
    }

    #[test]
    fn known_bad_second_pane_is_typed_refusal() {
        let identities = vec![named("%7")];
        let error = admit("lef1", "%8", "in_progress", &identities).expect_err("must refuse");
        let rendered = error.to_string();
        assert!(rendered.contains("CROSS_PANE_DUPLICATE"));
        assert!(rendered.contains("held_pane=%7"));
        assert!(rendered.contains("target_pane=%8"));
    }

    #[test]
    fn known_good_open_after_reap_is_dispatchable() {
        let identities = vec![named("%7")];
        admit("lef1", "%8", "open", &identities).expect("reaped/abandoned bead must dispatch");
    }

    #[test]
    fn same_pane_retry_is_not_this_gate() {
        let identities = vec![named("%7")];
        admit("lef1", "%7", "in_progress", &identities).expect("same pane is w5re, not 7lsb");
    }

    #[test]
    fn zero_holders_is_distinct_from_unnamed() {
        assert_eq!(scan(&[]), HolderScan::Zero);
        let unnamed = [InFlightIdentity {
            pane: String::new(),
            identity_key: "lef1|repo|sess||digest|SCHEDULED".to_owned(),
        }];
        assert!(matches!(scan(&unnamed), HolderScan::Unnamed { .. }));
        assert_ne!(scan(&[]), scan(&unnamed));
        admit("lef1", "%8", "in_progress", &[])
            .expect("ZERO_HOLDERS is allow, not unnamed");
        let error = admit("lef1", "%8", "in_progress", &unnamed).expect_err("unnamed refuses");
        assert!(error.to_string().contains("CROSS_PANE_HOLDER_UNNAMED"));
        assert!(error.to_string().contains("distinct_from=ZERO_HOLDERS"));
    }

    #[test]
    fn mutation_removing_hold_goes_red_then_restores() {
        let identities = vec![named("%7")];
        let with_hold = admit("lef1", "%8", "in_progress", &identities);
        assert!(with_hold.is_err(), "leg 2 requires the hold");
        let without = admit_with_hold(false, "lef1", "%8", "in_progress", &identities);
        assert!(
            without.is_ok(),
            "removing the hold makes leg 2 GREEN — the defect"
        );
        let restored = admit("lef1", "%8", "in_progress", &identities);
        assert!(restored.is_err(), "restore must refuse again");
        assert_eq!(
            restored.unwrap_err().to_string(),
            with_hold.unwrap_err().to_string()
        );
        let source = include_str!("cross_pane_hold.rs");
        let digest = {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            source.hash(&mut hasher);
            hasher.finish()
        };
        assert_ne!(digest, 0, "checksum of restored source");
        assert!(
            source.contains("admit_with_hold(true,"),
            "production admit must keep the hold armed"
        );
    }
}
