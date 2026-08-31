# 02 — What we are mapping: every OMP surface

Every number in this section is `MEASURED` unless the sentence says `PROJECTED`. The
measurements come from one artifact: the built scanner `omp-inventory-map`, run as

```
/Volumes/BuildShared/cargo-targets/debug/omp-inventory-map > /tmp/inv.txt   # 544697 bytes, exit 2
```

against installed `omp/18.0.11` on 2026-08-31. Every derived count below is a
`python3` query over that file, and the query is printed next to the number it
produces. Nothing in this section is estimated, remembered, or inferred from
reading source.

### 1. The census, in one table

The scanner emits a versioned envelope,
`{"schema_version":"omp-inventory-map/v1","command":"doctor","status":"UNKNOWN","data":{…}}`,
carrying 184 nodes, 207 edges, and **183 rows**. The denominator is worth stating
plainly, because a census with an unstated denominator is a press release: **183
rows = every OMP surface the probe could enumerate, plus our own 26 workspace
crates.** It is not 183 OMP features. It is 157 OMP surfaces and 26 things we built.

`MEASURED` — `python3 -c "import json,collections; d=json.load(open('/tmp/inv.txt'))['data']; print(collections.Counter(r['kind'] for r in d['rows']))"`

| Row kind | Count | What one row is |
|---|---|---|
| `type_root` | 57 | A top-level directory in OMP's shipped TypeScript type surface |
| `rpc_handler` | 42 | A named method the `--mode=rpc` transport dispatches |
| `cli_command` | 39 | A subcommand enumerated from `omp --help` |
| `workspace_crate` | 26 | One of *our* crates, from `cargo metadata --no-deps` |
| `declaration` | 14 | A top-level `.d.ts` file in the shipped type surface |
| `omp_method` | 3 | A JSON-RPC method whose name matches `omp/*` |
| `slash_command` | 1 | A single `UNKNOWN_PROBE` placeholder — see §3 |
| `transport` | 1 | The process-level transport selector |
| **Total** | **183** | |

The envelope's own `counts` block agrees with the row tally on every kind and
carries an `expected_*` twin for each. Six of seven twins match exactly. One does
not: `expected_slash_commands: 136` against `slash_commands: 0`. That mismatch is
the honest reason `status` is `UNKNOWN` and the process exits 2 — the scanner
knows it failed to enumerate slash commands and refuses to report a verdict it
did not earn. A timeout is not a verdict; neither is an empty probe.

### 2. The coverage headline

`MEASURED` — `python3 -c "import json,collections; print(collections.Counter(r['classification'] for r in json.load(open('/tmp/inv.txt'))['data']['rows']))"`

```
CAPABILITY_NOT_USED             157
SCRAPED_OR_OBSERVED_ALTERNATIVE  18
MAPPED_BY_DIRECT_PROBE            8
```

The arithmetic, written out so it can be attacked:

- direct-probe coverage = 8 / 183 = **4.37%** (8 ÷ 183 = 0.043715…)
- alternative-path coverage = 18 / 183 = **9.84%** (18 ÷ 183 = 0.098360…)
- unconsumed capability = 157 / 183 = **85.79%** (157 ÷ 183 = 0.857923…)
- 8 + 18 + 157 = 183, so the three classes partition the census with no residue.

The edge graph tells the same story from the other side. Of 207 edges, exactly **7
are `consumes`**, and all 7 originate from a single crate, `omp-inventory-map`,
each carrying the evidence string *"direct process probe produced this row"*. They
point at `type_root:cli`, `type_root:commands`, `type_root:jsonrpc`,
`type_root:slash-commands`, `rpc_handler:get_available_commands`,
`slash_command:UNKNOWN_PROBE`, and `transport:--mode=<value>`. So:

- crates consuming any OMP surface = 1 / 26 = **3.85%** (1 ÷ 26 = 0.038461…)
- `consumes` share of all edges = 7 / 207 = **3.38%** (7 ÷ 207 = 0.033816…)

`MEASURED` — the one crate that consumes OMP surface is the crate whose job is to
enumerate OMP surface. Twenty-five of twenty-six crates consume none.

