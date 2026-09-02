# Ground Truth Contract

Bead: `omp-orchestrator-ground-truth-contract-gm1`

## Purpose

This contract defines the authority boundary for fleet observation: tmux authoritatively answers whether panes exist and their geometry, while ntm supplies named projections of sessions, activity, and coordination state that may be stale or empty-success. A comparator must preserve both answers, emit an error on disagreement, reject an empty or unreadable oracle, and never use a `fresh=true` flag as permission to trust a zero projection; the contract therefore distinguishes what a surface observes from what it projects and makes the independent oracle relationship explicit.

## Contract Artifacts

1. Direct pane oracle: `tmux list-panes -a` with an explicit format containing session, window, pane, command, width, and height.
2. Projection surfaces: `ntm list` and `ntm --robot-activity=<session>`; `ntm --robot-snapshot` is diagnostic evidence, never the existence oracle.
3. INVARIANT SUITE: `crates/fleet-reconcile/tests/differential.rs` and `crates/fleet-reconcile/tests/mutation.rs`.

The existing invariant suite compares the Rust implementation against its independent shell oracle, plants disagreement and empty-success fixtures, and exercises a mutation that disables the name-set rule. `crates/pane-oracle-diff/tests/planted_known_bads.rs` is the adjacent planted-known-bad suite for the same boundary.

## Ground Truth Model

Every authority boundary gets a stable ID.

| Value | Property | Description |
|---|---|---|
| `GT-TMUX-PANE-EXISTS` | authoritative | A pane listed by tmux exists in the selected tmux server. |
| `GT-TMUX-GEOMETRY` | authoritative | Pane window/pane coordinates, dimensions, and current command come from tmux. |
| `GT-NTM-LIST` | projected-enumeration | `ntm list` names the projection's sessions; it is compared with tmux before existence is asserted. |
| `GT-NTM-ACTIVITY` | projected-activity | `ntm --robot-activity=<session>` reports activity/projection state; it does not create panes or override tmux geometry. |
| `GT-NTM-SNAPSHOT` | stale-prone | A successful snapshot may contain `total_sessions: 0` while live sessions exist. |
| `GT-FRESH-FLAG` | non-authoritative | `fresh=true` describes projection age, not correctness or completeness. |
| `GT-DISAGREEMENT` | restrictive-error | A mismatch between independent surfaces is an error, not a tie-break. |
| `GT-EMPTY-ORACLE` | restrictive-error | An empty, missing, unreadable, or unparseable oracle is an error, never agreement. |

### Properties

- **GT-P1-EXISTENCE**: tmux is authoritative for pane existence and geometry.
- **GT-P2-PROJECTION**: ntm is a projection; successful empty output is not absence.
- **GT-P3-COMPARISON**: a comparator retains both observations and reports disagreement explicitly.
- **GT-P4-ANTI-VACUITY**: zero comparison rows or unreadable oracle input is an error.
- **GT-P5-FRESHNESS**: `fresh=true` never suppresses an independent comparison.
- **GT-P6-ADDRESSABILITY**: the live no-silent-zero surface set is `ntm list`, `ntm --robot-activity=<session>`, and `tmux list-panes -a`; each command's scope and exit status remain evidence fields.

## Laws

- **L1-TMUX-AUTHORITY** — pane existence and geometry are read from tmux; ntm cannot create an existence or geometry fact. *Test:* `crates/fleet-reconcile/tests/differential.rs::rust_matches_shell_on_nonempty_fixture_set`.
- **L2-NTM-PROJECTION** — a successful empty ntm snapshot must not be accepted as an empty fleet when tmux has panes. *Test:* `crates/fleet-reconcile/tests/differential.rs::rust_matches_shell_on_nonempty_fixture_set`.
- **L3-DISAGREEMENT-ERROR** — any disagreement between the independent surfaces is a named failure, never a silent preference for ntm or tmux. *Test:* `crates/fleet-reconcile/tests/differential.rs::comparator_sees_manufactured_disagreement`.
- **L4-EMPTY-ORACLE** — an empty comparison set, missing oracle, unreadable input, or unparseable projection cannot produce agreement. *Test:* `crates/fleet-reconcile/tests/differential.rs::rust_matches_shell_on_nonempty_fixture_set`.
- **L5-FRESH-NOT-SAFE** — `fresh=true` does not authorize a zero session result; compare with `ntm list` and tmux regardless of freshness. *Test:* `crates/pane-oracle-diff/tests/planted_known_bads.rs`.

