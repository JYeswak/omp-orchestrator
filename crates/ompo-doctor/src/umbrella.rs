//! The umbrella addressability layer for the `ompo` binary.
//!
//! Contract: `docs/contracts/umbrella_adapter_dispatch.md`. Bead:
//! `omp-orchestrator-jplf.7.2`. `UAD-NAME = ompo` and `UAD-ADDRESS = BOTH axes` were
//! ratified 2026-09-07: a positional `<adapter>` selects a workspace TARGET, `--scope
//! <family>` selects a PROBE FAMILY, and neither subsumes the other -- `s1_l1_doctor.md:90`
//! `L1-BUILD-SCOPE` already requires scope.
//!
//! THIS MODULE OWNS ADDRESSABILITY ONLY. Doctor semantics live in `run_doctor`; install
//! semantics live in `ompo-start`. Per `fh C47` one identity-bearing contract gets exactly
//! one canonical definition, and the identity here is the invocation shape
//! `ompo <verb> [<adapter>]`.

use serde_json::{json, Value};

/// The adapter roster, GENERATED from the workspace by `build.rs`.
///
/// `LAW-UAD-ROSTER-DERIVED`: adding a bin target changes this with no source edit. Never
/// hand-edit; `roster_tracks_the_runner_not_a_literal` compares it against `cargo metadata`.
include!(concat!(env!("OUT_DIR"), "/adapters.rs"));

/// Envelope schema version for every umbrella command.
pub const SCHEMA_VERSION: &str = "omp.umbrella/v1";

/// Verbs the umbrella dispatches. `start` owns the ordered walkthrough and
/// `portal` projects liveness and next action into a robot envelope.
pub const VERBS: &[&str] = &["init", "doctor", "help", "capabilities", "start", "portal", "quickstart", "completion", "upstream-report" "state",];

/// The adapter roster. Never empty: `build.rs` refuses to generate an empty one, and
/// [`roster_or_error`] is the runtime guard for the same property.
#[must_use]
pub fn adapters() -> &'static [&'static str] {
    ADAPTERS
}

/// `LAW-UAD-EMPTY-ROSTER-IS-ERROR`: a roster of zero adapters is an ERROR, never a pass.
/// An empty scan set reports identically to a complete one that found nothing.
pub fn roster_or_error() -> Result<&'static [&'static str], String> {
    if ADAPTERS.is_empty() {
        return Err("UAD_EMPTY_ROSTER reason=zero_adapters_generated".to_owned());
    }
    Ok(ADAPTERS)
}

/// True when `name` is a workspace bin target this umbrella can address.
#[must_use]
pub fn is_adapter(name: &str) -> bool {
    ADAPTERS.contains(&name)
}

/// A namespaced probe id, validated AT CONSTRUCTION.
///
/// `UAD-PROBE-ID` / `LAW-UAD-PROBE-ID-AT-CONSTRUCTION`: the pattern is
/// `^omp(\.[a-z][a-z0-9_-]*){2,}$` and a bare-segment id is rejected where it is BUILT, not
/// where it is reviewed. A review-time check passes and a construction-time check refuses;
/// only the second is visible in a green suite.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProbeId(String);