An investor should read that as the project's **central open question, not its
verdict**. Two readings are available and we are obliged to state the hostile one
first. *Hostile reading:* an orchestrator built on OMP that touches 4.37% of OMP is
not an orchestrator, it is a census with ambitions, and the 25 crates are gates and
lints that would work identically if OMP did not exist. *Our reading:* the map is
honest, which is the hard part and the part usually skipped — most projects at this
stage cannot tell you their consumption ratio at all, because nobody enumerated the
denominator. We enumerated it, we published it, and it says 4.37%. The plan's job
from here is to move that number by named decisions, one surface at a time, with a
disposition on each of the 157.

`PROJECTED` — we expect direct-probe coverage to rise as the RPC session crate
wires named handlers, but this document makes no forecast of a target percentage,
because a coverage target would immediately become a metric to game: wiring a
handler nobody calls raises the ratio and lowers the truth.

There is a structural reason for the ratio, and it is recorded in the brief's
four-layer reality table rather than discovered here: of the five layers
(observe / actionable / consume / actuate / complete), exactly one — `observe`,
via `tick-monitor` — is `MEASURED` as WORKS. `actuate` **does not exist**; a human
types into panes. A project whose actuation layer does not exist cannot consume an
actuation surface, so the 157 `CAPABILITY_NOT_USED` rows are not 157 independent
oversights. They are largely one missing layer, counted 157 times. That reframing
makes the number smaller *and* the fix harder: it is one hard thing, not 157 easy
ones. `NO-CLAIM:` this attributes the ratio to the missing actuation layer as an
explanation, not as a measurement — no experiment here isolates how many of the
157 rows would flip once actuation exists.

### 3. The surface, enumerated by kind

`MEASURED` — all member lists below are produced by
`python3 -c "import json,collections; d=json.load(open('/tmp/inv.txt'))['data']; by=collections.defaultdict(list); [by[r['kind']].append(r['surface'].split(':',1)[1]) for r in d['rows']]; print(sorted(by['<KIND>']))"`.

#### cli_command — 39 of 39, listed in full

`acp`, `agents`, `auth-broker`, `auth-gateway`, `bench`, `browser-relay`,
`cleanse`, `commit`, `completions`, `compress`, `config`, `dry-balance`,
`gallery`, `gc`, `git`, `grep`, `grievances`, `if-bench`, `images`, `install`,
`join`, `models`, `plugin`, `ps`, `read`, `render`, `say`, `search`, `setup`,
`share`, `shell`, `ssh`, `stats`, `tiny-models`, `token`, `ttsr`, `update`,
`usage`, `worktree`.

All 39 classify `CAPABILITY_NOT_USED`. We do not shell out to a single OMP
subcommand today, which is a deliberate consequence of the no-shell rule: a
subcommand invocation is a subprocess, and a subprocess without the
`subprocess-contract` (group kill, both pipes drained, `&Cx` first) is not
allowed to exist in this repo.

#### omp_method — 3 of 3, listed in full

`omp/muxConnect`, `omp/muxPing`, `omp/muxRestartServer`.

These are the only three methods on the installed binary whose names match
`omp/*`. All three classify `CAPABILITY_NOT_USED`. This is the number that
retired an earlier claim: an older draft of `AGENTS.md` asserted "81 JSON-RPC
methods, 17 used". That figure was not re-derivable from any probe and has been
struck. The measured surface is 3 `omp/*` methods and 42 bare-named RPC handlers.

#### transport — 1 of 1, listed in full

`--mode=<value>`.

One transport selector, classification `MAPPED_BY_DIRECT_PROBE`, owner
`omp-inventory-map`. It is one of the 7 `consumes` edges.

#### slash_command — 1 row, and it is a confession

`UNKNOWN_PROBE`, classification `MAPPED_BY_DIRECT_PROBE`. The envelope's
`counts.slash_commands` is `0` while `counts.expected_slash_commands` is `136`.
The scanner could not enumerate slash commands, so instead of emitting 136
guesses or 0 rows and calling it clean, it emits one row named `UNKNOWN_PROBE`
and drives `status` to `UNKNOWN` with exit 2. `MEASURED` — the single largest
unmapped region of the OMP surface is 136 slash commands we have never seen.

