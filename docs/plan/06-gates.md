# 06 — The testing, validation, and gating frameworks we apply

*Serves R6 ("the testing / validation / gating frameworks that we are applying") at the design-spec depth R10 demands. Obeys the writing contract in `00-brief.md` §6.*

A gate is a claim about the future: *this class of defect cannot land again.* The claim is worth exactly what the evidence behind it is worth — evidence that the gate would have caught the defect, would have let the legitimate case through, and fires *because of* the predicate it names rather than incidentally. This section states the nine frameworks we apply, why each beats the weaker option, the shape each takes here, and — where a leg is missing — which gate is load-bearing on faith.

It is written to be failed. §2.4 indicts our own census. §2.8 reports a **live RED** in the best-covered gate in the repo, found while writing this section. §5 states the objections we cannot yet answer.

---

## 1. The measured inventory

MEASURED 2026-08-31, re-derived at time of writing. The counts and the table below match `00-brief.md` §3.5's *table* exactly; they do not match its summary prose, and the disagreement is stated with the recomputation that settles it immediately after the table.

```
python3 -c "import pathlib,re; c=pathlib.Path('crates');
  print(len(sorted(c.glob('*/tests/*.rs'))),
        sum(len(re.findall(r'#\[test\]', p.read_text())) for p in c.rglob('*.rs')))"
  -> 26 370
```

Two independent sources agree: this walk, and the brief's own `find`/`grep -rc` measurement (§3.5), which was taken without the `--include=` flag the tooling warning below indicts.

26 integration test files, 370 `#[test]` functions. Per-gate leg inventory (MEASURED, `grep -rli <property>` per gate crate), verbatim from the brief:

| crate | tests | known_bad | known_good | mutation | anti_vacuity |
|---|---:|---:|---:|---:|---:|
| `no-shell-gate` | 34 | 4 | 3 | 2 | 6 |
| `omp-inventory-map` | 23 | 0 | 2 | 1 | 1 |
| `undrained-pipe-lint` | 10 | 1 | 1 | 1 | 3 |
| `commit-build-fence` | 10 | 0 | 1 | 0 | 0 |
| `state-wildcard-lint` | 9 | 1 | 1 | 1 | 0 |
| `kernel-bypass-gate` | 6 | 1 | 1 | 0 | 0 |
| `pre-delete-citation-check` | 6 | 1 | 1 | 0 | 0 |
| `path-literal-guard` | 3 | 1 | 0 | 0 | 2 |

**2 of 8 gates have all four legs** — `no-shell-gate` and `undrained-pipe-lint`. **4 of 8 have no mutation leg**: `commit-build-fence`, `kernel-bypass-gate`, `pre-delete-citation-check`, `path-literal-guard`. 4 of 8 have no anti-vacuity leg. 2 of 8 have no known-bad. 1 of 8 has no known-good.

**Disagreement with the brief, on the brief's own table.** `00-brief.md` §3.5 states "1 of 8 gates has all four legs" and "5 of 8 have no mutation leg." Both contradict the table printed immediately above them. Recomputing from that table verbatim:

```
python3 -c "rows={...verbatim from 00-brief.md §3.5...};
  print(len([k for k,v in rows.items() if all(x>0 for x in v[1:])]))"
  all four legs   : 2 ['no-shell-gate', 'undrained-pipe-lint']
  no known_bad    : 2 ['omp-inventory-map', 'commit-build-fence']
  no known_good   : 1 ['path-literal-guard']
  no mutation     : 4 ['commit-build-fence', 'kernel-bypass-gate',
                       'pre-delete-citation-check', 'path-literal-guard']
  no anti_vacuity : 4 ['commit-build-fence', 'state-wildcard-lint',
                       'kernel-bypass-gate', 'pre-delete-citation-check']
```

`undrained-pipe-lint` carries 1/1/1/3 — all four legs non-zero — so it is complete and was undercounted; and four gates lack a mutation leg, not five. The brief's other two counts are correct. An earlier draft of this section propagated both errors verbatim, which is the finding worth keeping: a headline transcribed rather than recomputed from its own table survives every review that reads the prose and not the arithmetic. The corrected headline is **2 of 8**, and it is worse than it looks, because §3.1 shows **0 of 8** satisfy all six properties.

The objection, stated before it is answered: *you have 370 tests and two complete gates, so the other 326 are decoration.* Partly conceded — several are high-value regression legs against verbatim live captures (§2.9), a distinct and real kind of evidence — but the honest headline is **2 of 8**, and a count of tests is the metric most likely to be gamed by whoever reports it.

**A tooling warning, MEASURED, that changes how the rest of this section is sourced — and is itself a gate violation.** Shell `grep -r` with `--include=` **returns empty instead of failing** in this harness. Measured both directions: `grep -rl 'forbid(unsafe_code)' --include='*.rs' crates` returns **0** (quoted or unquoted) while the harness `grep` tool on the same pattern returns **55 files**; shell `grep -r` *without* `--include` works correctly. A per-crate `grep -ql` loop likewise reported every crate as non-matching against files whose line 1 had already been read.

**The second instance is ours, and an earlier draft of this section got it backwards.** That draft asserted `tmux --version` "prints an error and exits 0 — it fails while reporting success." REFUTED: `tmux --version` exits **1** with empty stdout and 158 bytes on stderr, which is correct, well-behaved failure. The "exits 0" came from a probe reading `$?` after a pipeline, where the status belongs to the last stage — `PIPESTATUS=(1 0)`. **The instrument laundered a clean failure into a success and then reported it as the binary's defect.** tmux is not the offender; our measurement harness is. Retained rather than deleted, because a probe that misattributes its own bug to the thing it measures is a worse failure than the one first alleged, and deleting it would erase the only case in this section where the instrument manufactured the defect it reported.

**And the corrected hazard is inverted, which matters for gate design.** The real risk with a version probe is not exit 0 laundering a failure; it is a probe treating **non-zero as ABSENT** and recording a present binary as missing. `tmux -V` returns `tmux 3.6a` at exit 0, so tmux is present and healthy, while `tmux --version` exits 1 — and no single flag covers our nine binaries (`--version` answers 8 of 9, `-V` answers 5 of 9). A doctor that probes with one flag and reads only the exit status will mark a healthy binary missing.

**What would Jeffrey do — the precedent carries its own remediation AND its own tests.** All citations below name the CONSTRUCT, not just the line, because three of us independently cited this same precedent at three different line numbers and all three were partly right: we were each naming a different construct on adjacent lines. A line number without a construct is unverifiable and does not survive a reformat.

In `pi_agent_rust/src/doctor.rs`: `:924` is `fn check_tool(`, the function. `:950` is the **naive success arm**, `Ok(output) if output.status.success()` => PASS. `:967-968` is the **two-signal fallback arm**, `Ok(output) if discovered_path.is_some() && probe_failure_is_known_nonfatal(tool, args, &output)` => treat tool as present, commented at `:970-971` "Some shells (e.g. dash as `/bin/sh`) do not support `--version`. If this is the known non-fatal probe case, treat tool as present." The two signals are independent by construction: presence comes from `:1066` `fn which_tool(` resolving a path, never from the version probe. `:1052` `fn probe_failure_is_known_nonfatal(` matches stderr against "illegal option", "unknown option", or "invalid option", and `:1057` is the **one-tool allowlist**, `if tool.ne("sh") || args.ne(&["--version"]) { return false; }`.

