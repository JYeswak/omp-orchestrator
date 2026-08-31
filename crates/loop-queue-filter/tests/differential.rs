use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
};

fn fixture_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("loop-queue-filter-diff-{}", std::process::id()));
    fs::create_dir_all(&path).expect("create differential fixture");
    fs::write(path.join("CHARTER.md"), "- M1 selector\n| **W2** | later\n").expect("write charter");
    path
}

fn run_python(input: &str, args: &[&str], envs: &BTreeMap<String, String>) -> std::process::Output {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let root = manifest
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root");
    let mut command = Command::new("python3");
    command.arg(root.join("bin/loop-queue-filter.py"));
    command
        .args(args)
        .envs(envs)
        .current_dir(root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn Python oracle");
    child
        .stdin
        .take()
        .expect("oracle stdin")
        .write_all(input.as_bytes())
        .expect("write oracle input");
    child.wait_with_output().expect("wait for Python oracle")
}

fn run_rust(input: &str, args: &[&str], envs: &BTreeMap<String, String>) -> std::process::Output {
    let binary = std::env::var_os("LOOP_QUEUE_FILTER_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_BIN_EXE_loop-queue-filter")));
    let mut command = Command::new(binary);
    command
        .args(args)
        .envs(envs)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("spawn Rust selector");
    child
        .stdin
        .take()
        .expect("selector stdin")
        .write_all(input.as_bytes())
        .expect("write selector input");
    child.wait_with_output().expect("wait for Rust selector")
}

#[test]
fn differential_compares_nonempty_cases_and_detects_known_bad_probe() {
    let dir = fixture_dir();
    let cooldown = dir.join("cooldown.json").display().to_string();
    let mut envs = BTreeMap::new();
    envs.insert("QUEUE_WANT".into(), "2".into());
    envs.insert("HARVEST_EXCLUDE".into(), "1".into());
    envs.insert("QUEUE_COOLDOWN_FILE".into(), cooldown);
    envs.insert("REPO_DIR".into(), dir.display().to_string());
    let cases: Vec<(&str, Vec<&str>, BTreeMap<String, String>)> = vec![
        (
            r#"{"issues":[{"id":"cp-b","title":"second","description":"","status":"open","priority":1},{"id":"cp-a","title":"first","description":"","status":"open","priority":0}]}"#,
            vec![],
            envs.clone(),
        ),
        (
            r#"{"issues":[{"id":"cp-g","title":"[DECISION] purchase API access","description":"","status":"open","priority":0},{"id":"cp-a","title":"measure queue","description":"","status":"open","priority":1}]}"#,
            vec![],
            envs.clone(),
        ),
        (
            r#"{"issues":[{"id":"cp-r","title":"Provision","description":"BLOCKED ON DISK","status":"open","priority":0},{"id":"cp-u","title":"Attribute disk","description":"BLOCKED ON DISK","status":"open","priority":1}]}"#,
            vec![],
            envs.clone(),
        ),
        (
            r#"{"issues":[{"id":"EPIC-1","title":"parent","description":"","status":"open","issue_type":"epic","priority":0},{"id":"EPIC-1a","title":"leaf","description":"","status":"open","priority":0},{"id":"cp-z","title":"other","description":"","status":"open","priority":1}]}"#,
            vec!["EPIC-1"],
            envs.clone(),
        ),
        (
            r#"{"issues":[{"id":"cp-z","title":"ordinary","description":"","status":"open","priority":0},{"id":"cp-a","title":"M1 selector","description":"","status":"open","priority":0}]}"#,
            vec![],
            envs.clone(),
        ),
        (
            r#"{"issues":[{"id":"cp-d","title":"harvest[DOCTRINE]: one","description":"","status":"open","priority":0},{"id":"cp-c","title":"harvest[CONFORMANCE]: two","description":"","status":"open","priority":0}]}"#,
            vec![],
            {
                let mut e = envs.clone();
                e.remove("HARVEST_EXCLUDE");
                e.insert("HARVEST_CLASS".into(), "DOCTRINE".into());
                e
            },
        ),
        (
            r#"{"issues":[{"id":"cp-a","title":"counted","description":"","status":"open","priority":0}]}"#,
            vec!["--count"],
            envs.clone(),
        ),
        (
            r#"[{"id":"cp-a","title":"array input","description":"","status":"open","priority":0}]"#,
            vec![],
            envs.clone(),
        ),
    ];
    assert!(
        !cases.is_empty(),
        "anti-vacuity: differential must execute at least one case"
    );
    for (input, args, case_env) in &cases {
        let python = run_python(input, args, case_env);
        let rust = run_rust(input, args, case_env);
        if python.status != rust.status
            || python.stdout != rust.stdout
            || python.stderr != rust.stderr
        {
            panic!("differential disagreement input={input:?} python={python:?} rust={rust:?}");
        }
    }
    let probe_python = run_python(cases[1].0, &cases[1].1, &cases[1].2);
    let mut mutated_rust = run_rust(cases[1].0, &cases[1].1, &cases[1].2);
    mutated_rust.stdout.extend_from_slice(b"MUTATION");
    assert_ne!(
        probe_python.stdout, mutated_rust.stdout,
        "anti-vacuity: known-bad autonomy-policy mutation must be visible"
    );
    println!("DIFFERENTIAL KNOWN_BAD probe=autonomy-policy disagreements=1");
    println!("DIFFERENTIAL PASS cases={} disagreements=0", cases.len());
    let _ = fs::remove_dir_all(dir);
}