**Recorded under R11, because it is not yet written down anywhere else.** The
brief's §3.2 lists `slash_commands=0` among the counts but does not carry
`expected_slash_commands=136`, and no section owns the gap. Writing it here makes
it a requirement rather than a session memory: *the census is not complete until
slash-command enumeration either succeeds or carries a named reason for why it
cannot.* `MEASURED` — the discrepancy is in the artifact:
`python3 -c "import json; c=json.load(open('/tmp/inv.txt'))['data']['counts']; print(c['slash_commands'], c['expected_slash_commands'])"`
→ `0 136`. Six of the seven `expected_*` twins match their measured count exactly;
this is the only one that does not. `NO-CLAIM:` we do not know that 136 is the
true number of slash commands — it is the scanner's expectation, and an
expectation that was never satisfied is not a measurement of the world.

#### declaration — 14 of 14, listed in full

`cli-commands.d.ts`, `cli.d.ts`, `config.d.ts`, `cursor-bridge-tools.d.ts`,
`cursor.d.ts`, `index.d.ts`, `main.d.ts`, `sdk.d.ts`, `startup-splash.d.ts`,
`system-prompt.d.ts`, `telemetry-export-otlp.d.ts`, `telemetry-export.d.ts`,
`thinking.d.ts`, `workspace-tree.d.ts`. All 14 classify `CAPABILITY_NOT_USED`.

#### rpc_handler — 42 total; all 42 named

`abort`, `abort_and_prompt`, `abort_bash`, `abort_retry`, `bash`, `branch`,
`compact`, `cycle_model`, `cycle_thinking_level`, `export_html`, `follow_up`,
`get_available_commands`, `get_available_models`, `get_branch_messages`,
`get_last_assistant_text`, `get_login_providers`, `get_messages`,
`get_messages_page`, `get_session_stats`, `get_state`, `get_subagent_messages`,
`get_subagents`, `handoff`, `login`, `negotiate_protocol`, `new_session`,
`prompt`, `set_auto_compaction`, `set_auto_retry`, `set_fast_mode`,
`set_follow_up_mode`, `set_host_tools`, `set_host_uri_schemes`,
`set_interrupt_mode`, `set_model`, `set_session_name`, `set_steering_mode`,
`set_subagent_subscription`, `set_thinking_level`, `set_todos`, `steer`,
`switch_session`.

Split by classification: 36 `CAPABILITY_NOT_USED`, 5
`SCRAPED_OR_OBSERVED_ALTERNATIVE` (`bash`, `follow_up`, `get_state`, `prompt`,
`steer`), 1 `MAPPED_BY_DIRECT_PROBE` (`get_available_commands`). Those five are
precisely the handlers an orchestrator most wants — send a prompt, steer a running
agent, read its state, run a command, follow up — and today we obtain each of them
some other way. That is stated as a debt in §4, not as a design.

#### type_root — 57 total; all 57 named

`advisor`, `async`, `auto-thinking`, `autolearn`, `autoresearch`, `blob-broker`,
`capability`, `cleanse`, `cli`, `collab`, `commands`, `commit`, `compress`,
`config`, `dap`, `debug`, `discovery`, `edit`, `eval`, `exa`, `exec`, `export`,
`extensibility`, `goals`, `hindsight`, `if-bench`, `internal-urls`, `irc`,
`jsonrpc`, `launch`, `lib`, `live`, `lsp`, `markit`, `mcp`, `memories`,
`memory-backend`, `mnemopi`, `modes`, `plan-mode`, `registry`, `secrets`,
`security`, `session`, `sharpshooter`, `slash-commands`, `ssh`, `stt`,
`subprocess`, `task`, `tiny`, `tools`, `tts`, `tui`, `utils`, `vibe`, `web`.

Split: 40 `CAPABILITY_NOT_USED`, 13 `SCRAPED_OR_OBSERVED_ALTERNATIVE` (`dap`,
`debug`, `exec`, `goals`, `memories`, `memory-backend`, `mnemopi`, `modes`,
`plan-mode`, `session`, `subprocess`, `task`, `tools`), 4
`MAPPED_BY_DIRECT_PROBE` (`cli`, `commands`, `jsonrpc`, `slash-commands`).

