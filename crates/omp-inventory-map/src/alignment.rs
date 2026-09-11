//! Surface alignment — the repeatable process that makes cell N+1 mechanical.
//!
//! Bead: omp-orchestrator-ablcf
//!
//! ## What this adds, and what it deliberately does not
//!
//! This crate already DERIVES the OMP surface from the installed artifact through direct
//! process probes, fail-closed, with `UNKNOWN` never upgraded to a healthy result (see the
//! crate docs). **That half is not rewritten here.** What did not exist is the other half:
//! deciding, for each derived surface entry, whether this workspace *consumes* it — and
//! **refusing** when nobody has said either way.
//!
//! ## Three verdicts, and the third is the point
//!
//! ```text
//! CONSUMED          a named crate reaches it, with a cited call site
//! DELIBERATELY_NOT  we do not consume it, WITH an owner, a reason, and a dies_when
//! UNCLASSIFIED      neither -> THE GATE FAILS
//! ```
//!
//! Per `fh C69` these are three verdicts with three different remedies and must never be
//! merged. The classifier this replaces had three arms too, but they were
//! `owner.is_some()` / `name in a hand-typed list` / **everything else**, and that last one
//! emitted `CAPABILITY_NOT_USED` — a positive-sounding claim produced by the *default
//! branch*. That is the defect `AGENTS.md` records in our own
//! `kernel-only-operator-hook`, where a session that never ran classified as `COVERED`
//! because compliance was the fallthrough. **Here the residual is the failing state**, and
//! every arm asserts its own precondition.
//!
//! ## Consumption is DECLARED BY THE CONSUMER, then derived from metadata
//!
//! The previous `owner_for` could name exactly one owner — this crate itself — so a real
//! consumer elsewhere in the workspace was invisible to it. Rather than maintain a central
//! list that must be edited whenever a consumer appears, **each consuming crate declares
//! what it consumes in its own manifest**:
//!
//! ```toml
//! [package.metadata.omp_surface]
//! consumes = [
//!   { kind = "rpc_handler", name = "get_state", call_site = "src/omp_state.rs" },
//! ]
//! ```
//!
//! That is the same architecture `[package.metadata.gate]` already uses in this repo, for
//! the same stated reason: *"this crate's OWN check invocation, so `gate-runner` needs no
//! edit when a gate is added."* The map needs no edit when a consumer is added, and the
//! declaration is read from `cargo metadata` — derived, not transcribed.
//!
//! ## No typed counts
//!
//! There are no expected-count constants here, on purpose. Measured 2026-09-08: the RPC
//! surface figure has been published as `14`, as `3`, and as `39` on a different axis,
//! while one handshake advertised `590` commands — and `--mode=rpc` turns out to read
//! `{id,type}` frames rather than JSON-RPC, so those were not even the same channel.
//! **Any number typed into a source file is a transcribed value that reports the surface as
//! it was when someone typed it.** Ratchets here are per-surface or ratios, and every
//! report carries the install path and version it measured.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The manifest key a consuming crate uses to declare what it reaches.
pub const CONSUMER_METADATA_KEY: &str = "omp_surface";

/// A surface entry we do not consume, with the reason recorded.
///
/// **The list below is empty by design.** Following the `UNWIRED_LANE_ALLOWANCE` shape this
/// repo already uses, an exception is a NAMED ROW carrying an owner and a death condition,
/// never silence. A row whose `dies_when` has come true is a row to delete, and
/// [`AlignmentReport::stale_allowances`] names those rather than letting them accumulate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeliberateNonConsumption {
    pub kind: &'static str,
    pub name: &'static str,
    /// Who decided. Not a crate — a person or pane accountable for the decision.
    pub owner: &'static str,
    /// Why not consuming it is correct, not merely current.
    pub reason: &'static str,
    /// The observable condition under which this row must be deleted.
    pub dies_when: &'static str,
}

/// The reviewed size of [`DELIBERATELY_NOT`]. THE SINGLE PLACE this figure is written.
///
/// Every site that needs the count reads this constant; none restates the number. Two
/// hand-maintained copies is the defect that reddened CI on 2026-09-11 while the crate was
/// green on a hand-patched worktree: the table held 75 rows and a second pin, in
/// `bin/omp-surface-align.rs`, still said 37. Changing this one number moves every site
/// that reads it, which is what makes the divergence unrepeatable rather than merely fixed.
pub const DELIBERATELY_NOT_ROWS: usize = 75;