impl ProbeId {
    /// Build a probe id or refuse it with the reason and the required shape.
    pub fn new(raw: &str) -> Result<Self, String> {
        let mut segments = raw.split('.');
        if segments.next() != Some("omp") {
            return Err(format!(
                "UAD_PROBE_ID_INVALID id={raw:?} reason=missing_omp_root \
                 required=^omp(\\.[a-z][a-z0-9_-]*){{2,}}$"
            ));
        }
        let tail: Vec<&str> = segments.collect();
        if tail.len() < 2 {
            return Err(format!(
                "UAD_PROBE_ID_INVALID id={raw:?} reason=too_few_segments segments={} \
                 required=^omp(\\.[a-z][a-z0-9_-]*){{2,}}$",
                tail.len() + 1
            ));
        }
        for segment in &tail {
            let mut chars = segment.chars();
            let Some(first) = chars.next() else {
                return Err(format!(
                    "UAD_PROBE_ID_INVALID id={raw:?} reason=empty_segment \
                     required=^omp(\\.[a-z][a-z0-9_-]*){{2,}}$"
                ));
            };
            if !first.is_ascii_lowercase() {
                return Err(format!(
                    "UAD_PROBE_ID_INVALID id={raw:?} reason=segment_must_start_lowercase \
                     segment={segment:?} required=^omp(\\.[a-z][a-z0-9_-]*){{2,}}$"
                ));
            }
            if !chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-') {
                return Err(format!(
                    "UAD_PROBE_ID_INVALID id={raw:?} reason=illegal_character \
                     segment={segment:?} required=^omp(\\.[a-z][a-z0-9_-]*){{2,}}$"
                ));
            }
        }
        Ok(Self(raw.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The repo envelope, one shape for every umbrella command.
///
/// `UAD-ENVELOPE`, from `docs/plan/07-installability.md:118-121`.
#[must_use]
pub fn envelope(command: &str, status: &str, data: Value) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "command": command,
        "status": status,
        "data": data,
    })
}

/// The declared capabilities: the adapter roster and every probe id, both namespaced.
///
/// `UAD-CAPABILITIES-GOLDEN`: this is the artifact a golden test pins, so drift between the
/// declared list and the implemented one fails rather than passing quietly.
pub fn capabilities() -> Result<Value, String> {
    let roster = roster_or_error()?;
    let mut probe_ids = Vec::new();
    for probe in crate::PROBES {
        let id = ProbeId::new(&format!("omp.identity.binary.{}", probe.name.replace('-', "_")))?;
        probe_ids.push(id.as_str().to_owned());
    }
    Ok(envelope(
        "capabilities",
        "OK",
        json!({
            "adapters": roster,
            "adapter_count": roster.len(),
            "verbs": VERBS,
            "probe_ids": probe_ids,
            "probe_id_count": probe_ids.len(),
        }),
    ))
}

/// The usage line for one adapter, or a typed refusal naming the rejected string.
///
/// `LAW-UAD-UNKNOWN-IS-TWO`: an unknown adapter must exit 2 and the message must NAME the
/// rejected name. Exit 2 alone is also this binary's answer to an unknown VERB, so the code
/// cannot distinguish the two -- only the message can.
pub fn help_for(adapter: &str) -> Result<String, String> {
    let roster = roster_or_error()?;
    if !roster.contains(&adapter) {
        return Err(format!(
            "UAD_UNKNOWN_ADAPTER adapter={adapter:?} reason=absent_from_roster \
             roster_size={} hint=`ompo capabilities --json` enumerates every adapter",
            roster.len()
        ));
    }
    Ok(format!(
        "usage: ompo help {adapter}\n  \
         adapter={adapter} is a workspace bin target addressable through this umbrella.\n  \
         invoke it directly as `{adapter}` once installed, or read its own --help.\n  \
         NO-CLAIM: addressability only -- this says the name resolves, nothing about whether \
         {adapter} is installed, healthy, or correct."
    ))
}

