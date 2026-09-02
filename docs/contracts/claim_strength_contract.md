# claim_strength_contract

Phase 0 · T1. **Written before the code**, per `plan_to_pin_the_orchestrator_type_algebra.md`: the
contract precedes the type, the type precedes the bead, the bead precedes the code.

One concern: **how strong is the evidence behind a claim, and may this evidence justify that
claim?** Nothing else. Whether the claim is *true*, whether anyone *checked* it, and whether the
system *obeys* the answer are three other concerns with three other owners.

---

## 1. The problem this type exists to remove

The rule already exists in prose. `~/.claude/CLAUDE.md:210-213`, THE ARTIFACT RIGOR STANDARD,
layer L1 — mined from `frankengraphdb`:

> Machine-readable claim registry with a strength lattice — invariant(6) > proof(5) >
> bounded_model(4) > statistical(3) > slo(2) > benchmark(1). A weaker claim may NEVER enforce or
> justify a stronger one: `rank(justifier) >= rank(claim)` or the build fails.

**Citation correction, recorded because a citation to a file that does not contain the text is a
defect this repo keeps paying for.** `plan_to_pin_the_orchestrator_type_algebra.md:51` attributes
this to `AGENTS.md`. It is not there: `grep -c bounded_model AGENTS.md` → **0**, with a positive
control of `grep -c BUILT AGENTS.md` → **3**, so the instrument reads that file. The text lives in
`~/.claude/CLAUDE.md`. The rule is unchanged; only its address is.

Prose cannot enforce `rank(justifier) >= rank(claim)`. A benchmark that justifies an invariant is
a sentence a reviewer must catch, every time, forever. This type makes that sentence
**inexpressible**.

---

## 2. Derivation position — why this carrier is authored and not re-exported

`crates/omp-types` opens with a standing rule: *"its contents are **derived** — re-exported from
`asupersync` at the exact rev we pin (`fa3c01aec`, v0.4.9) — never authored here."* Authoring a
type here therefore requires establishing that no upstream carrier fits. Measured against the
mirror, and this is the check the crate header demands rather than an assumption:

| upstream candidate | where | what it actually is | verdict |
|---|---|---|---|
| `Severity` | `types/outcome.rs:164` | `Ok / Err / Cancelled / Panicked` — how an operation *ended* | **no**: outcome classification, not evidence kind |
| `Severity` (second) | `audit/ambient.rs:72` | a different type with the same name | **trap #1** again: the name is not the type |
| `EvidenceStrength` | `lab/oracle/evidence.rs:36` | Bayes-factor buckets: `Against / Negligible / Positive / Strong / VeryStrong`, built by `from_log10_bf` | **no** — see below |
| `ProofStrength` | `atp/proof/bundle.rs:236` | `Basic / Enhanced / Cryptographic` — artifact attestation depth | **no**: about signing an artifact, not about a claim's warrant |
| `ProofStrength` (second) | `atp/manifest.rs:791` | same three names, different type | **trap #1**, third instance |

**`EvidenceStrength` is the near miss, and the reason it cannot serve is exact: it has an
`Against` variant.** It is a *magnitude* — how far a statistical test moved a posterior, including
against the hypothesis. A justification order has no element meaning "this evidence argues the
other way"; such a value is not a weaker justifier, it is a refutation, and folding it into the
same order would make `justifies(Against, Against)` true. It is also continuous in origin (a
`f64` log₁₀ Bayes factor), so its boundaries are numeric thresholds rather than *kinds of
warrant*. Ours is a **kind** order: a `statistical` claim is weaker than a `proof` claim no matter
how large its Bayes factor.

`frankengraphdb`, the L1 exemplar, encodes the same semantics as **TOML registry strings plus a
checker crate** — its `architecture_decisions.toml:493` carries the prose
`no_claim_boundary = ["Statistical or model-qualified evidence is never an invariant or semantic
safety proof."]`. So the *semantics* are adopted from frankengraphdb, the *shape* (a severity
lattice as a Rust enum with a `rank`) is adopted from asupersync's `Outcome`/`Severity`, and only
the **carrier set is authored here**, because neither upstream carrier is this concern.

---

## 3. The carrier set — six levels, and the argument for each boundary

```
Invariant     6   holds by construction; a counterexample is a compile error or a type error
Proof         5   a machine-checked derivation over a stated model
BoundedModel  4   exhaustive check over a bounded domain; silent outside the bound
Statistical   3   a sample with a stated test; true in distribution, not per-instance
Slo           2   a budget declared and monitored; a breach is a signal, not a refutation
Benchmark     1   one measurement, one machine, one moment
```

Six is not a round number chosen for symmetry. Each boundary answers a different question, and a
boundary that answered the same question as its neighbour would be a level to delete.