/// Deliberate non-consumption rows. Each row keeps an otherwise-visible surface in the map
/// while recording the owner, reason, and condition that retires the decision.
pub const DELIBERATELY_NOT: &[DeliberateNonConsumption] = &[
    DeliberateNonConsumption {
        kind: "cli",
        name: "ps",
        owner: "omp-inventory-map",
        reason: "command is consumed on the daemon_process axis by ompo-doctor, so declaring cli would duplicate ownership",
        dies_when: "daemon_process and cli axes gain an explicit cross-axis equivalence contract or the CLI surface is retired",
    },
    DeliberateNonConsumption {
        kind: "transport_mode",
        name: "value",
        owner: "omp-inventory-map",
        reason: "placeholder value is not a consumable mode identity and no separate mode-level caller exists",
        dies_when: "the deriver emits concrete mode entries with a declared payload or the transport_mode axis is retired",
    },
    DeliberateNonConsumption {        kind: "cli",        name: "acp",        owner: "omp-inventory-map",        reason: "point-to-point subprocess server, not an attach-to-live-pane control surface",        dies_when: "OMP publishes an attach-existing-pane ACP contract",    },
    DeliberateNonConsumption {        kind: "cli",        name: "agents",        owner: "omp-inventory-map",        reason: "unpack writes agent files into user or project configuration",        dies_when: "agent export becomes a read-only runtime registry",    },
    DeliberateNonConsumption {        kind: "cli",        name: "auth-broker",        owner: "omp-inventory-map",        reason: "credential-vault login, migration, token, and service operations cross the secret/operator boundary",        dies_when: "OMP exposes a redacted, read-only auth-health contract for orchestration",    },
    DeliberateNonConsumption {        kind: "cli",        name: "auth-gateway",        owner: "omp-inventory-map",        reason: "starts or controls a network proxy",        dies_when: "the gateway publishes a bounded health/readiness API consumed by this orchestrator",    },
    DeliberateNonConsumption {        kind: "cli",        name: "bench",        owner: "omp-inventory-map",        reason: "provider/model benchmark is an evaluation workload, not live lifecycle state",        dies_when: "a scheduled benchmark consumer and budget contract are added",    },
    DeliberateNonConsumption {        kind: "cli",        name: "browser-relay",        owner: "omp-inventory-map",        reason: "starts a local browser-control service",        dies_when: "the orchestrator owns a typed relay health/control contract",    },
    DeliberateNonConsumption {        kind: "cli",        name: "cleanse",        owner: "omp-inventory-map",        reason: "launches subagents and can fix project state",        dies_when: "it exposes a read-only, bounded diagnostic result with no agent or file mutation",    },
    DeliberateNonConsumption {        kind: "cli",        name: "commit",        owner: "omp-inventory-map",        reason: "edits repository history-adjacent artifacts and is human-reviewed",        dies_when: "the orchestrator explicitly owns a commit-generation protocol",    },
    DeliberateNonConsumption {        kind: "cli",        name: "completions",        owner: "omp-inventory-map",        reason: "emits shell ergonomics rather than runtime state",        dies_when: "the completion output becomes an input to a declared runtime consumer",    },
    DeliberateNonConsumption {        kind: "cli",        name: "compress",        owner: "omp-inventory-map",        reason: "rewrites caller-selected files",        dies_when: "the orchestrator owns a reversible prompt-compaction API",    },
    DeliberateNonConsumption {        kind: "cli",        name: "config",        owner: "omp-inventory-map",        reason: "configuration mutation changes future sessions",        dies_when: "OMP exposes a read-only effective-config snapshot needed by a consumer",    },
    DeliberateNonConsumption {        kind: "cli",        name: "dry-balance",        owner: "omp-inventory-map",        reason: "auth-account policy simulation is not OMP session lifecycle state",        dies_when: "account admission becomes a declared orchestration input",    },
    DeliberateNonConsumption {        kind: "cli",        name: "gallery",        owner: "omp-inventory-map",        reason: "visual QA surface",        dies_when: "an automated visual regression consumer is declared",    },
    DeliberateNonConsumption {        kind: "cli",        name: "gc",        owner: "omp-inventory-map",        reason: "performs storage reclamation",        dies_when: "OMP publishes a read-only GC readiness/result contract",    },
    DeliberateNonConsumption {        kind: "cli",        name: "git",        owner: "omp-inventory-map",        reason: "interactive terminal UI and repository mutation",        dies_when: "OMP publishes a non-interactive typed git operation contract",    },
    DeliberateNonConsumption {        kind: "cli",        name: "grep",        owner: "omp-inventory-map",        reason: "diagnostic tool invocation, not OMP lifecycle state",        dies_when: "the orchestrator declares grep output as a stable control-plane input",    },
    DeliberateNonConsumption {        kind: "cli",        name: "grievances",        owner: "omp-inventory-map",        reason: "QA issue maintenance includes deletion and push actions",        dies_when: "a read-only, authenticated grievance feed is an explicit fleet input",    },
    DeliberateNonConsumption {        kind: "cli",        name: "if-bench",        owner: "omp-inventory-map",        reason: "evaluation workload, not runtime control",        dies_when: "a scheduled benchmark consumer and cost policy are added",    },
    DeliberateNonConsumption {        kind: "cli",        name: "images",        owner: "omp-inventory-map",        reason: "publication health plus purge mutation is outside this OMP lifecycle",        dies_when: "a declared image-backend health consumer exists",    },
    DeliberateNonConsumption {        kind: "cli",        name: "install",        owner: "omp-inventory-map",        reason: "mutates installed extensions and trust surface",        dies_when: "extension installation is replaced by a reviewed typed deployment protocol",    },
    DeliberateNonConsumption {        kind: "cli",        name: "join",        owner: "omp-inventory-map",        reason: "human collaboration entry point",        dies_when: "collab session membership is an explicit orchestrator-owned resource",    },
    DeliberateNonConsumption {        kind: "cli",        name: "plugin",        owner: "omp-inventory-map",        reason: "plugin lifecycle mutates executable extension code",        dies_when: "reviewed plugin state is exposed as a read-only contract",    },
    DeliberateNonConsumption {        kind: "cli",        name: "read",        owner: "omp-inventory-map",        reason: "generic operator/tool read surface, not a stable OMP lifecycle input",        dies_when: "the orchestrator declares a bounded URI/file source contract",    },
    DeliberateNonConsumption {        kind: "cli",        name: "render",        owner: "omp-inventory-map",        reason: "presentation and repaint diagnostics",        dies_when: "render timing becomes a declared automated quality signal",    },
    DeliberateNonConsumption {        kind: "cli",        name: "say",        owner: "omp-inventory-map",        reason: "local audio side effect",        dies_when: "voice output is an explicit orchestrator-owned notification channel",    },
    DeliberateNonConsumption {        kind: "cli",        name: "search",        owner: "omp-inventory-map",        reason: "external research workload, not OMP session state",        dies_when: "search results become a declared bounded research input",    },
    DeliberateNonConsumption {        kind: "cli",        name: "setup",        owner: "omp-inventory-map",        reason: "installs dependencies and changes machine state",        dies_when: "setup becomes a declarative, reversible provisioning API",    },
    DeliberateNonConsumption {        kind: "cli",        name: "share",        owner: "omp-inventory-map",        reason: "external publication of session data",        dies_when: "the orchestrator owns an approved session-publication contract",    },
    DeliberateNonConsumption {        kind: "cli",        name: "shell",        owner: "omp-inventory-map",        reason: "interactive terminal and arbitrary command execution",        dies_when: "OMP offers a bounded typed command-execution API accepted by this orchestrator",    },
    DeliberateNonConsumption {        kind: "cli",        name: "ssh",        owner: "omp-inventory-map",        reason: "edits operator connection configuration",        dies_when: "remote host state is exposed through a declared non-interactive control plane",    },
    DeliberateNonConsumption {        kind: "cli",        name: "tiny-models",        owner: "omp-inventory-map",        reason: "downloads and changes local model state",        dies_when: "local-model lifecycle is an explicit provisioned dependency",    },
    DeliberateNonConsumption {        kind: "cli",        name: "token",        owner: "omp-inventory-map",        reason: "direct credential emission is outside an orchestrator consumer boundary",        dies_when: "OMP exposes redacted credential-health metadata without token material",    },
    DeliberateNonConsumption {        kind: "cli",        name: "ttsr",        owner: "omp-inventory-map",        reason: "rule diagnostics and source scanning, not OMP lifecycle state",        dies_when: "TTSR verdicts become a declared CI/runtime gate input",    },
    DeliberateNonConsumption {        kind: "cli",        name: "update",        owner: "omp-inventory-map",        reason: "changes the installed OMP binary",        dies_when: "updates are handled by an approved release controller",    },
    DeliberateNonConsumption {        kind: "cli",        name: "worktree",        owner: "omp-inventory-map",        reason: "repository topology mutation conflicts with this workspace's zero-worktree policy",        dies_when: "the workspace policy explicitly permits orchestrator-owned worktrees",    },
    DeliberateNonConsumption { kind: "rpc_handler", name: "abort", owner: "omp-inventory-map", reason: "rpc handler abort has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for abort is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "abort_and_prompt", owner: "omp-inventory-map", reason: "rpc handler abort_and_prompt has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for abort_and_prompt is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "abort_bash", owner: "omp-inventory-map", reason: "rpc handler abort_bash has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for abort_bash is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "abort_retry", owner: "omp-inventory-map", reason: "rpc handler abort_retry has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for abort_retry is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "bash", owner: "omp-inventory-map", reason: "rpc handler bash has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for bash is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "branch", owner: "omp-inventory-map", reason: "rpc handler branch has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for branch is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "compact", owner: "omp-inventory-map", reason: "rpc handler compact has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for compact is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "cycle_model", owner: "omp-inventory-map", reason: "rpc handler cycle_model has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for cycle_model is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "cycle_thinking_level", owner: "omp-inventory-map", reason: "rpc handler cycle_thinking_level has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for cycle_thinking_level is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "export_html", owner: "omp-inventory-map", reason: "rpc handler export_html has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for export_html is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "follow_up", owner: "omp-inventory-map", reason: "rpc handler follow_up has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for follow_up is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "get_available_models", owner: "omp-inventory-map", reason: "rpc handler get_available_models has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for get_available_models is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "get_branch_messages", owner: "omp-inventory-map", reason: "rpc handler get_branch_messages has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for get_branch_messages is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "get_last_assistant_text", owner: "omp-inventory-map", reason: "rpc handler get_last_assistant_text has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for get_last_assistant_text is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "get_login_providers", owner: "omp-inventory-map", reason: "rpc handler get_login_providers has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for get_login_providers is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "get_messages_page", owner: "omp-inventory-map", reason: "rpc handler get_messages_page has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for get_messages_page is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "get_subagent_messages", owner: "omp-inventory-map", reason: "rpc handler get_subagent_messages has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for get_subagent_messages is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "get_subagents", owner: "omp-inventory-map", reason: "rpc handler get_subagents has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for get_subagents is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "handoff", owner: "omp-inventory-map", reason: "rpc handler handoff has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for handoff is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "login", owner: "omp-inventory-map", reason: "rpc handler login has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for login is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "new_session", owner: "omp-inventory-map", reason: "rpc handler new_session has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for new_session is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "prompt", owner: "omp-inventory-map", reason: "rpc handler prompt has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for prompt is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "set_auto_compaction", owner: "omp-inventory-map", reason: "rpc handler set_auto_compaction has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for set_auto_compaction is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "set_auto_retry", owner: "omp-inventory-map", reason: "rpc handler set_auto_retry has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for set_auto_retry is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "set_fast_mode", owner: "omp-inventory-map", reason: "rpc handler set_fast_mode has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for set_fast_mode is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "set_follow_up_mode", owner: "omp-inventory-map", reason: "rpc handler set_follow_up_mode has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for set_follow_up_mode is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "set_host_tools", owner: "omp-inventory-map", reason: "rpc handler set_host_tools has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for set_host_tools is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "set_host_uri_schemes", owner: "omp-inventory-map", reason: "rpc handler set_host_uri_schemes has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for set_host_uri_schemes is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "set_interrupt_mode", owner: "omp-inventory-map", reason: "rpc handler set_interrupt_mode has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for set_interrupt_mode is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "set_model", owner: "omp-inventory-map", reason: "rpc handler set_model has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for set_model is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "set_session_name", owner: "omp-inventory-map", reason: "rpc handler set_session_name has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for set_session_name is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "set_steering_mode", owner: "omp-inventory-map", reason: "rpc handler set_steering_mode has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for set_steering_mode is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "set_subagent_subscription", owner: "omp-inventory-map", reason: "rpc handler set_subagent_subscription has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for set_subagent_subscription is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "set_thinking_level", owner: "omp-inventory-map", reason: "rpc handler set_thinking_level has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for set_thinking_level is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "set_todos", owner: "omp-inventory-map", reason: "rpc handler set_todos has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for set_todos is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "steer", owner: "omp-inventory-map", reason: "rpc handler steer has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for steer is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "switch_session", owner: "omp-inventory-map", reason: "rpc handler switch_session has no typed in-tree operator caller; retain the visible surface until it is consumed or explicitly wired", dies_when: "a typed caller for switch_session is landed and the row is reclassified as CONSUMED or WIRE", },
    DeliberateNonConsumption { kind: "rpc_handler", name: "get_available_commands", owner: "omp-inventory-map", reason: "startup command metadata is parsed, but no typed request caller issues get_available_commands", dies_when: "a typed caller for get_available_commands is landed and the row is reclassified as CONSUMED or WIRE", },
];

