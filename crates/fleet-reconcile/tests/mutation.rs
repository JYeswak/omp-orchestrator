//! Fires-on-known-bad mutation legs. Each names its rule on a column-0 RED line.
//! A nonzero exit alone is not evidence (fh C31).

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_fleet-reconcile"))
}

fn write_fix(dir: &std::path::Path, tmux: &str, list: &str, snap: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(dir.join("tmux-sessions.txt"), tmux).unwrap();
    fs::write(dir.join("ntm-list.txt"), list).unwrap();
    fs::write(dir.join("snapshot.json"), snap).unwrap();
    fs::write(dir.join("ft-state.json"), "").unwrap();
}

fn run(args: &[&str], fixture: &std::path::Path) -> (i32, String) {
    let out = Command::new(rust_bin())
        .args(args)
        .env("FLEET_RECONCILE_FIXTURE_DIR", fixture)
        .output()
        .expect("spawn");
    (
        out.status.code().unwrap_or(99),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

fn verdict(json: &str) -> (String, String) {
    let v: serde_json::Value = serde_json::from_str(json.trim()).unwrap_or(serde_json::Value::Null);
    (
        v.get("detector")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
        v.get("verdict")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .into(),
    )
}

#[test]
fn mutation_name_sets_must_agree() {
    let dir = std::env::temp_dir().join(format!("fr-mut-nameset-{}", std::process::id()));
    write_fix(
        &dir,
        "alpha\nbeta\n",
        "  alpha: 1 pane\n",
        r#"{"success":true,"summary":{"total_sessions":1},"sessions":[{"name":"alpha"}]}"#,
    );
    let (off_rc, off_out) = run(
        &[
            "--json",
            "--mutation",
            "--disable-rule",
            "name_sets_must_agree",
        ],
        &dir,
    );
    let (_d, off_ver) = verdict(&off_out);
    assert_eq!(off_ver, "PASS");
    assert_eq!(off_rc, 0);
    println!("MUTATION name_sets_must_agree disabled -> PASS (false pass of a name-set disagree)");

    let (on_rc, on_out) = run(&["--json"], &dir);
    let (det, on_ver) = verdict(&on_out);
    assert_eq!(on_ver, "FAIL");
    assert_eq!(on_rc, 1);
    assert_eq!(det, "ntm_tmux_disagree");
    println!("MUTATION RED name_sets_must_agree: detector={det} verdict=FAIL");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn mutation_list_empty_text_fails_closed() {
    let dir = std::env::temp_dir().join(format!("fr-mut-emptytext-{}", std::process::id()));
    write_fix(
        &dir,
        "control-plane\n",
        "No tmux sessions running\n",
        r#"{"success":true,"summary":{"total_sessions":1},"sessions":[{"name":"control-plane"}]}"#,
    );
    let (_off_rc, off_out) = run(
        &[
            "--json",
            "--mutation",
            "--disable-rule",
            "list_empty_text_fails_closed",
        ],
        &dir,
    );
    let (_d, off_ver) = verdict(&off_out);
    assert_eq!(off_ver, "PASS");
    println!("MUTATION list_empty_text_fails_closed disabled -> PASS (false pass of ntm list empty-text)");

    let (_on_rc, on_out) = run(&["--json"], &dir);
    let (det, on_ver) = verdict(&on_out);
    assert_eq!(on_ver, "FAIL");
    assert_eq!(det, "ntm_list_empty_text");
    println!("MUTATION RED list_empty_text_fails_closed: detector={det} verdict=FAIL");
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn mutation_unparseable_is_fail() {
    let dir = std::env::temp_dir().join(format!("fr-mut-unparse-{}", std::process::id()));
    write_fix(&dir, "alpha\n", "alpha:\n", "not json");
    let (_off_rc, off_out) = run(
        &[
            "--json",
            "--mutation",
            "--disable-rule",
            "unparseable_is_fail",
        ],
        &dir,
    );
    let (_d, off_ver) = verdict(&off_out);
    assert_eq!(off_ver, "PASS");
    println!(
        "MUTATION unparseable_is_fail disabled -> PASS (false pass of an unobservable snapshot)"
    );

    let (_on_rc, on_out) = run(&["--json"], &dir);
    let (det, on_ver) = verdict(&on_out);
    assert_eq!(on_ver, "FAIL");
    assert_eq!(det, "ntm_snapshot_unparseable");
    println!("MUTATION RED unparseable_is_fail: detector={det} verdict=FAIL (unobservable is FAIL, never a false PASS)");
    let _ = fs::remove_dir_all(&dir);
}
