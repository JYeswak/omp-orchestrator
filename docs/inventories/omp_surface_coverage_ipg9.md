# OMP surface coverage — wave `ipg.9` (SECURITY): secrets, security, extensibility, config

Bead: `omp-orchestrator-omp-coverage-mission-ipg.9` — Wave SECURITY: secrets, security, extensibility, config

## Purpose

Classifies the four OMP **SECURITY** surfaces — secrets, security, extensibility and config — under the hook-certification constraint that a hook error reads as DENY across the whole fleet.

It owns the CLASSIFICATION of these 4 surfaces and nothing else. The machine-readable dispositions live in `docs/plan/OMP-COVERAGE-TABLE.jsonl`; the decision to adopt any category-(c) row is a bead, not a sentence here.

Extracted verbatim from `docs/plan/12-journey.md` Appendix I (lines 1300–1386 at `50df553`); the only transform is a one-level heading demotion.

---

> **ipg.9**: *Wave SECURITY. Per /hook-certification any hook we register must be Rust,
> asupersync-backed, cancel-correct, registered in hooks_certified.toml, and NEVER
> auto-registered — a hook error reads as DENY and can brick every Write/Edit/Bash in the fleet.*

**Swept 2026-09-01.** Four type roots, 104 files, 1,228KB, 891 exported symbols, walked to symbol
level. All four are agent-plane credential/security/extension/config features. None crosses the
process boundary into our orchestration layer.

| surface | OMP files | OMP KB | OMP symbols | 1-8 clauses | classification |
|---|---:|---:|---:|:-:|---|
| `secrets` | 7 | 44 | 60 | — — — — — — — — | **(a) NOT OURS** — credential placeholder keys and secret obfuscation (getSecretPlaceholderKey, MIN_OBFUSCATE_SECRET_LEN, RegexScanSegment) |
| `security` | 20 | 132 | 124 | — — — — — — — — | **(a) NOT OURS** — cloud security identity management (CodexSecurityCloudClient, ExactSecurityOAuthOptions, selectSecurityAccount, assertSecurityIdentityMatches) |
| `extensibility` | 54 | 376 | 447 | — — — — — — — — | **(a) NOT OURS** — extension/plugin system (Capability<T>, Extension, StringEnum, BashSpawnHook, provider-trust hooks); **largest by symbol count** |
| `config` | 23 | 672 | 260 | — — — — — — — — | **(a) NOT OURS** — settings schema and API-key resolution (ApiKeyResolver, ModelRegistry, showHookStatus); **largest by size** |

**Positive control: FAILED — 0 of 4 FULLY COVERED.** Eighth consecutive wave. The pattern is
exhaustive and structural: every OMP type root is either orchestration-plane (consumed in wave 1)
or agent-plane (not adopted). No exceptions have been discovered across eight waves and 40+
surfaces.

**Anti-vacuity: PASSED** — 4 surfaces enumerated, 104 files walked to symbol level, 0 is not the
count.

#### Per-surface detail

**`secrets` — (a) NOT OURS.** `getSecretPlaceholderKey`, `getExistingSecretPlaceholderKey`,
`MIN_OBFUSCATE_SECRET_LEN`, `RegexScanSegment`, `ReplaceRegexScan` — credential placeholder
generation and secret obfuscation/redaction for OMP's own providers. Our orchestrator holds no
credentials; coupling them to a vendored tool is the 08 §3 rule this surface would violate.

**`security` — (a) NOT OURS.** `CodexSecurityCloudClient`, `ExactSecurityOAuthOptions`,
`selectSecurityAccount`, `assertSecurityIdentityMatches` — cloud security identity management for
the Codex upstream. No hook types; the security surface is authz/OAuth for OMP's provider
connections, not dispatch-safety policy.

