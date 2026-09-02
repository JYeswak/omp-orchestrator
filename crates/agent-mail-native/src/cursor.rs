//! Two integer identities that Agent Mail hands back in the same JSON object
//! and that a caller must never interchange.
//!
//! Measured 2026-09-02 against the live daemon (`fetch_inbox_events`, recipient
//! `GreenFrog`): a single delivery event carried `cursor=2108` and
//! `message_id=36618`. Both are integers, both name "the event", and the
//! server's own tool description says `after` "is a delivery cursor, never a
//! message id". Passing one where the other belongs is therefore a live,
//! silent, plausible bug: `after=36618` is not a type error against a `u64`
//! parameter, it is a `CURSOR_AHEAD` refusal at best and a skipped backlog at
//! worst.
//!
//! Newtypes make that swap a compile error instead of a runtime surprise.

use serde::{Deserialize, Serialize};
use std::fmt;

/// A position in Agent Mail's durable, append-only delivery sequence.
///
/// The sequence is GLOBAL and monotonic across the project; `tail_cursor` is
/// this recipient's high-water mark within it. Measured simultaneously on
/// 2026-09-02: `GreenFrog` tail 5142, `AmberGate` tail 5140, `SnowyCanyon`
/// tail 0 with `oldest_available_cursor: null` (a registered agent that has
/// never been delivered an event). So a cursor is only meaningful paired with
/// the recipient it was read for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeliveryCursor(u64);

impl DeliveryCursor {
    /// The position before any event. Requesting `after: 0` is "replay
    /// everything still retained", not "nothing".
    pub const ORIGIN: Self = Self(0);

    /// Wrap a raw cursor from the daemon.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw sequence position, for serialisation to the daemon only.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// True when `self` is strictly further along the sequence than `earlier`.
    ///
    /// This is the durability predicate a restart-safe monitor needs: a cursor
    /// that did not advance means no progress, and a cursor that went
    /// BACKWARDS means the monitor lost its place.
    #[must_use]
    pub const fn advanced_beyond(self, earlier: Self) -> bool {
        self.0 > earlier.0
    }
}

impl fmt::Display for DeliveryCursor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "cursor:{}", self.0)
    }
}

/// A message's durable primary key. NOT a position in the delivery sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageId(i64);

impl MessageId {
    /// Wrap a raw message id from the daemon.
    #[must_use]
    pub const fn new(raw: i64) -> Self {
        Self(raw)
    }

    /// The raw id, for serialisation to the daemon only.
    #[must_use]
    pub const fn get(self) -> i64 {
        self.0
    }
}

impl fmt::Display for MessageId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "message:{}", self.0)
    }
}

/// How to position a read of the durable delivery log.
///
/// The daemon documents `position_now` as "cannot be combined with `after`".
/// Encoding that as an enum makes the illegal combination unrepresentable, so
/// the rule cannot be violated by a caller who never read the tool docs — and
/// there is no runtime validation branch to test, because there is no way to
/// express the invalid request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorQuery {
    /// Return this recipient's durable tail cursor and NO events.
    ///
    /// This is how a fresh monitor establishes a baseline without consuming
    /// backlog it has not processed.
    PositionNow,
    /// Return events strictly after `cursor`.
    After(DeliveryCursor),
    /// Return events from the oldest still-retained position.
    FromOldestRetained,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cursor_and_message_id_do_not_interchange() {
        // The measured pair from one real event: cursor 2108, message 36618.
        let cursor = DeliveryCursor::new(2108);
        let message = MessageId::new(36618);
        // Distinct raw values, distinct types, distinct Display prefixes.
        assert_eq!(cursor.get(), 2108);
        assert_eq!(message.get(), 36618);
        assert_eq!(cursor.to_string(), "cursor:2108");
        assert_eq!(message.to_string(), "message:36618");
    }

    #[test]
    fn advancement_is_strict_and_detects_regression() {
        let baseline = DeliveryCursor::new(5142);
        assert!(
            DeliveryCursor::new(5157).advanced_beyond(baseline),
            "a later cursor must read as advanced"
        );
        assert!(
            !baseline.advanced_beyond(baseline),
            "an unchanged cursor is NOT progress"
        );
        assert!(
            !DeliveryCursor::new(5100).advanced_beyond(baseline),
            "a regressed cursor must never read as advanced"
        );
    }

    #[test]
    fn origin_is_replay_not_nothing() {
        assert_eq!(DeliveryCursor::ORIGIN.get(), 0);
        assert!(DeliveryCursor::new(1).advanced_beyond(DeliveryCursor::ORIGIN));
    }

    #[test]
    fn cursor_serialises_as_a_bare_integer() {
        // The daemon expects `{"after": 5142}`, not `{"after": {"0": 5142}}`.
        let encoded = serde_json::to_string(&DeliveryCursor::new(5142)).expect("encode");
        assert_eq!(encoded, "5142");
        let decoded: DeliveryCursor = serde_json::from_str("5142").expect("decode");
        assert_eq!(decoded, DeliveryCursor::new(5142));
    }
}