**`Benchmark → Slo` — is there a declared expectation?** A benchmark is a number. An SLO is a
number *plus a threshold someone committed to*, so a breach is meaningful without further
argument. Collapsing them loses the only thing that makes a performance figure actionable. This
repo has the scar: `NUMBERS.toml` exists precisely because a figure with no declared expectation
rots silently, and a `LIVE` figure differs from a pinned one exactly in whether a threshold was
promised.

**`Slo → Statistical` — is the claim about a population or a budget?** An SLO says "p95 under
X"; a statistical claim says "this property holds in distribution with a stated test". The second
supports *generalisation*, the first only *compliance*. A monitored budget cannot answer "does
this hold for inputs we have not seen", which is the question a statistical claim is for.

**`Statistical → BoundedModel` — is the domain sampled or exhausted?** This is the sharpest
boundary in the set. A sample leaves a probability of being wrong on any given input; an
exhaustive check over a bounded domain leaves **none inside the bound**. That is a difference in
kind, not degree, and it is why a property test that enumerates its whole carrier set is stronger
than one that samples it — the distinction this contract's own L3/L4 tests rely on.

**`BoundedModel → Proof` — is the bound the limit of the claim?** A bounded model is silent
outside its bound and a reader must know where the silence starts. A proof carries its model
explicitly and holds wherever the model does. Merging them would let a check over inputs 0..64
read as a statement about all inputs, which is the single most common overclaim in verification
work.

**`Proof → Invariant` — can the claim be violated at all?** A proof is a derivation *about* code;
an invariant is a property the code *cannot* violate because the types forbid it. A proof can be
correct about the wrong program, or drift from the program as the program changes. An invariant
cannot drift, because breaking it stops the build. `#![forbid(unsafe_code)]` is an invariant;
"we audited every unsafe block" is at best a proof.

**Why not a seventh level for `Human review`?** Because it is not comparable. A review can find a
defect a proof missed and can also miss everything, so it has no stable position in the order.
Admitting it would break L1 (totality) — and a level that breaks the law the type exists to
enforce is a level that belongs in a different type. §7 says so explicitly.

**Why not fewer than six?** Every merge above loses a question a reader must answer to know what a
claim is worth. Six is the smallest set where each boundary is a distinct question with a
different answer.

The numeric ranks are **1..=6 dense and ascending**, so `rank` is a total function onto a
contiguous range and `Ord` on the discriminants agrees with `Ord` on the ranks. Dense matters: a
gap would invite a future level to be inserted by renumbering, and renumbering is exactly the
silent widening L5 forbids.

---

## 4. Operations

| operation | signature | meaning |
|---|---|---|
| `rank` | `ClaimStrength -> u8` | position in the order, 1..=6. Total, injective, no failure mode. |
| `justifies` | `(justifier, claim) -> bool` | `rank(justifier) >= rank(claim)`. **The load-bearing operation.** |
| `Ord` / `PartialOrd` | derived | `a <= b` iff `rank(a) <= rank(b)`. Derived, not hand-written, so it cannot disagree with itself. |
| `weakest_of` | `(a, b) -> ClaimStrength` | the **meet**: a compound claim is only as strong as its weakest evidence. |
| `ALL` | `&'static [ClaimStrength; 6]` | the whole carrier set, so a test can exhaust rather than sample. |
| `as_str` / `FromStr` | `<-> &str` | the registry spelling (`"invariant"`, `"bounded_model"`, …) so a TOML row round-trips. |

There is deliberately **no** `promote`, `upgrade`, `strengthen`, `max_of`, or `set_strength`. Their
absence is L5, and it is enforced by the API surface rather than by a rule: the type is `Copy` with
no interior mutability and no operation returning a stronger value than both inputs. `weakest_of`
exists and `strongest_of` does not, because combining two pieces of evidence never produces a
stronger warrant than the better of them *for the compound claim* — it produces the weaker.

---

## 5. The laws

**L1 · TOTAL ORDER.** Any two strengths are comparable: for all `a, b`, exactly one of `a < b`,
`a == b`, `a > b`. No incomparable pair exists. *Why it matters:* an incomparable pair makes
`justifies` undecidable for that pair, and a gate that cannot decide fails open or fails closed
arbitrarily.

**L2 · ANTISYMMETRY.** `a >= b && b >= a` implies `a == b`. *Why it matters:* without it two
distinct levels could mutually justify each other, and the order would have a cycle — a benchmark
could justify an invariant by a chain of two steps.

**L3 · TRANSITIVITY.** `a >= b && b >= c` implies `a >= c`. **Tested over generated triples, not
examples.** *Why it matters:* justification chains are the normal case — a claim justified by a
claim justified by evidence — and non-transitivity would make a chain's validity depend on the
order it was assembled in.

**L4 · JUSTIFICATION.** `justifies(j, c)` iff `rank(j) >= rank(c)`. **A `Benchmark` can never
justify an `Invariant`.** This is the law with teeth and the one prose cannot enforce. **Tested
over all generated pairs**, and the negative direction is asserted explicitly: for every pair
where `rank(j) < rank(c)`, `justifies(j, c)` must be `false`.