**`extensibility` — (a) NOT OURS.** 447 symbols across 54 files — **the largest surface by symbol
count in the entire workspace.** `Capability<T>`, `Extension`, `ExtensionManifest`,
`StringEnum`, `clampThinkingLevel`, `BashSpawnHook`, provider-trust hooks (legacy shim). This is
OMP's extension/plugin loading system: how it discovers, validates, and instantiates agent
capabilities from installed extensions. No crate in our workspace loads agent extensions. The
`BashSpawnHook` type is a JavaScript hook, not a Rust hook — the /hook-certification doctrine
(Rust, asupersync-backed, cancel-correct, hooks_certified.toml) does not apply to OMP's
JS extension hooks.

**`config` — (a) NOT OURS.** 672KB — the largest surface by size. `ApiKeyResolver`,
`ApiKeyResolverRegistry`, `ModelRegistry`, settings schema (including `statusLine.showHookStatus`).
Ambient config would make spawns environment-dependent; our crates pass explicit flags for
receipt discipline. The `statusLine.showHookStatus` setting confirms OMP has a hook-status display
surface, but it is a UI setting, not a hook-registration API.

#### The hook-certification angle, assessed honestly

None of the four surfaces contains a hook-registration API that competes with /hook-certification.
The `BashSpawnHook` in extensibility is a JavaScript callback in OMP's extension system, not a
system-level hook — it cannot brick Write/Edit/Bash the way a bad pre-commit hook can. The
`statusLine.showHookStatus` setting is a display toggle. The /hook-certification doctrine (Rust,
asupersync-backed, cancel-correct, hooks_certified.toml, never auto-registered) is our own
standard for OUR hooks, and no OMP surface provides an alternative that would bypass it.

The closest crossing point is the `config` surface: if OMP's settings could register hooks, the
config→hook path would be a bypass of /hook-certification. Measured: settings-schema.d.ts contains
`showHookStatus` (a display toggle) but no hook-registration API. The bypass does not exist.

---

#### BLOCKER resolution — the ipg.6 symbol count had three values and no rule

`GradeJourney` filed a BLOCKER: prose said **533**, the table above it sums to
**503**. Re-measuring produced a third answer, **621**.

Three numbers, and the defect is not arithmetic — **none of them shipped a
derivation**, so none could be checked and none could be wrong. The gap is
concentrated rather than spread: `eval` counts 207 under an explicit rule against
the table's 95, which is 112 of the 118-symbol spread on its own.

Registered as `ipg6_root_symbols` with the counting rule stated in the command.
That does not make 621 truer than 533 — it makes it **falsifiable**, which is the
only property the other two lacked. If the rule is wrong, the command is where to
argue with it.

**This is the section's own Foundation preflight loop rule applied to the section:** *"every number
carries the command that derives it."* Three consecutive rounds graded this
section and none caught it, because a reader comparing prose to a table sees two
numbers and picks one. Only re-running a command produces a third.

## Cross-References

- `docs/inventories/omp_surface_coverage_index.md` — the eleven-wave roll-up, and the wave that has no document
- `docs/plan/12-journey.md` — the nine-stage runbook this appendix was extracted from
- `docs/plan/OMP-COVERAGE-TABLE.jsonl` — machine-readable dispositions
- `.beads/issues.jsonl` — `omp-orchestrator-omp-coverage-mission-ipg.9`

## Validation

Derives both sides and compares them: the surface set this document tabulates, against the
surface set its bead declares. It quotes neither, so it cannot go stale the way a copied list
does. An empty parse on either side is FAIL, never PASS.

```bash
cd /Users/josh/Developer/omp-orchestrator && W=9 && \
D=$(sed -nE 's/^\| `?([a-z0-9:_-]+)`? \|.*/\1/p' "docs/inventories/omp_surface_coverage_ipg${W}.md" \
    | grep -vE '^(surface|-+)$' | sort -u) && \
B=$(jq -r --arg id "omp-orchestrator-omp-coverage-mission-ipg.${W}" \
      'select(.id==$id)|.title' .beads/issues.jsonl | head -1 \
    | sed 's/^[^:]*: *//' | tr ',' '\n' | tr -d ' ' | sed '/^$/d' | sort -u) && \
[ -n "$D" ] && [ -n "$B" ] && diff <(printf '%s\n' "$D") <(printf '%s\n' "$B") \
  && echo "PASS ipg.${W}" || echo "FAIL ipg.${W}"
```
