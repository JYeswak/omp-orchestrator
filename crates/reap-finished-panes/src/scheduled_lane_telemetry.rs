#![allow(dead_code)]

use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// Records one invocation without delegating telemetry to a shell helper.
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

fn timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    let (hour, minute, second) = (rem / 3600, (rem % 3600) / 60, rem % 60);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let year = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let month_part = (5 * doy + 2) / 153;
    let day = (doy - (153 * month_part + 2) / 5 + 1) as u32;
    let month = if month_part < 10 {
        month_part + 3
    } else {
        month_part - 9
    } as u32;
    let year = if month <= 2 { year + 1 } else { year };
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z")
}

impl Drop for Run {
    fn drop(&mut self) {
        let ledger = std::env::var_os("SCHEDULED_LANE_TELEMETRY_LEDGER")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(PathBuf::from)
                    .map(|home| home.join(".local/state/flywheel/scheduled-lane-runs.jsonl"))
            });
        let Some(ledger) = ledger else {
            return;
        };
        if let Some(parent) = ledger.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let row = serde_json::json!({
            "ts": timestamp(),
            "event": "scheduled_lane_run",
            "lane": self.lane,
            "elapsed_s": self.started.elapsed().as_secs(),
            "rc": 0,
            "invoker": "MANUAL",
            "invoker_proof": "unproven",
        });
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(ledger) {
            let _ = writeln!(file, "{row}");
        }
    }
}