**L5 · NO SILENT WIDENING.** There is no operation that raises a claim's strength. Strength is
assigned at creation from the evidence and is thereafter read-only. *Why it matters:* every
overclaim this repo has measured took the shape of a value that started honest and was later read
as stronger — a benchmark quoted as a guarantee, a fixture-green gate read as a wired one. L5 is
tested structurally: `weakest_of` never exceeds either input, and no public operation returns a
strength strictly greater than every input it received.

---

## 6. Relationship to the dialects already in the tree

Measured: `git grep -nE '^pub enum (Severity|.*Strength|.*Priority|.*Confidence)' -- 'crates/*/src/*'`
returns **one** hit, `ntm-fleet-monitor::GradeResult`, which is a pass/fail grade outcome and not
an evidence order. **There is no existing local dialect of this concept to unify.** The graded types
that do exist grade a *different* thing:

| existing type | crate | what it grades | relationship |
|---|---|---|---|
| `ComposerEvidence` | `receiver-receipt` | whether a pane's composer holds typed text | **coexists** — evidence about delivery, not about a claim's warrant |
| `TransportAuthority` / `DeliveryAuthority` / `AckAuthority` | `ack-spine` | which plane vouched for a message | **coexists** — three authorities answering one question, Phase 1's problem |
| `ReceiptVerdict`, `AckVerdict`, `FenceVerdict`, … | five crates | what happened | **coexists** — outcome, not warrant |
| `BoundedOutcome` | `subprocess-contract` | how a child process ended | **coexists** |

So T1 is **new vocabulary, coexisting**, not a unification. That is a narrower claim than the plan's
framing ("so five crates stop inventing dialects") and it is the measured one: the five crates are
not inventing *this* concept, they are inventing *verdict* and *authority* dialects, which are T2's
and Phase 1's concerns.

**Migration is NOT in Phase 0.** No caller is changed by this document or by the type it specifies.

---

## 7. NON-COVERAGE — what this type deliberately does not decide

1. **Whether a claim is TRUE.** `ClaimStrength` grades the warrant, never the proposition. An
   `Invariant`-strength claim can be false if the invariant was stated wrong; the type says only
   that a counterexample would be a build failure.
2. **Whether anyone CHECKED it.** Assigning `Proof` does not run a prover. The gap between "typed
   as proof" and "proved" is BUILT ≠ WIRED, and it is a different mechanism's job.
3. **Whether a caller OBEYS the order.** A green property test proves the law for the type. Phase 1
   wires consumers; until then a crate may still hold a benchmark and call it a guarantee, and this
   type will not have stopped it.
4. **How evidence is CLASSIFIED.** Deciding that a given artifact is `BoundedModel` and not
   `Statistical` is a judgement made at the point of creation. The type enforces the order among
   the labels; it cannot audit whether a label was applied honestly. **This is the largest hole and
   it is deliberate** — a type that tried to classify evidence would need to read the evidence.
5. **Human review, expert opinion, or consensus.** Not comparable to the six (§3), so admitting
   them would break L1. They belong in a separate, explicitly partial type if they are ever needed.
6. **Aggregation across many claims.** "Is this repo well-evidenced overall?" is a report, not a
   lattice operation. `weakest_of` is defined for *one compound claim*, not for a corpus.
7. **Staleness.** A `Proof` from six months ago and one from today are the same strength. Freshness
   is orthogonal and belongs to the artifact, not the order — `NUMBERS.toml` already owns drift.
8. **Cost.** An `Invariant` may be cheaper than a `Benchmark`. The order is about warrant, and
   nothing here licenses "stronger therefore more expensive".

---

## 8. Acceptance

- Type in `crates/omp-types`, `#![forbid(unsafe_code)]`, compiling.
- One property test per law, five total. **L3 and L4 exhaust the carrier set** — 36 ordered pairs
  and 216 ordered triples over six levels — which is strictly stronger than sampling and is itself
  a `BoundedModel`-strength claim, with the bound being the carrier set and the carrier set being
  finite and closed. A sampled property test of the same laws would be `Statistical`; this
  contract's own vocabulary makes the difference statable.
- A mutation that breaks L4 so a `Benchmark` justifies an `Invariant` turns the L4 property RED,
  followed by a byte-identical restore proven by `sha256`.

## NO-CLAIM

Pinning the order does not make any caller obey it. Phase 1 wires it; a green property test proves
the law for the **type**, not for its consumers, and today the number of production call sites is
**zero**. The six-level carrier is a judgement about which boundaries carry distinct questions, and
§3 argues each one rather than asserting the set; a seventh level or a merged pair remains arguable
and the argument, not the count, is what this document commits to. The upstream survey in §2 is
measured against one daily mirror snapshot at one pinned rev — a later asupersync rev could ship a
carrier that fits, at which point the honest move is to delete this one and re-export.