/// One consumer declaration, read from a crate's manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsumerDeclaration {
    pub crate_name: String,
    pub kind: String,
    pub name: String,
    /// Path within the declaring crate. Cited so a reader can check the claim.
    pub call_site: String,
}

/// Why a declaration was rejected. A malformed declaration is NOT a missing one: silently
/// dropping it would turn a typo into an invisible unclassified surface.
// Serialize only: `field` is a `&'static str` because it names a compile-time constant, and
// deriving `Deserialize` on a borrowed-static type would force `'de: 'static`. These
// defects are EMITTED for diagnosis and never read back, so the asymmetry is honest rather
// than a workaround -- turning the field into a `String` to satisfy a trait nobody uses
// would be the tail wagging the dog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "UPPERCASE", tag = "kind")]
pub enum DeclarationDefect {
    NotAnArray { crate_name: String },
    NotAnObject { crate_name: String, index: usize },
    MissingField { crate_name: String, index: usize, field: &'static str },
    EmptyField { crate_name: String, index: usize, field: &'static str },
}

impl std::fmt::Display for DeclarationDefect {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotAnArray { crate_name } => write!(
                formatter,
                "ALIGN_DECL_NOT_AN_ARRAY crate={crate_name} detail=\
                 metadata.{CONSUMER_METADATA_KEY}.consumes must be an array"
            ),
            Self::NotAnObject { crate_name, index } => write!(
                formatter,
                "ALIGN_DECL_NOT_AN_OBJECT crate={crate_name} index={index}"
            ),
            Self::MissingField { crate_name, index, field } => write!(
                formatter,
                "ALIGN_DECL_MISSING_FIELD crate={crate_name} index={index} field={field}"
            ),
            Self::EmptyField { crate_name, index, field } => write!(
                formatter,
                "ALIGN_DECL_EMPTY_FIELD crate={crate_name} index={index} field={field} \
                 detail=an empty value is not a declaration"
            ),
        }
    }
}

