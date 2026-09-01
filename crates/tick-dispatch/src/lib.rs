#![forbid(unsafe_code)]

//! Admission order matches bin/tick-dispatch.sh. Do not skip a refuse.
//! ntm / pane-truth / discriminator / check.sh / fence stay EXTERNAL.

#[derive(Clone, Debug)]
pub struct TickDispatchRules {
    pub refuse_busy: bool,
    pub refuse_disc: bool,
    pub refuse_empty_render: bool,
    pub refuse_check: bool,
    pub refuse_ready: bool,
    pub refuse_unverified_send: bool,
}

impl Default for TickDispatchRules {
    fn default() -> Self {
        Self {
            refuse_busy: true,
            refuse_disc: true,
            refuse_empty_render: true,
            refuse_check: true,
            refuse_ready: true,
            refuse_unverified_send: true,
        }
    }
}

impl TickDispatchRules {
    pub fn disable(&mut self, name: &str) -> bool {
        match name {
            "refuse_busy" => self.refuse_busy = false,
            "refuse_disc" => self.refuse_disc = false,
            "refuse_empty_render" => self.refuse_empty_render = false,
            "refuse_check" => self.refuse_check = false,
            "refuse_ready" => self.refuse_ready = false,
            "refuse_unverified_send" => self.refuse_unverified_send = false,
            _ => return false,
        }
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TickDispatchDecision {
    Allow,
    Refuse {
        exit: i32,
        reason: &'static str,
        detail: String,
    },
}

pub fn pane_decision(verdict: &str, force_busy: bool, rules: &TickDispatchRules) -> TickDispatchDecision {
    match verdict {
        "DONE" | "IDLE" => TickDispatchDecision::Allow,
        other => {
            if !rules.refuse_busy || force_busy {
                TickDispatchDecision::Allow
            } else {
                TickDispatchDecision::Refuse {
                    exit: 1,
                    reason: "refuse_busy",
                    detail: format!(
                        "REFUSED: pane is {other}, not DONE/IDLE. A free pane is not the same as a finished bead, and a working pane is not a free one."
                    ),
                }
            }
        }
    }
}

pub fn disc_decision(rc: i32, rules: &TickDispatchRules) -> TickDispatchDecision {
    if !rules.refuse_disc {
        return TickDispatchDecision::Allow;
    }
    match rc {
        0 => TickDispatchDecision::Allow,
        1 => TickDispatchDecision::Refuse {
            exit: 1,
            reason: "refuse_disc",
            detail: "REFUSED: pane error discriminator found a terminated non-zero failure.".into(),
        },
        2 => TickDispatchDecision::Refuse {
            exit: 1,
            reason: "refuse_disc",
            detail: "REFUSED: pane error discriminator is UNKNOWN; refusing dispatch.".into(),
        },
        other => TickDispatchDecision::Refuse {
            exit: 1,
            reason: "refuse_disc",
            detail: format!("REFUSED: pane error discriminator unavailable (rc={other})."),
        },
    }
}

pub fn render_decision(empty: bool, rules: &TickDispatchRules) -> TickDispatchDecision {
    if empty && rules.refuse_empty_render {
        TickDispatchDecision::Refuse {
            exit: 1,
            reason: "refuse_empty_render",
            detail: "preflight: RENDER FAILED — refusing to send an unscanned packet".into(),
        }
    } else {
        TickDispatchDecision::Allow
    }
}

pub fn check_decision(rc: i32, rules: &TickDispatchRules) -> TickDispatchDecision {
    if rc != 0 && rules.refuse_check {
        TickDispatchDecision::Refuse {
            exit: 1,
            reason: "refuse_check",
            detail: "runtime admission: REFUSED — no admissible standing check.sh verdict".into(),
        }
    } else {
        TickDispatchDecision::Allow
    }
}

pub fn ready_decision(rc: i32, pane: &str, rules: &TickDispatchRules) -> TickDispatchDecision {
    if rc != 0 && rules.refuse_ready {
        TickDispatchDecision::Refuse {
            exit: 1,
            reason: "refuse_ready",
            detail: format!("ground-truth readiness: REFUSED — pane {pane} is not proven FREE"),
        }
    } else {
        TickDispatchDecision::Allow
    }
}

pub fn send_decision(send_rc: i32, jq_success: bool, rules: &TickDispatchRules) -> TickDispatchDecision {
    if send_rc != 0 {
        TickDispatchDecision::Refuse {
            exit: send_rc,
            reason: "refuse_unverified_send",
            detail: format!("robot send: REFUSED/FAILED (rc={send_rc})"),
        }
    } else if !jq_success && rules.refuse_unverified_send {
        TickDispatchDecision::Refuse {
            exit: 1,
            reason: "refuse_unverified_send",
            detail: "robot send: UNVERIFIED — structured response did not report success".into(),
        }
    } else {
        TickDispatchDecision::Allow
    }
}

/// Walk the shell's admission order. First refuse wins.
#[allow(clippy::too_many_arguments)]
pub fn admit(
    verdict: &str,
    force_busy: bool,
    disc_rc: i32,
    rendered_empty: bool,
    check_rc: i32,
    ready_rc: i32,
    pane: &str,
    rules: &TickDispatchRules,
) -> TickDispatchDecision {
    let d = pane_decision(verdict, force_busy, rules);
    if let TickDispatchDecision::Refuse { .. } = d {
        return d;
    }
    let d = disc_decision(disc_rc, rules);
    if let TickDispatchDecision::Refuse { .. } = d {
        return d;
    }
    let d = render_decision(rendered_empty, rules);
    if let TickDispatchDecision::Refuse { .. } = d {
        return d;
    }
    let d = check_decision(check_rc, rules);
    if let TickDispatchDecision::Refuse { .. } = d {
        return d;
    }
    ready_decision(ready_rc, pane, rules)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn done_and_idle_admit() {
        let r = TickDispatchRules::default();
        assert_eq!(pane_decision("DONE", false, &r), TickDispatchDecision::Allow);
        assert_eq!(pane_decision("IDLE", false, &r), TickDispatchDecision::Allow);
    }

    #[test]
    fn working_refuses_without_force() {
        let r = TickDispatchRules::default();
        assert!(matches!(
            pane_decision("WORKING", false, &r),
            TickDispatchDecision::Refuse {
                reason: "refuse_busy",
                ..
            }
        ));
        assert_eq!(pane_decision("WORKING", true, &r), TickDispatchDecision::Allow);
    }
}
