#![allow(dead_code)]

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;


/// Records one scheduled invocation when the owning process exits.
///
/// This is intentionally a small, dependency-free bridge shared by the Rust lanes. The helper
/// owns JSONL serialization and kernel-lineage classification; the guard owns the process clock.
pub(crate) struct Run {
    lane: &'static str,
    started: Instant,
}

impl Run {
    pub fn new(lane: &'static str) -> Self {
        Self {
            lane,
            started: Instant::now(),
        }
    }
}

impl Drop for Run {
    fn drop(&mut self) {
        let elapsed = self.started.elapsed().as_secs().to_string();
        let helper = std::env::var_os("SCHEDULED_LANE_TELEMETRY_HELPER")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("CONTROL_PLANE_REPO").map(|root| PathBuf::from(root).join("bin/lib/scheduled-lane-telemetry.sh")));
        let Some(helper) = helper else { return; };
        let _ = Command::new(helper)
            .args(["--record", self.lane, &elapsed, "0"])
            .status();
    }
}