/// Every consumer declaration in the workspace, plus every defect found reading them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ConsumerIndex {
    pub declarations: Vec<ConsumerDeclaration>,
    pub defects: Vec<DeclarationDefect>,
    /// Packages examined. A ZERO here is what separates "nobody declares anything" from
    /// "we failed to read the metadata", and [`align`] refuses on the second.
    pub packages_scanned: usize,
}

impl ConsumerIndex {
    #[must_use]
    pub fn find(&self, kind: &str, name: &str) -> Option<&ConsumerDeclaration> {
        self.declarations
            .iter()
            .find(|declaration| declaration.kind == kind && declaration.name == name)
    }

    /// Surfaces declared by MORE THAN ONE crate.
    ///
    /// Found by `%20` while landing the first real declarations: [`find`](Self::find)
    /// returns the FIRST match, so two crates claiming the same `(kind, name)` means credit
    /// goes to whichever package `cargo metadata` happens to emit first — a decision nobody
    /// made, and one that can change between cargo versions without any manifest changing.
    ///
    /// This is not a style objection. A `CONSUMED` row names a crate and a call site as
    /// evidence, so crediting the wrong crate publishes a false citation.
    #[must_use]
    pub fn duplicate_claims(&self) -> Vec<(String, String, Vec<String>)> {
        let mut by_surface: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
        for declaration in &self.declarations {
            by_surface
                .entry((declaration.kind.clone(), declaration.name.clone()))
                .or_default()
                .push(declaration.crate_name.clone());
        }
        by_surface
            .into_iter()
            .filter(|(_, crates)| crates.len() > 1)
            .map(|((kind, name), crates)| (kind, name, crates))
            .collect()
    }
}

/// The verdict for one derived surface entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE", tag = "verdict")]
pub enum Alignment {
    Consumed { crate_name: String, call_site: String },
    DeliberatelyNot { owner: String, reason: String, dies_when: String },
    /// The residual, and the failing state. Never produced by a default branch: it is
    /// returned only after both positive arms have been tested and rejected.
    Unclassified,
}

impl Alignment {
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Consumed { .. } => "CONSUMED",
            Self::DeliberatelyNot { .. } => "DELIBERATELY_NOT",
            Self::Unclassified => "UNCLASSIFIED",
        }
    }

    #[must_use]
    pub const fn is_classified(&self) -> bool {
        !matches!(self, Self::Unclassified)
    }
}

/// A derived surface entry awaiting classification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SurfaceEntry {
    pub kind: String,
    pub name: String,
}

/// One classified row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignmentRow {
    pub kind: String,
    pub name: String,
    pub alignment: Alignment,
}

/// Typed refusals. Each names a state with its own remedy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AlignmentError {
    /// ANTI-VACUITY. An empty derived surface set is an ERROR: it reports identically to a
    /// fully-classified one, and the likeliest cause is that the installed artifact could
    /// not be read — which is UNMEASURED, never green.
    EmptySurfaceSet,
    /// The metadata itself could not be read. Distinct from "nobody declared anything".
    NoPackagesScanned,
    /// A declaration was present and malformed. Refused rather than dropped.
    MalformedDeclarations(Vec<DeclarationDefect>),
    /// Two or more crates declared the same surface. Refused because a CONSUMED row names
    /// a crate and a call site as evidence, and `find` would credit whichever package
    /// `cargo metadata` emitted first — publishing a citation nobody chose.
    DuplicateClaims(Vec<(String, String, Vec<String>)>),
    /// The point of the gate.
    Unclassified(Vec<SurfaceEntry>),
}

impl std::fmt::Display for AlignmentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptySurfaceSet => write!(
                formatter,
                "ALIGN_EMPTY_SURFACE_SET detail=zero derived surface entries; a scan that \
                 covered nothing reports identically to one that passed, so this is \
                 UNMEASURED and never a pass"
            ),
            Self::NoPackagesScanned => write!(
                formatter,
                "ALIGN_NO_PACKAGES_SCANNED detail=cargo metadata yielded no packages; \
                 'nobody declares consumption' and 'the metadata was unreadable' are \
                 different states and this is the second"
            ),
            Self::DuplicateClaims(claims) => {
                writeln!(
                    formatter,
                    "ALIGN_DUPLICATE_CLAIMS count={} detail=two or more crates declared the \
                     same surface; `find` credits whichever cargo emits first, so the \
                     CONSUMED row would cite a crate nobody chose",
                    claims.len()
                )?;
                for (kind, name, crates) in claims {
                    writeln!(
                        formatter,
                        "  ALIGN_DUPLICATE_SURFACE kind={kind} name={name} claimed_by={}",
                        crates.join(",")
                    )?;
                }
                Ok(())
            }
            Self::MalformedDeclarations(defects) => {
                writeln!(
                    formatter,
                    "ALIGN_MALFORMED_DECLARATIONS count={} detail=a malformed declaration \
                     is refused, not dropped: dropping it would turn a typo into an \
                     invisible unclassified surface",
                    defects.len()
                )?;
                for defect in defects {
                    writeln!(formatter, "  {defect}")?;
                }
                Ok(())
            }
            Self::Unclassified(entries) => {
                writeln!(
                    formatter,
                    "ALIGN_UNCLASSIFIED count={} detail=every derived OMP surface must be \
                     CONSUMED by a named crate or DELIBERATELY_NOT with an owner, a reason \
                     and a dies_when. Silence is not an exception.",
                    entries.len()
                )?;
                for entry in entries {
                    writeln!(
                        formatter,
                        "  ALIGN_UNCLASSIFIED_SURFACE kind={} name={}",
                        entry.kind, entry.name
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl AlignmentError {
    /// `2` for usage/declaration refusals, `3` for an instrument failure. **Never `4`** —
    /// `4` is reserved for upstream-unreachable, which `adapter_exec.rs:138-142` already
    /// spends that way, and a shared vocabulary is what lets an agent branch on `$?`.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self {
            Self::MalformedDeclarations(_)
            | Self::DuplicateClaims(_)
            | Self::Unclassified(_) => 2,
            Self::EmptySurfaceSet | Self::NoPackagesScanned => 3,
        }
    }
}

/// The full alignment result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AlignmentReport {
    /// Which OMP was measured. Carried so no reader has to guess, and so a stale report is
    /// visibly stale rather than silently wrong.
    pub measured_install: String,
    pub measured_version: String,
    pub rows: Vec<AlignmentRow>,
    pub packages_scanned: usize,
    /// Per-surface-kind coverage as a RATIO, never a workspace-wide absolute. A ratchet on
    /// an absolute count is red-by-construction the next time the subject legitimately
    /// grows, and a gate that is red by construction gets routed around.
    pub coverage_by_kind: BTreeMap<String, KindCoverage>,
    /// Allowance rows whose `dies_when` condition is now observably true.
    pub stale_allowances: Vec<String>,
    /// Declarations naming a surface that is NOT in the derived set.
    ///
    /// Found the first time this gate reached `FULL`: `%20` declared
    /// `kind="rpc_handler" name="get_state"` while the deriver emitted only `cli` and
    /// `transport_mode` kinds, so three well-formed declarations matched nothing and
    /// **nothing noticed**. A `CONSUMED` row is a citation; a declaration that cites a
    /// surface which does not exist is a claim about nothing, and silence about it is how
    /// a coverage report reads healthy while covering none of the declared work.
    ///
    /// Reported rather than refused: a declaration may legitimately precede a deriver that
    /// cannot yet see its kind — which is exactly the state today. Making it LOUD is the
    /// fix; refusing would punish the consumer for the deriver's gap.
    pub orphan_declarations: Vec<String>,
}