**And the strongest part is the test pair, which pins BOTH arms.** Verified first-hand. `:13948` `fn check_tool_falls_back_when_probe_args_are_unsupported()` drives `check_tool` with `sh --version` in `ProbeExecution` mode and asserts `Severity::Pass` — the known-good leg for the fallback. `:13964` `fn check_tool_reports_invocation_failure_for_broken_executable()` is a **planted known-bad specimen**, and the craft is in the comment at `:13969-13970`: it writes the literal bytes "not an executable format" to a file and `chmod 0o755`s it "so spawn fails with *exec format error* rather than *not found*" — a specimen constructed to exercise the intended branch and not a neighbouring one, which is §2.1(c) applied to failure *modes* rather than to patterns. It then asserts `Severity::Fail` **and** that the title contains "invocation failed". That second test is the one we would have skipped: it proves the fallback did not become a blanket amnesty, i.e. a genuinely broken executable is still reported broken. It is the known-good/known-bad pair on one function — exactly the structure `path-literal-guard` lacks (§2.2).

**Adoption verdict: ADOPT WITH A NAMED GAP, and the gap contains a real tension.** Take the two-signal arm and both tests. The gap: the allowlist admits exactly one tool, so our `tmux: unknown option -- -` matches the stderr shape and **not** the allowlist, and a doctor built on this code marks tmux MISSING today. But the allowlist is not merely an oversight — it is also the **amnesty bound**, the thing that stops the fallback from waving through any tool whose probe happens to fail. Widening it to admit tmux weakens the bound by exactly that much. The resolution is not a wider pattern but a *per-tool table*: each entry names the tool, the args that are known-unsupported, the flag that does work (`tmux -V`), and the reason — the §2.8 allowance-row shape applied to version probes, so every admission is a named row with a reason rather than a broadened regex.

Four consequences. First, procedural: every figure below comes from the harness `grep`/`read` tools or an inline Python walk, and any number anywhere in this plan derived with `grep -r ... --include=` is a **false zero** until re-derived. Second: a tool that exits 0 with empty output rather than failing is exactly the never-silent-fail defect these eight gates exist to refuse — an empty scan set reported as a clean result (§2.4) — and it occurred in our own measurement path, so anti-vacuity is not only a rule we impose on gates but one our *instruments* violate. A gate whose scan is piped through such a tool reports GREEN over nothing while every leg passes. Third: presence and health are **separate claims requiring separate evidence**, and an exit status is evidence of neither on its own. Fourth, the design consequence: §4 item 5 must assert a **non-zero expected floor** on the scan set rather than mere non-emptiness, and no gate may accept an exit status as evidence of a successful probe without checking that the probe produced content.

**The irony belongs in the record verbatim.** `omp-inventory-map`'s universal `must_be_true` reads: *"The source probe is non-empty before a known verdict is emitted."* That is precisely the rule this whole subsection derives from two live instrument failures — and the census states it **183 times identically**, which is the vacuity §2.4 indicts. Right rule, vacuous application, in the same artifact.

NO-CLAIM: the leg table counts files whose *name* matches a property. A file named `mutation.rs` that mutates nothing counts here as a mutation leg. §4 specifies the meta-gate that closes that hole; it does not exist today.

---

## 2. The frameworks, one design spec each

### 2.1 Fires-on-known-bad (planted specimen)

A specimen of the exact defect class is planted into the gate's real scan set; the gate must go RED and *name* it; the specimen is removed and the gate must go GREEN — both directions in the same run.

**Why not weaker.** The weak form is "the gate is green, so the tree is clean," and that inference is invalid: a green gate is indistinguishable from one that scanned nothing, keyed on an unmatchable pattern, or was pointed at the wrong root. A gate that has never fired has zero evidence behind its claim. Unit assertions on the matcher are also weaker — they exercise the predicate without exercising *scan-set derivation*, which is where vacuity lives.

**Shape here.** MEASURED: `crates/no-shell-gate/tests/gate.rs` builds a throwaway git repo per leg (`fresh_git_tree`, 40-51), writes and `git add`s a `run.sh` so it appears in the real `git ls-files` index (`stage`, 69-77), asserts RED with the path named, then unstages and deletes and asserts GREEN — `planted_shell_is_red_then_green_after_delete` (203-228), with an independent `.py` twin (233-257) so neither extension is exercised only at unit level. The GREEN half keeps a `README.md` staged, so the clean verdict is rendered over a non-empty scan set and is a real verdict rather than vacuity.

**The staging lesson, MEASURED.** Probes staged into `git ls-files` produced `CARGO_EXIT=101` and the failure named **both** output paths — which is what proves the `.sh` and `.py` legs discriminate independently rather than one masking the other. A specimen written to disk but never staged is invisible to this gate, and the leg passes while proving nothing.

**Design spec.** (a) The specimen enters the *production* scan set — the git index here, a real file under `crates/*/src` for `path-literal-guard`. (b) The RED assertion checks the named path, not merely a nonzero exit. (c) Each forbidden pattern gets its own specimen. (d) Restore is byte-identical and verified: a planted leg that leaves the tree dirty converts one gate's evidence into every other gate's false positive. **Cost:** one `git init` per leg; `no-shell-gate` pays it four times.

### 2.2 Known-good positive control

A legitimate case, of the kind the gate must *not* flag, asserted to pass.

**Why not weaker.** An attack-only suite ships an over-strict gate, and that is a subtler death than it sounds. A gate with false positives does not get fixed; it gets *routed around* — `--no-verify`, a new exemption row, a lane that quietly stops calling it. Slower than having no gate, because the exemption looks like compliance and the coverage report still counts the gate as present. §2.8 documents this happening right now.

**Shape here.** MEASURED: `gate.rs:114-126`, `clean_list_passes`, pins Rust sources, manifests, markdown, `notes.sh.txt` (FINAL extension only, so the matcher cannot key on a substring), and a dotfile whose stem is not an extension. Those last two are the cases an over-strict extension matcher gets wrong — the boundary, not the obvious pass.

**The measured gap.** `path-literal-guard` has **known_good = 0**. It scans every `.rs` under `crates/*/src` for home-path literals and asserts zero hits (`tests/repo_wide.rs:28-38`); it has anti-vacuity (20-26) and a known-bad, but nothing pins what a *legitimate* path expression looks like. It can become arbitrarily strict with no test going RED, and it is repo-wide, so the blast radius is every crate. **`path-literal-guard` is the highest-risk gate in the set** on this measurement, despite being the smallest.

**Design spec.** Each gate declares at least one *adversarially chosen* legitimate case: the one nearest the boundary. For `path-literal-guard` that is a `repo_root()`-derived join, an `env!("CARGO_MANIFEST_DIR")` expression, and a legitimately-absolute `/tmp`-style path, all asserted to produce zero hits. **Cost:** near zero at runtime; the real cost is the judgement to pick the boundary honestly.

