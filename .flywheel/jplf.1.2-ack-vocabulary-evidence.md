# jplf.1.2 Ack vocabulary evidence

Bead: `omp-orchestrator-jplf.1.2`

## Decision

`AckKind` and `DeliveryClass` are authored locally in
`crates/omp-types/src/ack_vocabulary.rs`. They mirror the inhabited upstream
semantics at pinned asupersync rev `fa3c01aec`:

- `messaging/class.rs:17-29`: five `DeliveryClass` variants, ordered from
  `EphemeralInteractive` through `ForensicReplayable`.
- `messaging/class.rs:83-94`: five inhabited `AckKind` variants, ordered from
  `Accepted` through `Received`.
- `messaging/class.rs:59-62`: delivery-class to acknowledgement mapping.

The upstream messaging module remains unavailable without
`messaging-fabric`. At this pin, enabling that feature alone fails at
`messaging/consumer.rs:1299-1300`, where an ungated default implementation
calls `TaskId::new_ephemeral` and `RegionId::new_ephemeral`; those constructors
require `test-internals`. Neither feature is enabled here, and the pin was not
advanced.

## Landed

Commit `df6828ce2a5dfe3ba1608338ae6095cd75d74eb3` (`feat(omp-types): author
ack vocabulary [test]`) adds the authored module, root re-exports, updated
upstream/local distinction, and inventory evidence. The `ObligationLedger`
half remains landed in `eea218d`.

## Verification

Remote lane, unchanged pin and default feature set:

```text
RCH_WORKER=contabo-3 CARGO_BUILD_JOBS=2 RCH_REQUIRE_REMOTE=1
RCH_VISIBILITY=quiet rch exec -- cargo test -j 2 -p omp-types --lib

running 5 tests
...
test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Remote command finished: exit=0
```

The five tests include:

- `ack_kind_vocabulary_is_inhabited_and_ordered`
- `delivery_class_vocabulary_is_inhabited_and_mapped`
- `display_names_match_the_wire_vocabulary`
- existing Outcome/cancellation vocabulary test
- existing distinct identity-type test

Inventory compatibility:

```text
RCH_WORKER=contabo-3 CARGO_BUILD_JOBS=2 RCH_REQUIRE_REMOTE=1
RCH_VISIBILITY=quiet rch exec -- cargo test -j 2 -p omp-inventory-map --lib

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
Remote command finished: exit=0
```

The inventory rows for `AckKind` and `DeliveryClass` remain
`PresentUpstreamBlocked`, now explicitly naming the local authored mirror.

## NO-CLAIM

This receipt proves the local vocabulary compiles and its tested ordering and
mapping match the pinned upstream source. It does not prove transport delivery,
receiver acknowledgement, authority-plane commitment, live messaging-fabric
reachability, or downstream caller migration.
