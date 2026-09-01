use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

fn executable(dir: &Path, name: &str, body: &str) {
    let path = dir.join(name);
    fs::write(&path, body).expect("write fixture executable");
    let mut permissions = fs::metadata(&path).expect("fixture metadata").permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).expect("make fixture executable");
}

#[test]
fn disk_pressure_still_reports_observation_before_refusal() {
    let temp = tempfile::tempdir().expect("temporary test root");
    let repo = temp.path().join("repo");
    fs::create_dir_all(repo.join("docs/plan")).expect("fixture docs");
    fs::write(repo.join("docs/PLAN.md"), "fixture section\n").expect("fixture assembly");
    fs::write(repo.join("docs/plan/00-fixture.md"), "fixture section\n")
        .expect("fixture section");

    let fake_bin = temp.path().join("bin");
    fs::create_dir_all(&fake_bin).expect("fixture bin");
    executable(
        &fake_bin,
        "df",
        "#!/bin/sh\nprintf '%s\\n' 'Filesystem 1K-blocks Used Available Capacity Mounted on'\nprintf '%s\\n' '/dev/simulated 100000 95000 5000 95% /simulated'\n",
    );
    executable(
        &fake_bin,
        "tick-monitor",
        "#!/bin/sh\nprintf '%s\\n' '{\"omp_lifecycle\":{\"panes\":[{\"pane\":\"%probe\",\"state\":\"IDLE\",\"liveness\":\"CONFIRMED_IDLE\"}]},\"idle_panes\":{\"dispatchable\":[],\"free_capacity\":[]}}'\n",
    );
    executable(&fake_bin, "br", "#!/bin/sh\nprintf '%s\\n' '[]'\n");
    executable(
        &fake_bin,
        "reap-finished-panes",
        "#!/bin/sh\nprintf '%s\n' 'REAP_SWEEP reaped=0 skipped=1 awaiting_human=0 unswept=0 deadline_hit=0'\n",
    );

    let inherited_path = env::var_os("PATH").unwrap_or_default();
    let path = format!("{}:{}", fake_bin.display(), inherited_path.to_string_lossy());
    let output = Command::new(env!("CARGO_BIN_EXE_omp-orchestrator"))
        .current_dir(&repo)
        .args(["run", "--once", "--repo", "."])
        .env("PATH", path)
        .env("HOME", temp.path())
        .env("CARGO_TARGET_DIR", temp.path().join("build-target"))
        .env("OMP_TICK_MONITOR_BIN", "tick-monitor")
        .env("OMP_BR_BIN", "br")
        .env("OMP_HEARTBEAT_LEDGER", temp.path().join("heartbeat.jsonl"))
        .env("OMP_TICK_MONITOR_STATE", temp.path().join("monitor-state.json"))
        .env("OMP_PENDING_DISPATCH", temp.path().join("pending-dispatch"))
        .output()
        .expect("run orchestrator once");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("AFTER_CLI_OUTPUT_START\n{stdout}AFTER_CLI_OUTPUT_END");
    assert!(
        output.status.success(),
        "bounded pressure refusal should be reported as a tick outcome; stdout={stdout} stderr={stderr}"
    );
    let observation = stdout
        .find("OBSERVATION ")
        .expect("disk pressure must not suppress the observation line");
    let refusal = stdout
        .find("DISK_PRESSURE owner=josh next_action=cargo-clean-or-grow-volume")
        .expect("disk pressure refusal must retain its owner and next action");
    assert!(
        observation < refusal,
        "observation must precede disk refusal; stdout={stdout}"
    );
}