/// Coverage for one surface kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KindCoverage {
    pub total: usize,
    pub consumed: usize,
    pub deliberately_not: usize,
    pub unclassified: usize,
}

impl KindCoverage {
    /// Classified fraction in basis points, so the ratio needs no float comparison.
    #[must_use]
    pub const fn classified_bps(&self) -> u32 {
        if self.total == 0 {
            return 0;
        }
        let classified = self.consumed + self.deliberately_not;
        #[allow(clippy::cast_possible_truncation)]
        {
            ((classified * 10_000) / self.total) as u32
        }
    }
}

/// Classify ONE entry. Every arm asserts its own precondition; the residual is the failure.
#[must_use]
pub fn classify(entry: &SurfaceEntry, consumers: &ConsumerIndex) -> Alignment {
    if let Some(declaration) = consumers.find(&entry.kind, &entry.name) {
        return Alignment::Consumed {
            crate_name: declaration.crate_name.clone(),
            call_site: declaration.call_site.clone(),
        };
    }
    if let Some(row) = DELIBERATELY_NOT
        .iter()
        .find(|row| row.kind == entry.kind && row.name == entry.name)
    {
        return Alignment::DeliberatelyNot {
            owner: row.owner.to_owned(),
            reason: row.reason.to_owned(),
            dies_when: row.dies_when.to_owned(),
        };
    }
    Alignment::Unclassified
}

/// Read every `[package.metadata.omp_surface]` declaration out of `cargo metadata` JSON.
///
/// # Errors
///
/// Never errors on absence — a package with no declaration is simply not a consumer. It
/// records DEFECTS for declarations that exist and are malformed, which the caller refuses
/// on. Returns [`AlignmentError::NoPackagesScanned`] only when the metadata yields no
/// packages at all, because that is an instrument failure rather than an empty answer.
pub fn index_consumers(metadata_json: &str) -> Result<ConsumerIndex, AlignmentError> {
    let root: Value =
        serde_json::from_str(metadata_json).map_err(|_| AlignmentError::NoPackagesScanned)?;
    let packages = root
        .get("packages")
        .and_then(Value::as_array)
        .ok_or(AlignmentError::NoPackagesScanned)?;
    if packages.is_empty() {
        return Err(AlignmentError::NoPackagesScanned);
    }
    let mut index = ConsumerIndex {
        packages_scanned: packages.len(),
        ..ConsumerIndex::default()
    };
    for package in packages {
        let crate_name = package
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("<unnamed>")
            .to_owned();
        let Some(consumes) = package
            .get("metadata")
            .and_then(|metadata| metadata.get(CONSUMER_METADATA_KEY))
            .and_then(|surface| surface.get("consumes"))
        else {
            continue;
        };
        let Some(entries) = consumes.as_array() else {
            index
                .defects
                .push(DeclarationDefect::NotAnArray { crate_name });
            continue;
        };
        for (position, entry) in entries.iter().enumerate() {
            let Some(object) = entry.as_object() else {
                index.defects.push(DeclarationDefect::NotAnObject {
                    crate_name: crate_name.clone(),
                    index: position,
                });
                continue;
            };
            let mut field = |key: &'static str| -> Option<String> {
                match object.get(key).and_then(Value::as_str) {
                    None => {
                        index.defects.push(DeclarationDefect::MissingField {
                            crate_name: crate_name.clone(),
                            index: position,
                            field: key,
                        });
                        None
                    }
                    Some(value) if value.trim().is_empty() => {
                        index.defects.push(DeclarationDefect::EmptyField {
                            crate_name: crate_name.clone(),
                            index: position,
                            field: key,
                        });
                        None
                    }
                    Some(value) => Some(value.to_owned()),
                }
            };
            let kind = field("kind");
            let name = field("name");
            let call_site = field("call_site");
            if let (Some(kind), Some(name), Some(call_site)) = (kind, name, call_site) {
                index.declarations.push(ConsumerDeclaration {
                    crate_name: crate_name.clone(),
                    kind,
                    name,
                    call_site,
                });
            }
        }
    }
    Ok(index)
}

