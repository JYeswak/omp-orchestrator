#![forbid(unsafe_code)]

//! L4-SPAWN: every spawned pane is registered in Agent Mail BEFORE `tick_zero`.
//!
//! A roster is not liveness — but registration IS a spawn receipt: it is the one
//! artifact that outlives the spawning process and can be read back by a third
//! party (`am robot agents`). This module reads that roster back and refuses a
//! spawn wave in which any pane is unregistered.
//!
//! `am agents register` carries no pane field (measured 2026-09-11 against the
//! installed `am`: `--project --program --model --name --task
//! --attachments-policy`), so the spawn path must carry the pane id inside the
//! registration identity. This module therefore accepts a pane id token (`%N`)
//! appearing in either the agent `name` or the agent `task`, and requires that
//! token to be a WHOLE token, not a substring — `%140` must not satisfy `%1408`.

use serde::Deserialize;
use std::collections::BTreeSet;
use std::fmt;

/// The roster source this module reads back. Not a liveness source.
pub const ROSTER_SOURCE: &str = "am robot agents";

/// The send target a registration must precede.
pub const TICK_ZERO_TARGET: &str = "tick_zero";

#[derive(Debug, Deserialize)]
struct RosterDoc {
    #[serde(default)]
    agents: Option<Vec<RosterAgent>>,
}

#[derive(Debug, Deserialize)]
struct RosterAgent {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    task: Option<String>,
}

/// One pane id observed in the roster, with the identity that carried it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct MailRegistration {
    pub pane_id: String,
    pub agent_name: String,
}

/// The parsed `am robot agents` roster. Construction refuses an empty scan set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MailRoster {
    registrations: Vec<MailRegistration>,
    agent_count: usize,
}

impl MailRoster {
    pub fn agent_count(&self) -> usize {
        self.agent_count
    }

    pub fn registrations(&self) -> &[MailRegistration] {
        &self.registrations
    }

    /// Pane ids the roster lists, deduplicated and ordered.
    pub fn registered_panes(&self) -> BTreeSet<&str> {
        self.registrations
            .iter()
            .map(|reg| reg.pane_id.as_str())
            .collect()
    }

    pub fn registration_for(&self, pane_id: &str) -> Option<&MailRegistration> {
        self.registrations
            .iter()
            .find(|reg| reg.pane_id == pane_id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailRegError {
    /// Roster bytes could not be parsed at all.
    RosterUnparsable { reason_code: String, detail: String },
    /// The roster carried no `agents` array, or carried an empty one. An empty
    /// scan set is an error, never a pass: zero agents cannot witness zero
    /// unregistered panes.
    RosterEmpty { reason_code: String },
    /// The spawn wave itself was empty. Nothing to prove, so nothing passes.
    EmptyWave { reason_code: String },
    /// A spawned pane is absent from the roster.
    PaneNotRegistered {
        reason_code: String,
        pane_id: String,
        source: &'static str,
        agent_count: usize,
    },
}

impl MailRegError {
    pub fn reason_code(&self) -> &str {
        match self {
            Self::RosterUnparsable { reason_code, .. }
            | Self::RosterEmpty { reason_code }
            | Self::EmptyWave { reason_code }
            | Self::PaneNotRegistered { reason_code, .. } => reason_code,
        }
    }
}

impl fmt::Display for MailRegError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RosterUnparsable {
                reason_code,
                detail,
            } => write!(f, "{reason_code} source={ROSTER_SOURCE} detail={detail}"),
            Self::RosterEmpty { reason_code } => {
                write!(
                    f,
                    "{reason_code} source={ROSTER_SOURCE} — an empty roster is an error, not a pass"
                )
            }
            Self::EmptyWave { reason_code } => {
                write!(f, "{reason_code} — an empty spawn wave proves nothing")
            }
            Self::PaneNotRegistered {
                reason_code,
                pane_id,
                source,
                agent_count,
            } => write!(
                f,
                "{reason_code} pane={pane_id} source={source} agents={agent_count} \
                 — pane was not registered in Agent Mail before {TICK_ZERO_TARGET}"
            ),
        }
    }
}

impl std::error::Error for MailRegError {}

/// Every `%N` token in a string, as whole tokens.
fn pane_tokens(haystack: &str) -> Vec<String> {
    let bytes: Vec<char> = haystack.chars().collect();
    let mut out = Vec::new();
    let mut idx = 0;
    while idx < bytes.len() {
        if bytes[idx] == '%' {
            let mut end = idx + 1;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }
            if end > idx + 1 {
                out.push(bytes[idx..end].iter().collect());
            }
            idx = end;
        } else {
            idx += 1;
        }
    }
    out
}

/// Parse `am robot agents --json` stdout into a roster. Refuses an empty set.
pub fn parse_roster(stdout: &str) -> Result<MailRoster, MailRegError> {
    let doc: RosterDoc =
        serde_json::from_str(stdout).map_err(|err| MailRegError::RosterUnparsable {
            reason_code: "L4_MAIL_ROSTER_UNPARSABLE".to_owned(),
            detail: err.to_string(),
        })?;
    let agents = match doc.agents {
        Some(agents) if !agents.is_empty() => agents,
        Some(_) | None => {
            return Err(MailRegError::RosterEmpty {
                reason_code: "L4_MAIL_ROSTER_EMPTY".to_owned(),
            })
        }
    };
    let agent_count = agents.len();
    let mut registrations = Vec::new();
    for agent in &agents {
        let name = agent.name.clone().unwrap_or_default();
        for field in [agent.name.as_deref(), agent.task.as_deref()] {
            let Some(field) = field else { continue };
            for pane_id in pane_tokens(field) {
                let reg = MailRegistration {
                    pane_id,
                    agent_name: name.clone(),
                };
                if !registrations.contains(&reg) {
                    registrations.push(reg);
                }
            }
        }
    }
    registrations.sort();
    Ok(MailRoster {
        registrations,
        agent_count,
    })
}

/// Require every spawned pane to be registered in Agent Mail before `tick_zero`.
///
/// Returns the registration receipt for each pane, in wave order. A missing
/// registration fails; an empty wave fails; an empty roster fails.
pub fn require_registered_before_tick_zero(
    wave_panes: &[String],
    roster: &MailRoster,
) -> Result<Vec<MailRegistration>, MailRegError> {
    if wave_panes.is_empty() {
        return Err(MailRegError::EmptyWave {
            reason_code: "L4_MAIL_EMPTY_WAVE".to_owned(),
        });
    }
    let mut receipts = Vec::with_capacity(wave_panes.len());
    for pane in wave_panes {
        // Roster pane ids are already whole `%N` tokens, so equality IS
        // whole-token matching: `%140` cannot satisfy `%1408`.
        let found = roster.registration_for(pane);
        match found {
            Some(reg) => receipts.push(reg.clone()),
            None => {
                return Err(MailRegError::PaneNotRegistered {
                    reason_code: "L4_MAIL_PANE_NOT_REGISTERED".to_owned(),
                    pane_id: pane.clone(),
                    source: ROSTER_SOURCE,
                    agent_count: roster.agent_count(),
                })
            }
        }
    }
    Ok(receipts)
}

/// Spawn-path seam: parse the roster readback, then require registration.
pub fn spawn_requires_mail_registration(
    roster_stdout: &str,
    wave_panes: &[String],
) -> Result<Vec<MailRegistration>, MailRegError> {
    let roster = parse_roster(roster_stdout)?;
    require_registered_before_tick_zero(wave_panes, &roster)
}