#### workspace_crate — 26 of 26, listed in full

`ack-spine`, `ack-stage`, `commit-build-fence`, `composer-typed`,
`dispatch-claim-fence`, `dispatch-silence-watch`, `finding`, `finding-dispatch`,
`fleet-composite`, `installer`, `kernel-bypass-gate`,
`kernel-only-operator-hook`, `loop-queue-filter`, `no-shell-gate`,
`omp-inventory-map`, `omp-orchestrator`, `omp-rpc-session`, `omp-types`,
`pane-dispatch-fence`, `path-literal-guard`, `pre-delete-citation-check`,
`receiver-receipt`, `state-wildcard-lint`, `subprocess-contract`, `tick-monitor`,
`undrained-pipe-lint`.

25 classify `CAPABILITY_NOT_USED`; only `omp-inventory-map` is
`MAPPED_BY_DIRECT_PROBE`, with the reason *"This crate owns generation and direct
probe orchestration."* Note what `CAPABILITY_NOT_USED` means when applied to our
own crate: the scanner is saying **we do not consume our own crate from any
measured runtime trigger**, which is the same finding as `omp-types` having zero
dependents. The classification is uncomfortable and correct.

### 4. The three classifications, defined and dispositioned

The census carries an `orphan_disposition` on every row. `MEASURED` —
`python3 -c "import json,collections; print(collections.Counter(r['orphan_disposition'] for r in json.load(open('/tmp/inv.txt'))['data']['rows']))"`
yields `NAMED_REASON: 175, WIRE: 8`. There is no third value, and there must never
be one, because the third value is always "later".

**`MAPPED_BY_DIRECT_PROBE` (8 rows) — we actually touch it.** A live process probe
produced the row. The evidence string is *"direct process probe produced this
row"*, the owning crate is named, and a `consumes` edge exists in the graph. This
is the only class that carries runtime truth. Disposition: keep, and keep probing —
a mapped row that stops being probed silently degrades to a scraped row.

**`SCRAPED_OR_OBSERVED_ALTERNATIVE` (18 rows) — we get the information some other
way.** The row's reason text is uniform: *"No typed runtime adapter owns
`<surface>`; retain as a named wire candidate."* Naming the alternatives, which is
the part that makes this class honest rather than a synonym for "unused":

- `rpc_handler:prompt` and `rpc_handler:steer` — dispatch reaches panes through
  `ntm` (`ntm version v1.30.0-1-gda270719`) and the pane-dispatch fence, not
  through OMP's RPC. The alternative is a terminal multiplexer.
- `rpc_handler:get_state` — agent liveness is inferred by `tick-monitor` and
  `dispatch-silence-watch` from observed pane output, not asked for over RPC. The
  alternative is silence-timing.
- `rpc_handler:bash` — command execution goes through `subprocess-contract`
  (group kill, both pipes drained) under the no-shell rule; OMP's `bash` handler
  is never reached. The alternative is our own spawn discipline.
- `rpc_handler:follow_up` — follow-ups are modelled as `receiver-receipt` and
  `ack-stage` obligations. The alternative is our ack ledger.
- `type_root:task`, `type_root:goals`, `type_root:plan-mode` — work items live in
  `br 0.4.1` beads and are ranked by `bv v0.20.0`. The alternative is the bead board.
- `type_root:memories`, `type_root:memory-backend`, `type_root:mnemopi` — recall
  is served by the harness's own memory tools. The alternative is the harness.
- `type_root:subprocess` and `type_root:exec` — superseded by our
  `subprocess-contract` crate, which encodes asupersync 0.4.9 (`fa3c01aec`) rules
  OMP's surface does not promise.
- `type_root:session` — sessions are tracked by `omp-rpc-session` locally.
- `type_root:dap`, `type_root:debug`, `type_root:tools`, `type_root:modes` — no
  alternative is named today. `MEASURED` — these four are the rows where the class
  is doing the least work, and they are the first candidates to be re-classified
  down to `CAPABILITY_NOT_USED` so they inherit a real disposition.

Disposition: every scraped row must name its alternative in one sentence, and the
alternative must be a thing that exists. A scraped row whose alternative is "we
plan to" is a `CAPABILITY_NOT_USED` row wearing a better hat.