### 2.3 Mutation testing

Break the thing the gate keys on. The leg must go RED. Restore byte-identically and verify with a hash. A leg that stays green under mutation of its own predicate is not attributable to that predicate and proves nothing.

**Why not weaker.** Strongest single leg, and the one 4 of 8 gates lack. Known-bad proves the gate fires; mutation proves it fires *for the stated reason*. Without it a leg can pass on an unrelated assertion, an incidental exit code, or a fixture that trips a different branch — and the documented invariant is a story, not a mechanism.

**Shape here, two forms, and the difference matters.** MEASURED. *Form A — through the binary:* `crates/composer-typed/tests/mutation.rs` runs the real binary with `--mutation --disable-rule <name>`, asserts the *inverted* outcome, then runs clean and asserts the correct one, printing both. `mutation_dim_suggestion_is_not_typed` asserts `rc=0` with the rule disabled (greyed autosuggestion misclassified as typed) and `rc=1` enabled; the disable switch is a first-class production flag, so the mutation traverses the path production traverses. *Form B — through the real hook:* the production predicate was flipped in source, an active-registration `git commit` was run through the installed hook, the commit went **RED at exit 101**, and the source was restored with the sha reported on both sides. `crates/commit-build-fence/tests/hook.rs` is built for exactly this — `fresh_repo` (7-21) creates a real repo with a real baseline commit, and `run_git_with_store` (39-47) drives a real `git commit` with the fence bound via `OMP_BUILD_REGISTRATION`.

**Why through-the-real-hook is categorically stronger.** A fixture certifies the predicate. The real hook certifies the predicate, its installation, its invocation path, its exit-code contract, and git's interpretation of that exit code — five links, four of which a fixture cannot see. The measured cost of getting this wrong is §2.9: a gate whose selftest *and* mutation leg were both green against fixtures that had drifted from its real payload.

**Design spec.** (a) The mutated symbol is deliberately named for attributability — `no-shell-gate/tests/wired_lanes.rs:96` declares `const STRIP_TEST_CODE: bool = true` with the comment "deliberately named so its mutation is attributable." (b) Prefer a production `--disable-rule`-style switch to a source edit, because it is reversible by construction. (c) Where source must be edited, record the sha before and after and report both. (d) Run through the real invocation surface whenever one exists. **Cost:** highest of the four legs — `git init` plus hook install plus a real commit per leg, and the source-edit form needs restore discipline.

### 2.4 Anti-vacuity — including our own violation

An empty or unreadable scan set is an ERROR, never a pass. A deliverable that was never checked must never report like one that passed.

**Why not weaker.** Highest-yield property in the set, because it is the failure that makes every other leg lie. `no-shell-gate` carries it at three levels — unit (`empty_scan_set_is_an_error_not_a_pass`), end-to-end (`empty_index_is_an_error_not_a_pass`), and CLI exit code (`binary_exits_2_on_empty_index`: exit **2** for gate-error, distinct from **1** for violation-found). `path-literal-guard/tests/repo_wide.rs:14` states the principle — "a verdict without its coverage is unauditable" — and prints the whole scan set beside the verdict.

**THE SELF-INDICTMENT.** MEASURED (`00-brief.md` §3.3):

```
python3 -c "…Counter(json.dumps(r.get('must_be_true')) for r in rows)…"
  crate rows:     n=26   distinct must_be_true=1  distinct negative_evidence=1
  non-crate rows: n=157  distinct must_be_true=1  distinct negative_evidence=1
```

All 183 census rows carry the four mandatory fields with **zero missing** — and exactly **one distinct** `must_be_true` and **one distinct** `negative_evidence` across the entire census. The universal invariant is `["The source probe is non-empty before a known verdict is emitted.","A versioned inventory envelope carries the probe state."]` — an invariant about *the scanner*, not about any of the 183 things scanned. For the 26 crate rows, `inputs`/`outputs` describe the scanner's provenance (`cargo metadata --format-version 1 --no-deps` plus the crate's `Cargo.toml`), not the crate's contract, and `what_it_provides` is boilerplate — "Workspace crate X from cargo metadata" — distinct only because the name varies.

The four-field discipline is satisfied **syntactically and vacuously**: the exact defect anti-vacuity exists to catch, committed by the instrument that enforces anti-vacuity elsewhere, indicting the conductor rather than a worker. We publish it in our own section because an investor who finds it unaided has found a gap we did not see.

**Design spec for the fix.** A distinct-invariant-count check: for a census of `n > 1` rows, if `distinct(must_be_true) == 1` or `distinct(negative_evidence) == 1`, FAIL with `VACUOUS_INVARIANT_SET`, naming the repeated value. Two refinements the naive version needs: (a) partition by row-kind, so a legitimately homogeneous kind is not forced to fabricate variation — the measurement above already partitions crate from non-crate rows and both violate independently; (b) reject an invariant that names the *scanner* instead of the row's subject, enforceable as "a crate row's `must_be_true` must mention that crate's identifier."

**What would Jeffrey do — and a retraction, because this section published a false zero.** An earlier draft stated: `searched vacuous|vacuity in asupersync/docs, asupersync/AGENTS.md, aadc/AGENTS.md — no matches`, and concluded "the *word* is absent from the mirror." **REFUTED.** Re-derived with the harness `grep`, no extension filter, across the whole `asupersync` repo rather than three hand-picked files: `vacuous|vacuity|vacuously` matches **36+ files in that one repo**. The search space was the defect — three files chosen because I expected doctrine to live in docs, when the concept lives in test code, production telemetry, a shell gate, and a Lean proof. This is the same false-zero class as the `--include=` grep (§1) with a different mechanism: not a filter that matched nothing, but a *search space too narrow to contain the answer*. A not-found is only publishable if the search space is justified, not merely stated.

The prior art is richer than the design in this subsection, on four counts.

**(1) Anti-vacuity guards live INSIDE the metamorphic relation.** `src/runtime/scheduler/metamorphic_tests.rs:440-441` asserts, before checking its relation: *"MR4 VIOLATION: zero cancel dispatches across {} injected cancel tasks — streak-bound assertion would be vacuous"*; `:517-522` repeats it for MR5 labelled "ABSOLUTE-CORRECTNESS ANCHOR", and `:661-662` for MR7 — *"timed-lane dispatched {} of {} injected tasks — EDF ordering check would be vacuous on the missing tasks."* The relation does not merely hold; the test first proves the workload was actually exercised. **This is the leg that would have caught `pane-truth`** (§2.9): a fixture-format drift means zero live payloads were classified, and a guard asserting "at least N inputs reached the classifier" goes RED where a green relation says nothing.

