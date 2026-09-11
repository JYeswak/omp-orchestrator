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
///
/// EVERY entry here MUST appear in [`usage`]. That is not a convention — it is
/// asserted by `every_verb_appears_in_usage`, because this const and `usage()`
/// were two sources of truth and had ALREADY drifted: `quickstart`, `completion`
/// and `upstream-report` all dispatched while appearing nowhere in root `--help`,
/// so three shipped verbs were invisible to an operator reading help. The test is
/// the mechanism; a hand-maintained second list is what produced the drift.
pub const VERBS: &[&str] = &[
    "init",
    "doctor",
    "help",
    "capabilities",
    "start",
    "supervise",
    "portal",
    "quickstart",
    "completion",
    "upstream-report",
    "validate",
    "audit",
    "why",
    "health",
    "repair",
    // f3maq: `undo` is declared, compiled and dispatched (`main.rs` verb table).
    // It lands here and in `usage` in the SAME commit as its match arm, because
    // `compare_verb_parity` and `every_verb_appears_in_usage` couple all three
    // sites -- a partial wiring reds the parity leg rather than half-shipping.
    "undo",
    // Added concurrently by %20 while pane 1 held this site. It DOES dispatch
    // (`main.rs:80  "state" => run_state(rest)`), so it is a legitimate entry --
    // it was simply missing its usage line, which `every_verb_appears_in_usage`
    // demanded on the test's first day. That is the parity leg working, not a
    // collision to revert.
    "state", "stats", "messages", "ps", "parity",];

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
         \x20 doctor --adapter <name>|all [--json]             EXECUTE one adapter, or the whole roster\n\
         \x20 health [--repo PATH] [--json]                    single-shot state read; spawns nothing\n\
         \x20 repair --scope <s> [--dry-run] [--apply]         idempotent fix; --dry-run is the DEFAULT\n\
         \x20 validate <thing> [--repo PATH] [--json]          pure read; verifies without executing\n\
         \x20 audit [--limit N] [--repo PATH] [--json]         recent state mutations with provenance\n\
         \x20 why <id> [--repo PATH] [--json]                  provenance trace for one object\n\
         \x20 undo <scope> [--from SHA] [--dry-run | --apply]  restore a backup; --dry-run is the DEFAULT\n\
         \x20 start [--repo PATH] [--session NAME] [--json]    run the ordered S1 walkthrough\n\
         \x20 supervise [--repo PATH] [--session NAME] [--once|--max-ticks N]  run observe -> dispatch -> receipt\n\
         \x20 portal [--repo PATH] [--session NAME] --json     emit the S1 robot portal envelope\n\
         \x20 state [--json]                                   project OMP's own state over --mode=rpc\n\
         \x20 stats [--json]                                   project OMP's session cost and token counts\n\
         \x20 messages [--json]                                project OMP's message roles and counts\n\
         \x20 ps [--repo PATH] [--json]                 project-scoped supervised daemon rows\n\
         \x20 parity --installed PATH [--json]                  compare installed capabilities verbs with this source\n\
         \x20 upstream-report <adapter> [--apply] [--json]     draft an upstream issue; --apply gated\n\
         \x20 quickstart [--json]                              orientation for a new operator or agent\n\
         \x20 completion <shell>                               emit a completion script for <shell>\n\
         \x20 help <adapter>                                   usage for one of {} workspace adapters\n\
         \x20 capabilities [--json]                            enumerate adapters, verbs, probe ids\n\
         ADDRESSING: a positional <adapter> selects a workspace TARGET; --scope selects a PROBE\n\
         FAMILY. Both axes exist and neither subsumes the other.\n\
         MUTATION: `repair` and any --apply path default to a dry run. `--dry-run` with `--apply`\n\
         is a typed refusal, not a precedence rule.\n\
         EXIT CODES: 0 success · 1 degraded · 2 usage or safety refusal · 3 instrument error ·\n\
         4 upstream unreachable. `2` and `4` are distinct: a name absent from the roster is a\n\
         usage error, a roster member absent from PATH is upstream-unreachable.",
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

    /// LEG 2 OF THE VERB CLOSING CONTRACT, in the direction that had already broken.
    ///
    /// `VERBS` and [`usage`] were two hand-maintained lists. Measured 2026-09-07:
    /// `quickstart`, `completion` and `upstream-report` were all in `VERBS`, all
    /// dispatched from `main.rs`, and NONE appeared in root `--help` — so three
    /// shipped verbs were invisible to an operator reading help, and the canonical
    /// `check-cli-scoping` root-help probe for `--dry-run` failed for the same
    /// reason. A convention did not prevent that. This test does.
    #[test]
    fn every_verb_appears_in_usage() {
        let rendered = usage();
        let missing: Vec<&str> = VERBS
            .iter()
            .copied()
            .filter(|verb| !rendered.contains(verb))
            .collect();
        assert!(
            missing.is_empty(),
            "these verbs dispatch but are absent from root usage, so an operator \
             reading --help cannot discover them: {missing:?}"
        );
    }

    /// The negative half: usage must not advertise a verb the umbrella does not
    /// dispatch. An array entry with no dispatch is a lie; a usage line with no
    /// dispatch is the same lie in prose, and it is the worse failure direction
    /// because an operator acts on help text.
    ///
    /// KEYED ON STRUCTURE, NOT ON CASE. The first version of this test guessed a
    /// verb line from "starts with a lowercase token" and FAILED on its own
    /// subject: prose continuation lines inside the same `format!` also begin
    /// with lowercase words, so it flagged sentence fragments as advertised
    /// verbs. Verb lines are the ones `\x20` indents; prose lines render flush.
    /// That is a real discriminator rather than a heuristic — the same lesson
    /// this crate's peers hit tonight with `^error` against ANSI output and with
    /// a needle drawn from a commit message instead of from source.
    #[test]
    fn usage_advertises_no_verb_outside_the_verb_set() {
        let rendered = usage();
        let advertised: Vec<String> = rendered
            .lines()
            .filter(|line| line.starts_with(' '))
            .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
            .filter(|token| !VERBS.contains(&token.as_str()))
            .collect();
        assert!(
            advertised.is_empty(),
            "root usage advertises indented tokens that are not dispatchable verbs: \
             {advertised:?}"
        );
    }

    /// The canonical `check-cli-scoping` probe greps ROOT help for `--dry-run`,
    /// because a mutating CLI whose dry-run is undiscoverable is one an operator
    /// will not use. Pinned here so the mention cannot be dropped in an edit.
    #[test]
    fn root_usage_documents_the_dry_run_default_and_the_exit_dictionary() {
        let rendered = usage();
        for needle in ["--dry-run", "--apply", "EXIT CODES", "upstream unreachable"] {
            assert!(
                rendered.contains(needle),
                "root usage must document {needle:?}: the canonical checker probes root \
                 --help for the mutation and exit-code contract"
            );
        }
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