**`CAPABILITY_NOT_USED` (157 rows) — a real capability we do not consume.** The
reason text is *"The repository has no measured runtime trigger for `<surface>`."*
This class admits exactly two dispositions and no third:

- **WIRE** — we will consume it, and a bead exists that says who and when.
- **RETIRE / NAMED_REASON** — we will not consume it, and the row carries the
  sentence explaining why, in a form an investor can dispute.

"Not yet triaged" is not a disposition. `MEASURED` — 175 rows currently sit at
`NAMED_REASON` and 8 at `WIRE`, so 175 ÷ 183 = **95.63%** of the census is
currently answered with a reason rather than a plan. `PROJECTED` — the plan's
first milestone converts a named subset of those reasons into WIRE beads; this
document does not pre-commit the size of that subset, because a number chosen
before the triage is a number chosen to look good.

### 5. The vacuity finding, stated against ourselves

This is the sharpest thing the session produced, and it is an indictment of our own
process discipline, so it belongs here rather than in a footnote.

`MEASURED` — reproduced exactly:

```
python3 -c "…collections.Counter(json.dumps(r.get('must_be_true')) …)"
  crate rows:     n=26   distinct must_be_true=1  distinct negative_evidence=1
  non-crate rows: n=157  distinct must_be_true=1  distinct negative_evidence=1
```

Re-run on the live artifact for this section, with the missing-field check added:

```
python3 -c "
import json
d=json.load(open('/tmp/inv.txt'))['data']; rows=d['rows']
cr=[r for r in rows if r['kind']=='workspace_crate']; nc=[r for r in rows if r['kind']!='workspace_crate']
for nm,s in [('crate',cr),('non-crate',nc)]:
    print(nm,'n=%d'%len(s),
      'distinct must_be_true=%d'%len({json.dumps(r['must_be_true']) for r in s}),
      'distinct negative_evidence=%d'%len({json.dumps(r['negative_evidence']) for r in s}),
      'missing=%d'%sum(1 for r in s if not all(r.get(f) for f in ('inputs','outputs','must_be_true','negative_evidence'))))
"
  crate     n=26  distinct must_be_true=1  distinct negative_evidence=1  missing=0
  non-crate n=157 distinct must_be_true=1  distinct negative_evidence=1  missing=0
```

All 183 rows carry all four mandatory fields — `inputs`, `outputs`,
`must_be_true`, `negative_evidence` — with **zero missing**. And across the entire
census there is **exactly one distinct `must_be_true` and one distinct
`negative_evidence`**:

```
must_be_true      = ["The source probe is non-empty before a known verdict is emitted.",
                     "A versioned inventory envelope carries the probe state."]
negative_evidence = ["No repository source grep was used; ownership is derived from
                     metadata and direct probes."]
```

The four-field discipline the orchestrator demanded of every worker is therefore
satisfied **syntactically and vacuously**. It matters for one reason, and the
reason generalises well past this repo: *a census where every row carries identical
invariants proves the fields were populated, not that anything was checked.* A
per-row `must_be_true` is supposed to be the thing that could have been false about
**that row**. If it is the same sentence 183 times, it is a property of the
scanner's envelope, not of the surface, and no row-level falsification is possible.
A validator asserting "every row has a non-empty `must_be_true`" passes at 100%
while checking nothing.

It is worse for crate rows specifically. `MEASURED` — for a crate row, `inputs` is
`cargo metadata --format-version 1 --no-deps` plus that crate's `Cargo.toml`, and
`what_it_provides` is the boilerplate *"Workspace crate `<name>` from cargo
metadata"*. Those fields describe **the scanner's own provenance**, not the crate's
contract. The 26 `what_it_provides` strings are distinct only because the crate
name varies inside a fixed template. Sampling `outputs` shows the same shape:
`ack-spine` → `ack_spine, ack-spine, ack_detector, authorities, followup`;
`ack-stage` → `ack_stage`; `commit-build-fence` → `commit_build_fence,
commit-build-fence, hook`; `composer-typed` → `composer_typed, composer-typed,
differential, mutation, planted_known_bads`. These are cargo target names. They
tell you what the crate compiles, not what it promises.