/// The umbrella's own usage.
#[must_use]
pub fn usage() -> String {
    format!(
        "usage: ompo <verb> [args]\n\
         \x20 init [--repo PATH] [--output PATH] [--json]      write and read back the inception manifest\n\
         \x20 doctor [--repo PATH] [--scope FAMILY] [--json]   probe tools, emit lifecycle events\n\
         \x20 start [--repo PATH] [--session NAME] [--json]    run the ordered S1 walkthrough\n\
         \x20 portal [--repo PATH] [--session NAME] --json     emit the S1 robot portal envelope\n\
         \x20 help <adapter>                                   usage for one of {} workspace adapters\n\
         \x20 capabilities [--json]                            enumerate adapters, verbs, probe ids\n\
         ADDRESSING: a positional <adapter> selects a workspace TARGET; --scope selects a PROBE\n\
         FAMILY. Both axes exist and neither subsumes the other.",
        ADAPTERS.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_generated_roster_is_not_empty() {
        assert!(
            roster_or_error().is_ok(),
            "an empty roster must be an ERROR, and build.rs must never generate one"
        );
        assert!(adapters().len() >= 2, "roster = {}", adapters().len());
    }

    #[test]
    fn the_umbrella_binary_names_itself_as_an_adapter() {
        // Positive control for the generator: `ompo` is an explicit [[bin]] and
        // `ompo-start-foundation-append` is an explicit block with a differing file stem, so
        // both prove the explicit-block path is read rather than guessed from the filename.
        assert!(is_adapter("ompo"), "roster: {:?}", adapters());
        assert!(
            is_adapter("ompo-start-foundation-append"),
            "an explicit [[bin]] whose name differs from its file stem must be read from the \
             manifest, not inferred from src/bin/*.rs"
        );
    }

    #[test]
    fn an_implicitly_discovered_bin_is_in_the_roster() {
        // tick-monitor declares [[bin]] ZERO times; cargo discovers src/main.rs. A
        // [[bin]]-grep census INVERTS this, which is why build.rs replicates cargo's rules.
        assert!(
            is_adapter("tick-monitor"),
            "an implicit src/main.rs bin must be addressable; roster: {:?}",
            adapters()
        );
    }

    #[test]
    fn a_bare_probe_segment_is_a_construction_error() {
        let error = ProbeId::new("omp.identity").expect_err("two segments must be refused");
        assert!(error.contains("reason=too_few_segments"), "got {error:?}");
        assert!(error.contains("required="), "the refusal must state the shape: {error:?}");

        let rooted = ProbeId::new("identity.binary.tmux").expect_err("a missing omp root refuses");
        assert!(rooted.contains("reason=missing_omp_root"), "got {rooted:?}");

        let shouty =
            ProbeId::new("omp.Identity.binary").expect_err("an uppercase segment refuses");
        assert!(
            shouty.contains("reason=segment_must_start_lowercase"),
            "got {shouty:?}"
        );

        // KNOWN-GOOD: the shape the plan specifies must be accepted.
        let ok = ProbeId::new("omp.identity.binary.tick_monitor").expect("valid id");
        assert_eq!(ok.as_str(), "omp.identity.binary.tick_monitor");
    }

    #[test]
    fn an_unknown_adapter_is_refused_by_name() {
        let error = help_for("definitely-not-a-target").expect_err("unknown adapter refuses");
        assert!(
            error.contains("definitely-not-a-target"),
            "the refusal MUST name the rejected string; got {error:?}"
        );
        assert!(error.contains("UAD_UNKNOWN_ADAPTER"), "got {error:?}");
    }

    #[test]
    fn a_known_adapter_yields_a_usage_line() {
        let text = help_for("ompo").expect("a roster member must have a usage line");
        assert!(text.starts_with("usage: ompo help ompo"), "got {text:?}");
        assert!(
            text.contains("NO-CLAIM"),
            "addressability must not be read as health"
        );
    }

    #[test]
    fn capabilities_declares_every_adapter_and_a_valid_probe_id_set() {
        let value = capabilities().expect("capabilities must build");
        assert_eq!(value["schema_version"], SCHEMA_VERSION);
        assert_eq!(value["status"], "OK");
        assert_eq!(
            value["data"]["adapter_count"].as_u64().unwrap() as usize,
            adapters().len(),
            "the declared count must equal the roster it came from"
        );
        let ids = value["data"]["probe_ids"].as_array().expect("probe_ids");
        assert_eq!(ids.len(), crate::PROBES.len(), "one id per declared probe");
        for id in ids {
            ProbeId::new(id.as_str().expect("string id"))
                .expect("every declared probe id must satisfy the namespace at construction");
        }
    }
}
