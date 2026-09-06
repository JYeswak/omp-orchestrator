#![forbid(unsafe_code)]

//! Upstream OMP `IrcDeliveryReceipt` (`irc/bus.d.ts`).
//!
//! Outcomes are how a message reached an IRC-bus recipient (`injected` / `woken` /
//! `revived` / `failed`) — **not** what they did with it, and **not** a tmux/ntm
//! pane send. Pane transport cannot produce this type; mapping sender exit onto it
//! is the cp-z42vu shape (sender success read as delivery).

use std::fmt;

/// Byte-for-byte the upstream interface in `dist/types/irc/bus.d.ts`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IrcDeliveryReceipt {
    pub to: String,
    pub outcome: IrcDeliveryOutcome,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrcDeliveryOutcome {
    Injected,
    Woken,
    Revived,
    Failed,
}

impl IrcDeliveryOutcome {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Injected => "injected",
            Self::Woken => "woken",
            Self::Revived => "revived",
            Self::Failed => "failed",
        }
    }
}

/// Receiver evidence taken from an upstream hub/IRC receipt — never from sender exit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubReceiverEvidence {
    pub receipt: IrcDeliveryReceipt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaneIrcRefusal {
    PaneTransportHasNoIrcReceipt {
        transport: String,
        sender_exit: i32,
    },
}

impl fmt::Display for PaneIrcRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PaneTransportHasNoIrcReceipt {
                transport,
                sender_exit,
            } => write!(
                f,
                "PANE_TRANSPORT_HAS_NO_IRC_RECEIPT transport={transport} sender_exit={sender_exit} \
                 upstream=IrcDeliveryReceipt plane=irc-bus — ntm/tmux cannot emit this type"
            ),
        }
    }
}

/// Hub send: record the upstream receipt as receiver evidence, not sender exit.
pub fn record_hub_irc_receipt(receipt: IrcDeliveryReceipt) -> HubReceiverEvidence {
    HubReceiverEvidence { receipt }
}

/// Pane send: typed reason the transport cannot use IrcDeliveryReceipt.
/// Sender exit is an argument so a caller cannot pretend it was unused.
pub fn pane_transport_cannot_use_irc_receipt(
    transport: &str,
    sender_exit: i32,
) -> Result<IrcDeliveryReceipt, PaneIrcRefusal> {
    let _ = sender_exit;
    Err(PaneIrcRefusal::PaneTransportHasNoIrcReceipt {
        transport: transport.to_owned(),
        sender_exit,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hub_receipt_is_receiver_evidence_not_sender_exit() {
        let receipt = IrcDeliveryReceipt {
            to: "agent-b".to_owned(),
            outcome: IrcDeliveryOutcome::Woken,
            error: None,
        };
        let evidence = record_hub_irc_receipt(receipt.clone());
        assert_eq!(evidence.receipt.outcome, IrcDeliveryOutcome::Woken);
        assert_ne!(evidence.receipt.outcome.as_str(), "0");
    }

    #[test]
    fn pane_transport_records_why_it_cannot_use_the_type() {
        let transport = format!(
            "{}-{}",
            tick_monitor::NTM,
            tick_monitor::ntm_send_flag().trim_start_matches('-')
        );
        let err = pane_transport_cannot_use_irc_receipt(&transport, 0)
            .expect_err("sender exit 0 is not an IRC receipt");
        let text = err.to_string();
        assert!(text.contains("PANE_TRANSPORT_HAS_NO_IRC_RECEIPT"));
        assert!(text.contains("IrcDeliveryReceipt"));
        assert!(text.contains("sender_exit=0"));
    }
}