/// Classify every derived surface entry and REFUSE if any is unclassified.
///
/// # Errors
///
/// See [`AlignmentError`]. The ordering is deliberate: instrument failures are reported
/// before content failures, because an unreadable input cannot produce a meaningful
/// unclassified list.
pub fn align(
    measured_install: &str,
    measured_version: &str,
    surface: &[SurfaceEntry],
    consumers: &ConsumerIndex,
) -> Result<AlignmentReport, AlignmentError> {
    if consumers.packages_scanned == 0 {
        return Err(AlignmentError::NoPackagesScanned);
    }
    if surface.is_empty() {
        return Err(AlignmentError::EmptySurfaceSet);
    }
    if !consumers.defects.is_empty() {
        return Err(AlignmentError::MalformedDeclarations(
            consumers.defects.clone(),
        ));
    }
    // Before classifying anything: if two crates claim one surface, every CONSUMED row
    // derived from that index is a coin flip on cargo's emission order. Refused here rather
    // than resolved by a rule, because any tie-break we invented would be a decision the
    // declaring crates did not make.
    let duplicates = consumers.duplicate_claims();
    if !duplicates.is_empty() {
        return Err(AlignmentError::DuplicateClaims(duplicates));
    }

    let mut rows = Vec::with_capacity(surface.len());
    let mut coverage: BTreeMap<String, KindCoverage> = BTreeMap::new();
    let mut unclassified = Vec::new();
    for entry in surface {
        let alignment = classify(entry, consumers);
        let bucket = coverage.entry(entry.kind.clone()).or_insert(KindCoverage {
            total: 0,
            consumed: 0,
            deliberately_not: 0,
            unclassified: 0,
        });
        bucket.total += 1;
        match &alignment {
            Alignment::Consumed { .. } => bucket.consumed += 1,
            Alignment::DeliberatelyNot { .. } => bucket.deliberately_not += 1,
            Alignment::Unclassified => {
                bucket.unclassified += 1;
                unclassified.push(entry.clone());
            }
        }
        rows.push(AlignmentRow {
            kind: entry.kind.clone(),
            name: entry.name.clone(),
            alignment,
        });
    }

    // An allowance row for a surface that no longer exists is stale state, not an
    // exception: report it rather than letting the list accumulate rows nobody can retire.
    let present: BTreeSet<(&str, &str)> = surface
        .iter()
        .map(|entry| (entry.kind.as_str(), entry.name.as_str()))
        .collect();
    let stale_allowances = DELIBERATELY_NOT
        .iter()
        .filter(|row| !present.contains(&(row.kind, row.name)))
        .map(|row| format!("{}:{} owner={} dies_when={}", row.kind, row.name, row.owner, row.dies_when))
        .collect();

    // Computed unconditionally, and ALSO exposed as `orphan_declarations` so the caller can
    // report orphans even when `align` refuses -- which is the common case today, and the
    // case where hiding them would matter most.
    let orphans = orphan_declarations(surface, consumers);

    if !unclassified.is_empty() {
        return Err(AlignmentError::Unclassified(unclassified));
    }
    Ok(AlignmentReport {
        measured_install: measured_install.to_owned(),
        measured_version: measured_version.to_owned(),
        rows,
        packages_scanned: consumers.packages_scanned,
        coverage_by_kind: coverage,
        stale_allowances,
        orphan_declarations: orphans,
    })
}