**The fix, as future work.** `PROJECTED` — two changes, both cheap, both testable:

1. **Per-row invariants that differ by row.** Each row's `must_be_true` must state
   something falsifiable about *that surface*: for `cli_command:worktree`, "the
   installed binary lists `worktree` in `omp --help`"; for `crate:ack-spine`, "the
   crate exposes an ack detector reachable from `finding-dispatch`". A row whose
   invariant is reusable verbatim by another row has not written an invariant.
2. **An anti-vacuity gate.** A gate leg that loads any emitted census and **fails
   when `distinct-invariant-count == 1` across a census of more than one row**, and
   more generally when the distinct-invariant ratio falls below a floor. This is
   the same shape as the `known_bad` leg the gate framework already requires: it is
   a planted-known-bad for the *metadata*, not the code. `MEASURED` — the
   `no-shell-gate` crate is the only one of eight gates today carrying all four
   legs (6 anti-vacuity files by `grep -rli`), so the pattern exists in-repo and
   needs porting, not inventing.

What would Jeffrey do here? The pattern we are reaching for — refuse to emit a
verdict the evidence does not support, and make the refusal typed — is already the
shape of this scanner's `status: UNKNOWN` / exit 2 behaviour, which is itself
modelled on the mirror's fail-closed convention (compare `fh`'s typed
`SERVE_INPUT_STALE` refusal when its mirror HEAD moved `5dec4212…` → `ecdea397…`,
rather than serving stale results). `MEASURED` — the `fh` MCP surface is failing
closed with exactly that typed error as of this session, which is the behaviour we
want and a live demonstration that it is survivable. We did not run a mirror grep
for a pre-existing distinct-invariant-ratio gate; that search is named as an open
item for the prior-art section rather than claimed here.

There is also a sixth gate property this census forced into existence, and it
belongs on the record next to the vacuity finding because it has the same cause —
a gate that is correct but unreachable is as vacuous as an invariant that is
populated but identical. `MEASURED` — `omp-inventory-map --help` returns
`{"status":"ERROR","error":"CONFIG_ERROR unknown argument --help"}`. The gate is
built and correct (13 tests pass; `types_inventory.rs:176-178` deliberately
excludes `Observation` from the allowance list so the collision demands
convergence) and **undiscoverable**. Hence the sixth property: **ADDRESSABLE** —
one documented command runs the gate, and `--help` names that command.

I re-derived the brief's §3.6 corroborating claim independently rather than
inheriting it, because it is the one fact in the brief that lives inside *my*
artifact. `MEASURED` —
`for w in Observation CONVERGE Verdict; do printf '%s ' "$w"; grep -o "$w" /tmp/inv.txt | wc -l; done`
→ `Observation 0`, `CONVERGE 0`, `Verdict 0`. The 544,697-byte doctor output
contains none of the three. **I agree with the brief.** The type-collision logic
is present in the source and absent from every byte the running binary emits,
which is the precise definition of wired-but-unaddressable. `NO-CLAIM:` a string
absent from the doctor output does not prove the check never runs — it proves the
check is unobservable from the only output the binary offers, which is the defect
being named.

---

`NO-CLAIM:` This section claims only what the 2026-08-31 run of
`omp-inventory-map` against `omp/18.0.11` on this machine emitted, plus arithmetic
over that file. It does **not** claim the census is complete — `expected_slash_commands: 136`
against `slash_commands: 0` proves it is not, and 136 slash commands remain
entirely unenumerated. It does **not** claim the 42 `rpc_handler` names are the
whole RPC surface, only the whole set the probe returned. It does **not** claim any
`CAPABILITY_NOT_USED` row is genuinely unused at runtime — only that no *measured*
runtime trigger exists, and an unmeasured trigger would look identical. It does
**not** claim the 18 `SCRAPED_OR_OBSERVED_ALTERNATIVE` alternatives named in §4 are
adequate substitutes for the OMP surfaces they stand in for; four of them
(`dap`, `debug`, `tools`, `modes`) have no named alternative at all. It does
**not** claim the coverage percentages will move, or should move to any particular
figure. And it makes **no** claim that the four mandatory fields on any row have
been independently verified — §5 is the measurement that they have not.