**(2) Vacuity can be a typed state rather than a failure.** `src/messaging/jetstream.rs:2460` serialises `waiter_fairness_mode: "vacuous_zero_wait_refusal"` into production telemetry, and `src/messaging/jetstream_flow_control_audit.rs:6` explains it: *"for the current controller, fairness is vacuous because hidden waiters are impossible (`max_waiters = 0`)."* The system does not claim fairness and does not fail; it **names its own vacuity as a first-class value**, and `scripts/run_jetstream_publish_backpressure_smoke.sh:181-186` gates on that exact string. This is strictly better than the binary FAIL this subsection specified, for the case where vacuity is *legitimate and permanent*: a trivially-satisfied invariant carried forward under a name a later reader cannot mistake for evidence. Our `VACUOUS_INVARIANT_SET` is the right response to *accidental* vacuity; this is the right response to *structural* vacuity, and we had no vocabulary for the second.

**(3) A positive control is justified as anti-vacuity, fusing §2.2 and §2.4.** `tests/atp_rq_observability_metrics.rs:134-135`: *"Positive control: the manifest DOES carry its content-descriptor fields, so the negative assertions above are meaningful (not vacuous on an empty blob)."* The known-good leg exists *because* the attack leg would otherwise be vacuous — one mechanism, not two. Same shape at `src/stream/buffered.rs:1238` ("two empty or two all-identical sequences would compare equal and prove nothing"), `src/runtime/builder.rs:7363-7364` ("an empty fingerprint would make the equality below vacuous"), and `tests/three_lane_tests.rs:7593-7594` ("a pass here means the worker never touched the shard — vacuous").

**(4) Anti-vacuity is pushed into the type system and the API return value.** `src/trace/tla_export.rs:111-113` declares `pub type EntityKey = (u32, u32)` precisely so slot-reuse aliasing cannot make invariants "pass *vacuously* because one entity silently overwrote the other in the map." And `src/combinator/map_reduce.rs:142-143` has `all_succeeded()` return **false** on empty input "even though the aggregate decision is `AllOk` (vacuously true)" — refusing to let a caller read vacuous truth as success, with the test at `:728-729`. Measured yield of the discipline: `CHANGELOG.md:1077-1078` records six HTTP/1.1 RFC 9112 tests "that previously **passed vacuously** when codec validation was missing," and `audit_index.jsonl:3251` a metamorphic MR2 that "**was vacuous** because it toggled an unrelated testing Cx instead of the inspected runtime state" — our exact defect, found by audit, in his repo.

**Adopted.** (i) Every metamorphic relation in §2.6 ships an input-reached guard before its relation. (ii) The `VACUOUS_INVARIANT_SET` check gains a sibling: a **named structural-vacuity mode** carried in the envelope for invariants that are trivially satisfied by construction, gated on the name. (iii) Known-good legs are documented as anti-vacuity mechanisms for their paired attack legs, not as independent niceties.

### 2.5 Differential / oracle testing

Two independent implementations of the same judgement are compared. An absent, empty, or unreadable oracle is an ERROR or an announced SKIP — never silent agreement.

**Why not weaker.** For a judgement with a large input space and no compact specification, a second independent implementation is a far denser oracle than a hand-written expectation table — and the failure mode of tables is measured. `crates/composer-typed/tests/differential.rs:26-30` records that `frankenscipy-ivg5` audited 12 conformance runners and **11 invoked no oracle at all**, comparing against hand-typed `case.expected` fields while still populating an `oracle_status` field, so the report looked differential while nothing differential ran.