The laws are comparison laws: they require a known-good agreement control, a planted disagreement, and an empty/unreadable refusal. They do not turn a projection into direct observation.

## Authority and Recovery Rules

- **GT-R1-DIRECT-ORACLE**: use `tmux list-panes -a` for pane existence and geometry. Include `session_name`, `window_index`, `pane_index`, `pane_id`, `pane_width`, `pane_height`, and `pane_current_command` in the capture when geometry matters.
- **GT-R2-PROJECTION-READ**: use `ntm list` for the named session projection and `ntm --robot-activity=<session>` for activity state, but compare both with tmux before a dispatch or capacity claim.
- **GT-R3-EMPTY-FAILURE**: `total_sessions: 0`, an empty `ntm list`, empty activity output, an absent socket, malformed JSON, or a missing tmux capture is a refusal with an evidence code, not a clean zero.
- **GT-R4-FRESH-IGNORED**: a fresh projection can still be empty or wrong; the age bit may be reported but cannot bypass comparison.
- **GT-R5-SERIAL-PROBE**: probe NTM commands serially. A concurrent command harness can manufacture timeouts through shared runtime and pipe contention; a timeout is not proof that the command itself is hung.
- **GT-R6-RECHECK**: on disagreement, preserve both raw observations, command identities, exit codes, and timestamps; do not silently retry until the mismatch disappears.

## Validation

```bash
cargo test -p fleet-reconcile --test differential --offline -- --nocapture
```

## Cross-References

- `crates/fleet-reconcile/src/lib.rs` — comparator and explicit ntm/tmux disagreement rules
- `crates/fleet-reconcile/src/main.rs` — JSON command surface and failure output
- `crates/fleet-reconcile/tests/differential.rs` — independent shell/Rust comparison suite
- `crates/fleet-reconcile/tests/mutation.rs` — mutation coverage for the comparison rule
- `crates/pane-oracle-diff/src/lib.rs` — adjacent projection-vs-oracle comparator
- `crates/pane-oracle-diff/tests/planted_known_bads.rs` — empty/undercount known-bad fixtures
- `crates/pane-truth/src/lib.rs` — direct tmux pane-state interpretation
- `crates/pane-truth/tests/differential.rs` — tmux differential oracle path
- `~/.claude/references/claude-md-ntm-defects.md` — measured stale-projection, fresh-zero, command-reliability, and ground-truth surface defects
- `docs/contracts/pane_observation_contract.md` — adjacent pane observation and two-capture boundary
- `docs/contracts/lifecycle_contract.md` — pane lifecycle states; this contract supplies authority for observations, not lifecycle transitions
- `docs/plans/plan_to_pin_the_orchestrator_type_algebra.md` — contract/type/bead/code ordering

## Non-Coverage

- This contract does not mutate tmux, ntm, launchd, sockets, sessions, panes, or provider state.
- It does not define pane liveness semantics, spinner/timer interpretation, two-capture timing, readiness admission, dispatch fencing, or process lifecycle terminals.
- It does not claim that `ntm list` or `--robot-activity` is infallible; they are projection observations that must participate in an independent comparison.
- It does not define recovery, retry, or tie-breaking policy after a disagreement; the required result here is a restrictive error with retained evidence.
- It does not prove that a command executed on a particular host has the same tmux server, socket, session scope, or permissions as another command.

## NO-CLAIM

A green differential test proves only that the named fixtures and independent shell oracle exercise the comparator's authority rules. It does not prove every live NTM response is current, that tmux itself has no failure, that a pane is safe to dispatch, or that a caller consumes the restrictive disagreement. The contract pins **pane observation authority**; it does not pin the pane lifecycle algebra, the sibling RPC lifecycle, or runtime adoption of any verdict.