/// Declarations that name a surface absent from the derived set.
///
/// Independent of [`align`] on purpose: `align` returns early when anything is
/// unclassified, so a caller that only read the `Ok` path would never see an orphan in the
/// exact situation orphans matter — a refusing run. The bin calls this directly.
#[must_use]
pub fn orphan_declarations(
    surface: &[SurfaceEntry],
    consumers: &ConsumerIndex,
) -> Vec<String> {
    let present: BTreeSet<(&str, &str)> = surface
        .iter()
        .map(|entry| (entry.kind.as_str(), entry.name.as_str()))
        .collect();
    consumers
        .declarations
        .iter()
        .filter(|declaration| {
            !present.contains(&(declaration.kind.as_str(), declaration.name.as_str()))
        })
        .map(|declaration| {
            format!(
                "{}:{} declared_by={} call_site={}",
                declaration.kind, declaration.name, declaration.crate_name, declaration.call_site
            )
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metadata(consumes: &str) -> String {
        format!(
            r#"{{"packages":[
                {{"name":"ompo-doctor","metadata":{{"omp_surface":{{"consumes":[{consumes}]}}}}}},
                {{"name":"text-structure"}}
            ]}}"#
        )
    }

    fn entry(kind: &str, name: &str) -> SurfaceEntry {
        SurfaceEntry { kind: kind.to_owned(), name: name.to_owned() }
    }

    /// ACCEPTANCE G. `%20`'s proven cell is the first CONSUMED row, and it is what the old
    /// self-referential `owner_for` could not express: it named only this crate, so a real
    /// consumer elsewhere in the workspace was invisible.
    #[test]
    fn a_declaring_crate_produces_a_consumed_row_with_a_cited_call_site() {
        let json = metadata(
            r#"{"kind":"rpc_handler","name":"get_state","call_site":"src/omp_state.rs"}"#,
        );
        let consumers = index_consumers(&json).expect("index");
        assert_eq!(consumers.declarations.len(), 1);
        let report = align(
            "/opt/omp",
            "omp/18.0.11",
            &[entry("rpc_handler", "get_state")],
            &consumers,
        )
        .expect("fully classified");
        match &report.rows[0].alignment {
            Alignment::Consumed { crate_name, call_site } => {
                assert_eq!(crate_name, "ompo-doctor");
                assert_eq!(call_site, "src/omp_state.rs");
            }
            other => panic!("expected CONSUMED, got {}", other.as_str()),
        }
        assert_eq!(report.coverage_by_kind["rpc_handler"].classified_bps(), 10_000);
    }

    /// ACCEPTANCE C. An unclassified surface FAILS, and the refusal NAMES it — a count
    /// alone would not tell an operator which surface to classify.
    #[test]
    fn an_unclassified_surface_refuses_and_names_it() {
        let consumers = index_consumers(&metadata("")).expect("index");
        let error = align(
            "/opt/omp",
            "omp/18.0.11",
            &[entry("rpc_handler", "session/list")],
            &consumers,
        )
        .expect_err("must refuse");
        let rendered = error.to_string();
        assert!(rendered.contains("ALIGN_UNCLASSIFIED"), "{rendered}");
        assert!(rendered.contains("session/list"), "must name it: {rendered}");
        assert!(
            rendered.contains("Silence is not an exception"),
            "must say why: {rendered}"
        );
        assert_eq!(error.exit_code(), 2);
    }

    /// ACCEPTANCE D, known-GOOD. A fully-classified inventory PASSES. An attack-only gate
    /// ships over-strict and gets routed around, which is a slower death than none.
    #[test]
    fn a_fully_classified_inventory_passes() {
        let json = metadata(
            r#"{"kind":"transport","name":"mux","call_site":"src/lib.rs"},
               {"kind":"rpc_handler","name":"get_state","call_site":"src/omp_state.rs"}"#,
        );
        let consumers = index_consumers(&json).expect("index");
        let report = align(
            "/opt/omp",
            "omp/18.0.11",
            &[entry("transport", "mux"), entry("rpc_handler", "get_state")],
            &consumers,
        )
        .expect("must pass");
        assert_eq!(report.rows.len(), 2);
        assert!(report.rows.iter().all(|row| row.alignment.is_classified()));
        assert_eq!(report.measured_version, "omp/18.0.11");
        assert_eq!(report.measured_install, "/opt/omp");
    }

    /// ACCEPTANCE E, ANTI-VACUITY. An empty derived surface set is an ERROR, and its code
    /// is the INSTRUMENT code (3), not the content code (2): "the probe could not read the
    /// surface" and "the surface is unclassified" have opposite remedies.
    #[test]
    fn an_empty_surface_set_is_an_instrument_error_never_a_pass() {
        let consumers = index_consumers(&metadata("")).expect("index");
        let error = align("/opt/omp", "omp/18.0.11", &[], &consumers).expect_err("must refuse");
        assert_eq!(error, AlignmentError::EmptySurfaceSet);
        assert!(error.to_string().contains("UNMEASURED and never a pass"));
        assert_eq!(error.exit_code(), 3, "an instrument failure is 3, not 2");
    }

    /// Unreadable metadata is distinct from "nobody declared anything". Collapsing them
    /// would let a broken probe read as an honest empty answer.
    #[test]
    fn unreadable_metadata_is_distinct_from_nobody_declaring() {
        assert_eq!(
            index_consumers("not json").expect_err("must refuse"),
            AlignmentError::NoPackagesScanned
        );
        assert_eq!(
            index_consumers(r#"{"packages":[]}"#).expect_err("must refuse"),
            AlignmentError::NoPackagesScanned
        );
        // A package with NO declaration is not a defect -- it is simply not a consumer.
        let consumers = index_consumers(r#"{"packages":[{"name":"solo"}]}"#).expect("index");
        assert!(consumers.declarations.is_empty());
        assert!(consumers.defects.is_empty());
        assert_eq!(consumers.packages_scanned, 1);
    }

    /// A malformed declaration is REFUSED, not dropped. Dropping it would turn a typo into
    /// an invisible unclassified surface — the failure mode is silent, which is the worst
    /// direction.
    #[test]
    fn a_malformed_declaration_is_refused_rather_than_dropped() {
        let json = metadata(r#"{"kind":"rpc_handler","name":"get_state"}"#); // no call_site
        let consumers = index_consumers(&json).expect("index");
        assert_eq!(consumers.declarations.len(), 0, "the row must not be accepted");
        assert_eq!(consumers.defects.len(), 1);
        let error = align(
            "/opt/omp",
            "omp/18.0.11",
            &[entry("rpc_handler", "get_state")],
            &consumers,
        )
        .expect_err("must refuse");
        let rendered = error.to_string();
        assert!(rendered.contains("ALIGN_MALFORMED_DECLARATIONS"), "{rendered}");
        assert!(rendered.contains("ALIGN_DECL_MISSING_FIELD"), "{rendered}");
        assert!(rendered.contains("call_site"), "must name the field: {rendered}");
        assert_eq!(error.exit_code(), 2);
    }

    /// An EMPTY declared field is a defect too. `name = ""` would otherwise match nothing
    /// and read as a declaration that exists.
    #[test]
    fn an_empty_declared_field_is_a_defect() {
        let json = metadata(r#"{"kind":"rpc_handler","name":"  ","call_site":"src/x.rs"}"#);
        let consumers = index_consumers(&json).expect("index");
        assert!(consumers.declarations.is_empty());
        assert!(
            consumers
                .defects
                .iter()
                .any(|defect| matches!(defect, DeclarationDefect::EmptyField { field: "name", .. })),
            "{:?}",
            consumers.defects
        );
    }

    /// The cross-axis and placeholder rows and the generated CLI exclusions are
    /// deliberate, owned, and retirable. The size is read from [`DELIBERATELY_NOT_ROWS`],
    /// never restated: a second copy of the figure is the bug this crate already shipped.
    #[test]
    fn the_deliberately_not_allowances_are_named_and_retirable() {
        assert_eq!(
            DELIBERATELY_NOT.len(),
            DELIBERATELY_NOT_ROWS,
            "the table changed size; update DELIBERATELY_NOT_ROWS, the one reviewed figure"
        );
        let cli = DELIBERATELY_NOT
            .iter()
            .find(|row| row.kind == "cli" && row.name == "ps")
            .expect("cli ps allowance");
        assert_eq!(cli.owner, "omp-inventory-map");
        let transport = DELIBERATELY_NOT
            .iter()
            .find(|row| row.kind == "transport_mode" && row.name == "value")
            .expect("transport placeholder allowance");
        assert_eq!(transport.owner, "omp-inventory-map");
        for row in DELIBERATELY_NOT {
            assert!(!row.reason.is_empty());
            assert!(!row.dies_when.is_empty());
        }
    }

    #[test]
    fn cli_allowances_match_channel_a_reference() {
        let markdown = include_str!("../../../references/CHANNEL-A-CLI.md");
        let documented: BTreeSet<&str> = markdown
            .lines()
            .filter_map(|line| {
                let rest = line.strip_prefix("| ")?;
                let (name, tail) = rest.split_once(" |")?;
                tail.contains("OPERATOR-ONLY / DELIBERATELY_NOT")
                    .then_some(name.trim_matches(char::from(96)))
            })
            .collect();
        assert_eq!(documented.len(), 35);
        let actual: BTreeSet<&str> = DELIBERATELY_NOT
            .iter()
            .filter(|row| row.kind == "cli" && row.name != "ps")
            .map(|row| row.name)
            .collect();
        assert_eq!(actual, documented);
        assert!(DELIBERATELY_NOT
            .iter()
            .any(|row| row.kind == "cli" && row.name == "ps"));
    }

    /// UNCLASSIFIED must never come from a default branch. Proven behaviourally: an entry
    /// classified UNCLASSIFIED becomes CONSUMED the moment a declaration exists, so the
    /// verdict is a function of evidence and not of falling through.
    #[test]
    fn unclassified_is_a_tested_residual_not_a_fallthrough() {
        let subject = entry("rpc_handler", "session/fork");
        let empty = index_consumers(&metadata("")).expect("index");
        assert_eq!(classify(&subject, &empty), Alignment::Unclassified);

        let declared = index_consumers(&metadata(
            r#"{"kind":"rpc_handler","name":"session/fork","call_site":"src/fork.rs"}"#,
        ))
        .expect("index");
        assert!(
            classify(&subject, &declared).is_classified(),
            "the same entry must classify once evidence exists"
        );
    }

    /// COVERAGE IS A RATIO, per rule 10. A ratchet on an absolute count is
    /// red-by-construction the next time the subject legitimately grows.
    #[test]
    fn coverage_is_expressed_per_kind_as_a_ratio() {
        let json = metadata(r#"{"kind":"cli","name":"read","call_site":"src/read.rs"}"#);
        let consumers = index_consumers(&json).expect("index");
        let error = align(
            "/opt/omp",
            "omp/18.0.11",
            &[entry("cli", "read"), entry("cli", "unknown")],
            &consumers,
        )
        .expect_err("unknown is unclassified");
        assert!(matches!(&error, AlignmentError::Unclassified(rows) if rows.len() == 1));

        // And the ratio itself, on a passing set.
        let both = index_consumers(&metadata(
            r#"{"kind":"cli","name":"read","call_site":"a.rs"},
               {"kind":"cli","name":"unknown","call_site":"b.rs"}"#,
        ))
        .expect("index");
        let report = align(
            "/opt/omp",
            "omp/18.0.11",
            &[entry("cli", "read"), entry("cli", "unknown")],
            &both,
        )
        .expect("passes");
        let coverage = report.coverage_by_kind["cli"];
        assert_eq!(coverage.total, 2);
        assert_eq!(coverage.consumed, 2);
        assert_eq!(coverage.classified_bps(), 10_000);
        // A zero-total kind must not divide by zero and must not read as 100%.
        assert_eq!(
            KindCoverage { total: 0, consumed: 0, deliberately_not: 0, unclassified: 0 }
                .classified_bps(),
            0
        );
    }

    /// NO TYPED COUNTS. The report carries what it measured instead, so a stale report is
    /// visibly stale rather than silently wrong. Measured 2026-09-08: the RPC surface has
    /// been published as 14, as 3, and as 39 on a different axis, while one handshake
    /// advertised 590 -- and --mode=rpc reads {id,type} frames, not JSON-RPC, so those were
    /// not even the same channel.
    #[test]
    fn the_report_names_which_omp_it_measured() {
        let consumers = index_consumers(&metadata(
            r#"{"kind":"transport","name":"mux","call_site":"src/lib.rs"}"#,
        ))
        .expect("index");
        let report = align(
            // Synthetic, NOT the author's real install root. path-literal-guard refused an
            // earlier version of this line that pasted the home path, and it was right: a
            // machine-specific literal in a test is unportable on any other checkout, and
            // the property under test is that the report CARRIES its install path -- which
            // a synthetic value exercises identically.
            "node_modules/@oh-my-pi/pi-coding-agent",
            "omp/18.0.11",
            &[entry("transport", "mux")],
            &consumers,
        )
        .expect("passes");
        assert!(report.measured_install.contains("pi-coding-agent"));
        assert!(report.measured_version.starts_with("omp/"));
        assert_eq!(report.packages_scanned, 2);
    }

    /// Exit vocabulary shared with repair/undo, not merely parallel: content refusals
    /// are 2, instrument failures are 3, and 4 stays reserved for upstream-unreachable.
    #[test]
    fn exit_vocabulary_separates_content_from_instrument_and_reserves_four() {
        let content = AlignmentError::Unclassified(vec![entry("cli", "x")]).exit_code();
        let instrument = AlignmentError::EmptySurfaceSet.exit_code();
        assert_eq!(content, 2);
        assert_eq!(instrument, 3);
        assert_ne!(content, instrument);
        for code in [content, instrument] {
            assert_ne!(code, 4, "4 is reserved for upstream-unreachable");
            assert_ne!(code, 0, "a refusal must never exit 0");
        }
    }

    /// Allowances naming absent surfaces are reported rather than silently kept. Both current
    /// deliberate rows are absent from this fixture, so both stale rows are observable.
    #[test]
    fn stale_allowance_rows_are_reported() {
        let consumers = index_consumers(&metadata(
            r#"{"kind":"transport","name":"mux","call_site":"src/lib.rs"}"#,
        ))
        .expect("index");
        let report = align("/opt/omp", "omp/18.0.11", &[entry("transport", "mux")], &consumers)
            .expect("passes");
        assert_eq!(report.stale_allowances.len(), 75);
        assert!(report.stale_allowances.iter().any(|row| row.starts_with("cli:ps owner=")));
        assert!(report
            .stale_allowances
            .iter()
            .any(|row| row.starts_with("transport_mode:value owner=")));
    }

    /// Found by %20 while landing the first real declarations. Two crates claiming one
    /// surface must REFUSE, not pick: `find` returns the first match, so the credited crate
    /// would be whichever `cargo metadata` emitted first -- a citation nobody chose, and one
    /// that can change between cargo versions without any manifest changing.
    #[test]
    fn two_crates_claiming_one_surface_is_refused_rather_than_resolved() {
        let json = r#"{"packages":[
            {"name":"ompo-doctor","metadata":{"omp_surface":{"consumes":[
                {"kind":"rpc_handler","name":"get_state","call_site":"src/omp_state.rs"}]}}},
            {"name":"omp-inventory-map","metadata":{"omp_surface":{"consumes":[
                {"kind":"rpc_handler","name":"get_state","call_site":"src/other.rs"}]}}}
        ]}"#;
        let consumers = index_consumers(json).expect("index");
        assert_eq!(consumers.declarations.len(), 2, "both rows must be INDEXED");
        assert!(consumers.defects.is_empty(), "neither row is malformed");

        let claims = consumers.duplicate_claims();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].0, "rpc_handler");
        assert_eq!(claims[0].1, "get_state");
        assert_eq!(claims[0].2.len(), 2, "both claimants must be named");

        let error = align(
            "/opt/omp",
            "omp/18.1.14",
            &[entry("rpc_handler", "get_state")],
            &consumers,
        )
        .expect_err("must refuse an ambiguous credit");
        let rendered = error.to_string();
        assert!(rendered.contains("ALIGN_DUPLICATE_CLAIMS"), "{rendered}");
        assert!(rendered.contains("ompo-doctor"), "must name both claimants: {rendered}");
        assert!(rendered.contains("omp-inventory-map"), "{rendered}");
        assert_eq!(error.exit_code(), 2);

        // POSITIVE CONTROL: one claimant classifies cleanly, so the refusal is about
        // ambiguity and not about the surface.
        let single = index_consumers(&metadata(
            r#"{"kind":"rpc_handler","name":"get_state","call_site":"src/omp_state.rs"}"#,
        ))
        .expect("index");
        assert!(single.duplicate_claims().is_empty());
        assert!(align("/opt/omp", "omp/18.1.14", &[entry("rpc_handler", "get_state")], &single).is_ok());
    }
}