**Shape here.** MEASURED. `composer-typed/tests/differential.rs` compares against `bin/composer-typed.py` under the house rule quoted at line 3: "python and shell are only allowed to use for comparisons, all gated oracles should be rust." It is a comparison, not a gate, and is *forbidden* to fail the suite when the oracle is absent — because `bin/composer-typed.py` can never exist here (the no-`.sh`/no-`.py` gate's exemption list is empty). Before that change both tests FAILED with a uniform `python=2` across all 8 cases — python3 failing to open a nonexistent script, read as 8 semantic disagreements — which made an absent external tool the authority over a green Rust suite and turned `cargo test --workspace` red, burying the no-shell gate's own signal. The absence path is now a **typed** `OracleStatus` with a LOUD `announce_skip` (69-83), modelled on `franken_whisper/src/differential_oracle.rs:1-6` and its `DifferentialSkipReason::MissingExecutable`. `oracle-compare`'s stated role (`AGENTS.md:485`) is the same invariant in one line: "An empty or unreadable oracle must be an ERROR, never a silent agreement."

**Design spec.** The shell implementation remains the differential oracle for `pane-truth` while the Rust port is proven. Both are fed the same captured pane bytes; disagreement is a FINDING, not an automatic Rust defect, because §2.9 proves the shell side can be the wrong one. Oracle absence yields a typed skip that announces DID NOT RUN and is never counted as a passing comparison. The oracle is never a Cargo or runtime dependency. When the Rust port's four legs are green against *live* captures, the oracle is retired and the retirement recorded with the date and the leg set that replaced it.

**What would Jeffrey do.** `aadc/.beads/issues.jsonl:91` (bd-fbf) states the discipline exactly: "The Rust implementation is the only authority on what 'correct' means. A second independent implementation (deliberately slow, deliberately readable) catches regressions in the Rust implementation that pass its own tests." *Deliberately slow and readable* is a constraint we had not written down; it is now.

### 2.6 Metamorphic testing

Where the correct output is unknown, assert a *relation* between inputs and outputs that must hold regardless: idempotence, invariance under an irrelevant transformation, monotonicity under an ordered one.

**Why not weaker.** Pane classification has no ground-truth label. A captured status line is Working or Idle because the model rendered it so; there is no authority to consult. Expectation tables in this domain are precisely the artifact §2.9 proves drifts silently. A metamorphic relation survives that drift because it never names an expected output.

**Status.** PROJECTED for the design; MEASURED for the absence, with the search space stated because §1 and §2.4 both prove a bare zero is not a finding. Harness `grep` for `metamorphic`, no extension filter, over `crates`, `docs`, `AGENTS.md`, and `WAVE.md`: **zero hits under `crates/`**. The only hits in the repo are in `docs/plan/`, and they are this session's own plan text — this section and its siblings — not implementation. The search space is right because `crates/` is the entire workspace (root manifest is `members = ["crates/*"]`, a glob), so no implementation can exist outside it.

**What would Jeffrey do.** Extensive prior art, and the strongest argument in this section for adopting a framework we lack. `asupersync/CHANGELOG.md:1260-1261` reports a hardening pass with "**25 real production-code bugs fixed** (most of them pre-existing, surfaced by the recently-expanded metamorphic test suite)" — measured bug yield, not a style preference. `:1345` describes "hundreds of metamorphic relations" across the runtime, scheduler, obligation ledger, and RaptorQ; `:1029` names a "**Restart-budget metamorphic oracle**." `aadc/.beads/issues.jsonl:73` (bd-b7g) names three concrete relations for a text-alignment algorithm — block-order permutation, tab-width round-trip, whitespace append — with the rationale "Especially valuable for an algorithm where the 'correct' answer is hard to specify but transformations should commute," which describes pane classification exactly. `asupersync/ATP_DOD_CHECKLIST.md:19-21` makes it a done-condition with two fields that must be filled — `Command:` and `Properties tested:` — so a metamorphic claim cannot be made without naming its command. We adopt that field pair in §4.

**Design spec.** Three relations over `tick-monitor::classify`. **MR-1 (invariance):** prefixing/suffixing whitespace or re-wrapping must not change the classification — a pane does not become idle because tmux padded it. **MR-2 (monotonicity):** for a captured Working line, increasing the rendered timer must yield a strictly larger `timer_secs` and must never flip the variant. **MR-3 (non-interference):** inserting a token-budget or spend counter must not change the classification — the generalisation of the measured leg `token_budgets_and_spend_counters_are_not_elapsed_timers` (`tests/monitor.rs:45-57`), which pins two specific counters where MR-3 pins the class. Each ships with `Command:` and `Properties tested:` filled per the ATP checklist. A relation marked `#[ignore]` must carry a reason, as `asupersync/CHANGELOG.md:1460-1463` does when the lab scheduler cannot expose a policy.

**And each relation opens with an input-reached guard, adopted from `metamorphic_tests.rs:440-441` (§2.4).** Before asserting its relation, every MR asserts that a minimum number of live captures actually reached `classify` and produced a non-`Unknown` variant — the shape `assert!(classified >= N, "MR-k VIOLATION: {classified} of {offered} captures classified — relation would be vacuous on the rest")`. This is the single most valuable line in the whole design, because it is the leg `pane-truth` lacked: its fixtures were the wrong format, so zero live payloads were classified, and both its selftest and its mutation leg passed on an empty effective input set (§2.9). A relation that holds over nothing holds. The guard converts that silent pass into a named RED.

### 2.7 Golden artifacts / schema pinning

The output envelope is frozen; a shape change fails the build until the change is deliberate and versioned.

**Why not weaker.** Every consumer of a machine-readable envelope — CI, another crate, a foreign repo, an agent — is coupled to its shape. Without a pin, a field rename is a silent breaking change discovered as a mysterious parse failure in someone else's build. Per-field assertions are weaker: they cannot see a *removed* field or a changed `status` vocabulary.

**Shape here.** MEASURED: the envelope is `{"schema_version":"omp-inventory-map/v1","command":"doctor","status":"UNKNOWN","data":{…}}`, 544,697 bytes, exit 2. `SCHEMA_VERSION` is one constant threaded through every emission site in `crates/omp-inventory-map/src/main.rs` (9 sites, 80-212) and `src/lib.rs:613,1366`. `commit-build-fence` carries the stronger form: a numeric `schema_version` **validated on read** (`src/lib.rs:208-213`), so a store written by an older binary is rejected with observed and expected versions named rather than misparsed.

**Design spec for `omp-inventory-map/v1`.** (a) Commit a golden envelope with `data` elided to its key set; the test compares key sets and the `status` vocabulary, not the 544 KB payload — a golden that changes on every scan gets regenerated reflexively and pins nothing. (b) Assert the four mandatory row fields present on every row (passes today) *and* diverse per §2.4 (does not). (c) `status` is a closed vocabulary; an unrecognised value is a parse ERROR. (d) A `schema_version` bump requires the golden regenerated in the same commit, and the test keys on the version string, so bumping without regenerating is RED. (e) Adopt `commit-build-fence`'s validate-on-read: consumers reject a foreign version loudly instead of best-effort parsing it.

**(f) The count-twin invariant, and it is the load-bearing one.** The envelope carries paired counts — an observed count and an `expected_*` twin. MEASURED: six of the seven pairs match exactly, and one does not — `slash_commands=0` against `expected_slash_commands=136`. That single mismatch is why the envelope reports `status: UNKNOWN` and the binary exits 2, and it is the largest unmapped region of the OMP surface. The pin therefore asserts a *conditional*: for every twin pair, either observed equals expected, **or** `status` is `UNKNOWN` and the specific mismatching pair is named in the output. Both halves matter. Without the first, a scanner that silently drops to zero on every kind still reports a clean envelope. Without the second, `UNKNOWN` becomes a blanket amnesty that lets any number of new mismatches hide behind one already-known gap — which is exactly what `slash_commands` does today: the status is honest, and the envelope does not name which twin broke it. A gate that reports UNKNOWN without naming its own unknown is unactionable, and 136 unmapped surfaces is too large a hole to leave addressed only by a status string. **Cost:** one golden file; one regeneration step per deliberate schema change.

### 2.8 Conformance harness with an explicit allowance list — and a live RED

Every member of a *derived* set must satisfy a property. Exceptions live in a declared allowance list where each row names the member **and** a reason. The validator refuses a row with no reason, and refuses a row naming a member absent from the derived set.

**Why not weaker.** The weak form is a hand-listed expectation set, and `no-shell-gate/tests/wired_lanes.rs:43-49` names the prior art it avoids: control-plane's `check.sh` hand-lists `EXPECTED_GATES` while the verdict claims completeness, so "the list drifts and the suite reports vacuously green while most lanes are unexamined." Here the set is derived from disk (`derive_lanes`, 57-93; `workspace_crate_names`, 572-588) so a new crate is in scope the moment it exists, and an empty or unreadable derivation is an ERROR (`empty_scan_sets_are_errors_not_passes`, 473-483).

**Shape here.** MEASURED. The `UNWIRED_LANE_ALLOWANCE` pattern, taken from `franken_lean`. `wired_lanes.rs` carries four independent legs, each owning one predicate, one scan, one allowance, one validator — "Mutating one predicate must leave the other three green: no shared scan, no shared helper beyond `workspace_crate_names` (a pure directory read)" (565-570). Two allowances are **empty by construction**: `SURFACE_ALLOWANCE` (607) and `FORBID_ALLOWANCE` (647). The validators are `every_allowance_row_names_a_lane_and_carries_a_reason` (486-506) and `validate_allowance_rows` (590-604) — the latter requires a reason of **≥ 8 characters**, so a one-character reason is refused too. The maintenance contract is load-bearing: rows are checked against the DERIVED set every run, and stale rows are refused with "allowance names undeclared lane …", which **fired live** when extraction removed two members mid-grade. The harness caught the `installer` lane; **the RED was the pass** — a harness green on first run would have told us nothing.

**A LIVE RED, found writing this section.** MEASURED. Leg 3, `every_crate_declares_the_forbid_lint` (649-672), iterates the derived set (all 26 `crates/*` dirs holding a `Cargo.toml`; the root manifest is `members = ["crates/*"]`, a glob) and requires each `Cargo.toml` to satisfy `text.contains("unsafe_code") && text.contains("forbid")`, with `FORBID_ALLOWANCE` empty. Measured with an inline Python walk of `crates/*/Cargo.toml` and `crates/*/src/{lib,main}.rs`:

```
crate dirs            : 26
manifest lint present : 20
manifest lint MISSING : composer-typed, dispatch-silence-watch, loop-queue-filter,
                        no-shell-gate, pane-dispatch-fence, subprocess-contract
all src roots forbid  : 25   (missing: tick-monitor)
```

Six crates fail the predicate with no allowance available, so `missing` is non-empty and the assertion cannot hold. PROJECTED-BY-INSPECTION, not observed: we did not run the test, so the RED is derived from reading the predicate and measuring its inputs.

Three things follow, and they are the point of the finding. **First**, all six carry `#![forbid(unsafe_code)]` in every source root (harness `grep`, confirmed per crate) — the *invariant* holds while the *predicate* fails. This is a false positive of exactly the kind §2.2 says gets a gate routed around, in the gate with the best leg coverage in the repo. **Second**, the two mechanisms are near-complements: `tick-monitor` is covered by manifest but not by source attribute; the other six by attribute but not manifest. Union **26 of 26**, intersection **19 of 26** — every crate forbids unsafe by at least one mechanism, and neither single-mechanism predicate can see it. Leg 3 measures the *declaration site*, not the property. **Third**, the predicate is simultaneously over- and under-strict: an unordered substring conjunction over the whole manifest means `# TODO: forbid unsafe_code someday` in a comment passes. An empty allowance list made it look maximally strict; it is satisfiable by a comment.

**Disagreement with the brief, stated plainly.** `00-brief.md` §3.7 records "16 of 22 forbid unsafe." We measure 26 crate dirs, 20 of 26 manifests, 25 of 26 source roots, 26 of 26 by union. We do not think the brief is wrong so much as differently scoped — a third denominator over a different file set — but three live denominators for one property is itself the defect writing-contract rule 4 exists to prevent, and the property should be reported as `26/26 by union (19/26 by both mechanisms)` going forward.

**Design spec.** (a) Derive the set; never hand-list. (b) Empty derivation is an ERROR. (c) Every row: member + reason + the condition under which the row dies — `ALLOWED_COLLISIONS` (`omp-inventory-map/src/types_inventory.rs:180-206`) already states "Dies when …" per row. (d) A row naming an undeclared member is an ERROR. (e) A member naming itself is not a caller (`a_lane_naming_itself_is_not_wired`, 519-533). (f) Comments and test-only code are not evidence of wiring (`comments_and_test_only_code_do_not_prove_wiring`, 456-471). (g) **New, from the RED:** a conformance predicate targets the *property*, not one declaration site — leg 3 becomes "forbids unsafe by manifest lint OR source attribute," and gains the known-good leg it never had, pinning a crate that conforms by attribute only. (h) Substring conjunctions over a whole file are forbidden; parse the manifest and read `[lints.rust] unsafe_code`.

**What would Jeffrey do.** Two upgrades to the row shape. `asupersync/conformance/artifacts/conformance_registry_contract_v1.json:125-134` carries four-field rows — `disposition`, `superseded_by`, `reason`, and `retention_reason` ("File deletion is forbidden; retained for future metamorphic repair") — where ours carries two. And `asupersync/.github/no_mock_policy.json` runs the same pattern at repo scale with `pattern`, `category`, and `owner`. We adopt `owner` and `dies_when` as required fields: an allowance row with no owner is an orphan, and our five current `UNWIRED_LANE_ALLOWANCE` rows (20-41) share one reason and one landing bead with no named owner between them.

**A row that must be restated as BLOCKED, not pending.** `ALLOWED_COLLISIONS` deliberately omits `Observation` (`types_inventory.rs:176-179`) so the gate REFUSES that collision "until the convergence lands," and the convergence would be reached by adopting the shared vocabulary `omp-types` re-exports. MEASURED, and it changes the row's meaning: `ObligationLedger` occurs **zero** times in `omp-types`, and `AckKind`/`DeliveryClass` appear only in a doc comment naming them as blocked. They sit behind `#[cfg(feature = "messaging-fabric")]`, which transitively requires `test-internals`, which upstream issue #46 correctly removed from default features. So the half of the vocabulary that would collapse our 17 ack/receipt types across 3 incompatible dialects is **unreachable at our pinned rev** — blocked at an upstream feature boundary, not merely unadopted and not pending our work.

A refusal whose remedy is unreachable is not a decision awaiting execution; it is an indefinite RED with no owner and no landing condition. The row must therefore state the boundary: refused, blocked by `messaging-fabric` → `test-internals` at asupersync rev `fa3c01aec` (upstream #46), `dies_when` the feature boundary moves or we vendor the vocabulary locally, `owner` unassigned. Writing it as "until the convergence lands" implies the convergence is ours to schedule, which is the same shape as a close reason with no evidence: a sentence that reads like a plan and commits no one to anything.

**Cost.** The source-stripping machinery (`strip_comments`, `strip_test_code`, `brace_delta`) is ~100 lines of careful parsing and is the part most likely to harbour its own bug.

### 2.9 The floor-raise claim discipline

A gate header saying "guarantees", "proves", or "makes impossible" is itself a defect, because a reader who believes it stops looking. Each gate states what it mechanically enforces **and** what still passes.

**Why not weaker.** Not documentation hygiene — the control on the most expensive failure we have measured. MEASURED: `pane-truth`'s fixtures were the Claude Code status format (`Working (2s - esc to interrupt)`), so **its green selftest AND its mutation leg were both vacuous** against the payload it actually runs on (`crates/tick-monitor/tests/monitor.rs:15-19`), and it reported `liveness_two_capture: false` on every pane for exactly that reason. A complete-looking four-leg gate certified nothing. A "guarantees" header would have been actively harmful, redirecting attention from the only question that mattered.

**Shape here.** MEASURED: `no-shell-gate/tests/gate.rs:18` states its NO-CLAIM inline — "extensions of tracked files only." `wired_lanes.rs:7-8`: "This suite proves reachability only: a caller can invoke a lane while the invoked mode may still be weaker than the lane's live guarantee." `tick-monitor/tests/monitor.rs:15` states its fixtures are "VERBATIM captures from live panes on 2026-08-31, not hand-written approximations" — a claim about *provenance*, which is the direct remediation of the pane-truth defect.

**Design spec.** Every gate header carries three fields: **ENFORCES** (the mechanical predicate in the gate's own vocabulary), **STILL PASSES** (the nearest defect it does not catch), **PROVENANCE** (where the fixtures came from; live capture or constructed, with a date). The §4 meta-gate rejects a header containing "guarantee", "prove", "impossible", or "cannot happen" outside a NO-CLAIM sentence. **Cost:** prose discipline, and the restraint not to overstate a gate you just built.

---

## 3. The six required properties

1. **FIRES-ON-KNOWN-BAD** — a planted specimen in the real scan set turns it RED and names the specimen.
2. **KNOWN-GOOD** — an adversarially chosen legitimate case passes.
3. **MUTATION** — breaking the keyed predicate turns the leg RED; restore byte-identical and hash-verified; through the real invocation surface where one exists.
4. **ANTI-VACUITY** — an empty or unreadable scan set is an ERROR with a distinct exit code; and the invariant set is non-degenerate (§2.4).
5. **FLOOR-RAISE CLAIM** — ENFORCES / STILL PASSES / PROVENANCE; no "guarantees".
6. **ADDRESSABLE** — one documented command runs the gate, and `--help` names that command.

### 3.1 The six-by-eight coverage matrix

Legs 1-4 are transcribed from §1. Property 5 is MEASURED here: `grep` for
`NO-CLAIM|STILL PASSES|proves .{0,20}only|does not (catch|prove|claim)` across the eight gate
crates returns headers in exactly three of them — `no-shell-gate/src/lib.rs:11`
("WHAT STILL PASSES — do not read this gate as more than it is"),
`omp-inventory-map/src/types_inventory.rs:16` ("this proves SHAPE, not SEMANTICS"), and
`path-literal-guard/src/lib.rs:16` ("WHAT STILL PASSES"). Property 6 is UNMEASURED for seven
of eight, because establishing it requires running each binary, which this section is
forbidden to do; the one measured value is `omp-inventory-map`, which FAILS.

| gate | 1 known-bad | 2 known-good | 3 mutation | 4 anti-vacuity | 5 claim | 6 addressable |
|---|:--:|:--:|:--:|:--:|:--:|:--:|
| `no-shell-gate` | Y | Y | Y | Y | **Y** | — |
| `omp-inventory-map` | N | Y | Y | Y | **Y** | **N** |
| `undrained-pipe-lint` | Y | Y | Y | Y | **N** | — |
| `commit-build-fence` | N | Y | N | N | **N** | — |
| `state-wildcard-lint` | Y | Y | Y | N | **N** | — |
| `kernel-bypass-gate` | Y | Y | N | N | **N** | — |
| `pre-delete-citation-check` | Y | Y | N | N | **N** | — |
| `path-literal-guard` | Y | **N** | N | Y | **Y** | — |

**Zero gates satisfy all six.** Three set relations read against expectation and are the
useful part of the matrix. First, `path-literal-guard` has the *best* claim discipline in the
set and the *only* missing known-good — the gate most honest about what it does not catch is
the one least protected against catching too much. Second, three of the four gates with no
mutation leg also have no claim discipline (`commit-build-fence`, `kernel-bypass-gate`,
`pre-delete-citation-check`); the fourth is `path-literal-guard`, which has the best claim
discipline of any gate. Third, and tightest: the four gates missing anti-vacuity
(`commit-build-fence`, `state-wildcard-lint`, `kernel-bypass-gate`,
`pre-delete-citation-check`) are a **strict subset** of the five missing claim discipline —
`undrained-pipe-lint` is the only gate that carries anti-vacuity while claiming more than it
enforces. PROJECTED: the co-occurrence has one cause — a gate written to close a specific
incident, shipped the moment it went red on that incident, and never revisited to ask what
else it now claims or what it would report against an empty scan set.

NO-CLAIM: column 6 is seven-eighths unmeasured, so this matrix understates nothing and may
overstate the addressability of every gate but `omp-inventory-map`.

**The defect that produced the sixth.** MEASURED: `omp-inventory-map --help` returns `{"schema_version":"omp-inventory-map/v1","command":"doctor","status":"ERROR","data":null,"error":"CONFIG_ERROR unknown argument --help"}`. The gate is built and correct — 13 tests pass, and `src/types_inventory.rs:176-179` deliberately excludes `Observation` from the allowance list so the collision demands convergence rather than being waved through. And the running binary's 544 KB doctor output contains **zero** occurrences of `Observation`, `CONVERGE`, or `Verdict`. This is not built-versus-wired; it is **wired-but-unaddressable** — the gate runs, and nothing it emits or documents tells an operator or an agent that it exists or how to reach its verdict. A correct, well-tested, unreachable gate has the same effect on the tree as no gate.

MEASURED, with the search space named: harness `grep` for `ADDRESSABLE`, no extension filter, over `crates`, `docs`, `AGENTS.md`, `WAVE.md` — **zero hits under `crates/`**; every hit is in `docs/plan/`, i.e. this session's plan text written by this section and its siblings. `crates/` is the whole workspace (`members = ["crates/*"]`), so the property exists in prose and in no code. PROJECTED: that it will be enforced.

**What would Jeffrey do.** ADDRESSABLE is a named bead class in the mirror. `aadc/.beads/issues.jsonl:145` (bd-zku) is "Document undocumented CLI flags and subcommands in README — Five subsystems are implemented in src/main.rs but absent from the README CLI reference table," which is our defect class filed as work. `:133` (bd-u01) mechanises it: `check-readme-claims` asserts "Every flag in 'aadc --help' is in README CLI table," plus exit codes, default values, and short forms. `:68` (bd-abk) states the ambition — a custom `long_help` per flag turning `-h` into "a discoverable mini-tutorial." And `:128` (bd-r6h) supplies the reporting shape we adopt in §4: a definition-of-done matrix where each row is **PASS / FAIL / SKIP with an evidence path** — §2.5's typed skip applied to release readiness.

---

## 4. Design spec: how a new gate is admitted

Future tense. A gate will not be permitted to fail anyone's build until it satisfies all six properties, and the enforcement will not be a review convention. The admission checklist a candidate gate will satisfy:

1. It declares its scan-set derivation, and the derivation reads the world (git index, directory tree, `cargo metadata`) rather than a hand-listed constant.
2. It carries a planted known-bad in that real scan set: RED-with-name, then GREEN, with byte-identical restore.
3. It carries a known-good boundary case, chosen adversarially.
4. It carries a mutation leg with a deliberately named mutation point, run through the real invocation surface if one exists.
5. It carries anti-vacuity at every level it has — unit, end-to-end, CLI — with gate-error distinguished from violation-found by exit code (the `no-shell-gate` 2-versus-1 convention). Non-emptiness is insufficient: the gate asserts a **non-zero expected floor** on its scan set (e.g. "at least 26 crates," "at least one tracked file per declared lane"), because §1's silently-empty tool proves a scan can collapse to a plausible-looking small number without erroring. A floor turns "I scanned something" into a falsifiable claim.
6. Its predicate targets the property, not one declaration site, and never by substring conjunction over a whole file (§2.8g/h).
7. Its header carries ENFORCES / STILL PASSES / PROVENANCE and no unqualified "guarantee".
8. `--help` runs, exits 0, and names the command that runs the gate.
9. It is reachable from `cargo test` — not only from a `main.rs` subcommand a human must remember. MEASURED precedent: `tick-monitor`'s binary `--selftest` has 41 legs and `cargo test` reaches none of them, so its invariants were moved to where the suite executes them (`tests/monitor.rs:3-7`).
10. Every allowance row carries member, reason (≥ 8 chars), `dies_when`, and `owner`.
11. Each claimed framework fills the ATP field pair: `Command:` and `Properties tested:`.
12. It never reads an exit status as evidence of a successful probe. Presence and health are separate claims requiring separate evidence — a path resolution establishes presence, a content check establishes health, and a non-zero status establishes neither absence nor ill-health on its own. Precedent, its tests, and its gap: `pi_agent_rust/src/doctor.rs:967-968` (the two-signal fallback arm), pinned by `:13948` `check_tool_falls_back_when_probe_args_are_unsupported` and `:13964` `check_tool_reports_invocation_failure_for_broken_executable` (§1).
13. Every zero it reports — "no violations", "no prior art", "no occurrences" — carries the exact command **and** the justification that its search space could have contained the answer. A bare zero is not a finding. MEASURED cost of omitting this: two false zeros in this section alone, one from a `--include=` filter that silently matched nothing (§1) and one from a search space of three hand-picked files when the answer lived in 36 (§2.4).
14. Every citation it makes — to a precedent, a test, or a source line — names the **construct**, not just the line. MEASURED cost of omitting this: three of us cited one precedent at three different line numbers and all three were partly right, because each named a different construct on adjacent lines (§1). A line number is unverifiable alone and does not survive a reformat; a named construct can be re-found.

Reporting: one row per gate, **PASS / FAIL / SKIP with an evidence path**, per `aadc` bd-r6h. A SKIP with no evidence path is a FAIL.

The meta-gate enforcing it will be a conformance harness of exactly the §2.8 shape, whose derived set is *the gate crates themselves* and whose properties are items 1-14. `GATE_ADMISSION_ALLOWANCE` starts empty; every exception is a named crate with reason, `dies_when`, and `owner`; a row naming a non-gate crate is an ERROR. Its own known-bad is a planted gate crate missing one leg, refused by name; its mutation leg disables one property check and asserts the harness goes green, proving each check is independently attributable. It closes §1's NO-CLAIM by checking *behaviour* — does the mutation file actually invert an outcome — rather than a filename. Item 13 it checks on itself: the harness's own "zero non-conformant gates" verdict must carry its derivation and the argument that `crates/*` is the complete search space.

Ordering, and it is not cosmetic: the meta-gate is admitted **last**, after at least one gate satisfies all *six* properties — the four-leg bar is already met by two gates (§1) and is not the binding constraint; the binding constraint is that **zero** gates clear property 5 and property 6 together (§3.1). A meta-gate whose derived set is entirely non-conformant produces an eight-row allowance list on day one, and an eight-row allowance list is indistinguishable from no gate.

---

## 5. What would make this whole framework fail

**Gates that are green because they scan nothing.** The likeliest failure, already committed at census level (§2.4). Anti-vacuity is present in only 4 of 8 gates; `commit-build-fence`, `state-wildcard-lint`, `kernel-bypass-gate`, and `pre-delete-citation-check` each show `anti_vacuity = 0` and will report identically whether they passed or scanned an empty set. The countermeasure is mechanical and cheap and is not there.

**A mutation leg that is not attributable.** If the mutation point is not deliberately named, a refactor moves it and the leg keeps passing for a different reason. Worse, a mutation leg run against a fixture can be green while the production predicate is unreachable — measured in `pane-truth`.

**A fixture drifted from production, certifying nothing.** MEASURED, and the most expensive, because it defeats all four legs simultaneously. No amount of leg completeness detects it; only PROVENANCE does — fixtures must be verbatim live captures with a date, which `tick-monitor` now satisfies and nothing else in the repo is required to.

**An over-strict predicate routing a gate around.** No longer hypothetical: §2.8 reports leg 3 failing six conforming crates. The pressure that follows a false positive is to add allowance rows, and an allowance row is indistinguishable from compliance in every summary we produce.

**The allowance list as a pressure valve.** `UNWIRED_LANE_ALLOWANCE` has five rows sharing one reason and one landing bead. If that bead slips, the honest response is that the rows are stale; the dishonest response is to edit the reason. The maintenance contract catches a row naming an *undeclared* lane. It does not catch a row whose reason has quietly become false. That is an open hole in the strongest pattern in this section, and `owner` + `dies_when` (§2.8) narrow it without closing it.

**Gating discipline concentrated where the product is not.** The sharpest structural objection. Per `00-brief.md` §4, exactly one of five pipeline layers works: `observe` WORKS, `actionable` is BROKEN, `consume` is FENCED (162 refused ticks over 4.2 hours), and both `actuate` and `complete` DO NOT EXIST. Eight gates and 370 tests guard a pipeline that cannot yet dispatch or complete a unit of work. An investor is entitled to ask whether the gate budget bought defect prevention or the appearance of rigour. Our answer, and it is a partial concession: the gates encode the specific defect classes that consumed this session — vacuous verdicts, silent oracles, unattributable legs, unaddressable binaries — and those are the classes that will destroy the four missing layers as they land. But the ordering risk is real, and the mitigation is that no new gate is admitted (§4) until it clears a bar the existing eight mostly do not.

**370 tests as a proxy metric.** If the count becomes the target, the count will rise and the leg table will not. The only numbers worth reporting are the leg table and the §3.1 matrix, and the only honest headlines today are **2 of 8** on four legs and **0 of 8** on six properties.

**A transcribed headline nobody recomputes.** Demonstrated in this section: the brief's own summary line disagreed with the table directly above it on two of four counts, and the first draft of this section reproduced both errors while citing the correct table (§1). Every derived count in this plan is one transcription away from being wrong in the same way, and prose review does not catch it. The countermeasure is that a count in prose must carry the expression that computes it from the table, not the table's caption.

**A zero that was never a measurement.** The most dangerous failure in this section, because it is indistinguishable from a real result and it defeats the mirror-mining requirement outright. Two instances here, with two different mechanisms: a shell `grep -r --include=` that returns empty at exit 0 (§1), and a search space of three hand-picked files when the concept lived in 36 (§2.4). A third, from a sibling, filtered `--include='*.rs'` across a Go repository and read structural absence as semantic absence. All three produce a confident "no prior art found" that reads exactly like a true one, and in every case the correction came from someone re-deriving rather than reading. A framework built on prior art cannot tolerate a false zero, because the false zero is precisely the result that stops the search — and unlike a wrong number, nothing downstream contradicts it. §4 item 13 is the countermeasure, and it is weaker than the disease: it makes the search space auditable but cannot make it complete.

---

**NO-CLAIM.** This section describes the frameworks and their measured coverage as of 2026-08-31. It does not claim the eight gates are sufficient to prevent the defect classes they name; it does not claim the 370 tests are individually load-bearing. Two gates — `no-shell-gate` and `undrained-pipe-lint` — have all four legs; **none** of the eight satisfies all six properties, because ADDRESSABLE and the floor-raise claim discipline exist in this document and in no validator. Column 6 of §3.1 is measured for one gate of eight.

**Retractions this section carries rather than deletes**, because each is more instructive than the corrected value: (1) the leg-count headlines "1 of 8" / "5 of 8", transcribed from the brief and refuted by the brief's own table (§1); (2) `tmux --version` "exits 0 while failing", refuted — tmux exits 1 correctly, and the defect was our probe reading `$?` after a pipeline, `PIPESTATUS=(1 0)` (§1); (3) `searched vacuous|vacuity … no matches`, refuted — 36+ files in `asupersync` alone, and the prior art is richer than the design it was cited to justify (§2.4). Two of the three were errors of *measurement method*, not of arithmetic, and no amount of prose review would have caught them.

The leg-3 RED in §2.8 is PROJECTED-BY-INSPECTION from a measured input set, not an observed test failure: no cargo command, gate binary, test suite, formatter, or linter was executed in producing this section. Every figure comes from the harness `grep`/`read` tools or an inline Python walk; shell `grep -r` with `--include=` is measured to return empty at exit 0 on this machine and is not a source for any figure above. Each reported zero names its search space and why that space could have contained the answer — the rule §4 item 13 imposes on gates, applied here to this document.
