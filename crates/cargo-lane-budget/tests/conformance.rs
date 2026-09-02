use cargo_lane_budget::{derive_budget, parse_aggregate, resolve_lane_identity};

#[test]
fn derived_budget_is_session_scaled_and_floored() {
    assert_eq!(derive_budget(3, 22, 29, 96), 96);
    assert_eq!(derive_budget(7, 22, 29, 96), 183);
}

#[test]
fn container_vs_df_uses_capacity_not_allocated() {
    let fixture = "Size (Capacity Ceiling): 1000 B\nCapacity Not Allocated: 600 B\nCapacity Quota: 400 B\nCapacity Consumed: 100 B\n";
    let stats = parse_aggregate(fixture, 100).expect("authoritative APFS fixture parses");
    assert_eq!(stats.free_bytes, 600);
    assert_eq!(stats.allowed_bytes, 600);
    assert!(stats.headroom_bytes >= 0);
}

#[test]
fn over_budget_is_not_admitted() {
    assert!(derive_budget(1, 1, 0, 1) < 2);
}

#[test]
fn shared_lane_identity_does_not_depend_on_task() {
    assert_eq!(
        resolve_lane_identity(Some("a"), "session", false),
        resolve_lane_identity(Some("b"), "session", false)
    );
    assert_ne!(
        resolve_lane_identity(Some("a"), "session", true),
        resolve_lane_identity(Some("b"), "session", true)
    );
}
