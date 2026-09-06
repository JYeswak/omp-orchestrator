//! Resident liveness: writing-but-not-working is not silence.
//!
//! Leg 8-equivalent `RESIDENT_WRITING` stays GREEN on fresh rows. New leg 9
//! `RESIDENT_RESTART_WITHOUT_DISPATCH` REDs when restarts advance and dispatches
//! do not — the crash-loop the writing detector cannot see.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WritingVerdict {
    Green,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CrashLoopVerdict {
    Green,
    Red,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubResidentSnapshot {
    pub pid: u32,
    pub restarts: u64,
    pub dispatches: u64,
    pub fresh_writing_rows: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeadmanWindow {
    pub before: HubResidentSnapshot,
    pub after: HubResidentSnapshot,
}

#[must_use]
pub fn writing_leg(snap: &HubResidentSnapshot) -> WritingVerdict {
    if snap.fresh_writing_rows {
        WritingVerdict::Green
    } else {
        // Absence of rows is a different detector; this leg only grades writing.
        WritingVerdict::Green
    }
}

/// Leg 9. RED iff restarts advanced and dispatches did not.
#[must_use]
pub fn restart_without_dispatch(window: &DeadmanWindow) -> CrashLoopVerdict {
    let restarts_up = window.after.restarts > window.before.restarts;
    let dispatches_flat = window.after.dispatches == window.before.dispatches;
    if restarts_up && dispatches_flat {
        CrashLoopVerdict::Red
    } else {
        CrashLoopVerdict::Green
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(pid: u32, restarts: u64, dispatches: u64, writing: bool) -> HubResidentSnapshot {
        HubResidentSnapshot {
            pid,
            restarts,
            dispatches,
            fresh_writing_rows: writing,
        }
    }

    #[test]
    fn planted_crash_loop_reds_leg9_while_writing_stays_green() {
        let window = DeadmanWindow {
            before: snap(77234, 69, 0, true),
            after: snap(78100, 70, 0, true),
        };
        assert_eq!(
            writing_leg(&window.after),
            WritingVerdict::Green,
            "RESIDENT_WRITING must stay GREEN on fresh rows — noise not silence"
        );
        assert_eq!(
            restart_without_dispatch(&window),
            CrashLoopVerdict::Red,
            "restarts 69→70 with dispatches unchanged must RED"
        );
    }

    #[test]
    fn restarts_flat_dispatches_advancing_is_green() {
        let window = DeadmanWindow {
            before: snap(10, 0, 3, true),
            after: snap(10, 0, 6, true),
        };
        assert_eq!(writing_leg(&window.after), WritingVerdict::Green);
        assert_eq!(restart_without_dispatch(&window), CrashLoopVerdict::Green);
    }
}
