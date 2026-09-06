use commit_build_fence::{check, BuildRegistration, FenceVerdict, RegistrationStore};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

fn fresh_repo(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("commit-build-fence-{name}-{nonce}"));
    fs::create_dir_all(&dir).expect("create repo");
    run_git(&dir, &["init", "-q"]);
    run_git(&dir, &["config", "user.email", "fence@example.invalid"]);
    run_git(&dir, &["config", "user.name", "commit-fence-test"]);
    fs::write(dir.join("README.md"), "baseline\n").expect("write baseline");
    run_git(&dir, &["add", "--", "README.md"]);
    run_git(&dir, &["commit", "--quiet", "-m", "chore: baseline [test]"]);
    dir
}

fn run_git(dir: &Path, args: &[&str]) -> Output {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .expect("spawn git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    output
}

fn run_git_with_store(dir: &Path, store: &Path, args: &[&str]) -> Output {
    Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("OMP_BUILD_REGISTRATION", store)
        .output()
        .expect("spawn git with fence store")
}
fn run_fence(args: &[String]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_commit-build-fence"))
        .args(args)
        .output()
        .expect("spawn commit fence binary")
}

fn install_hook(dir: &Path) -> PathBuf {
    let hook = dir.join(".git/hooks/pre-commit");
    fs::copy(env!("CARGO_BIN_EXE_commit-build-fence"), &hook).expect("install fence hook");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755))
            .expect("make hook executable");
    }
    hook
}

fn current_head(dir: &Path) -> String {
    String::from_utf8(run_git(dir, &["rev-parse", "HEAD"]).stdout)
        .expect("head utf8")
        .trim()
        .to_owned()
}

fn stage_file(dir: &Path, name: &str, contents: &str) {
    fs::write(dir.join(name), contents).expect("write staged file");
    run_git(dir, &["add", "--", name]);
}

fn store_for(dir: &Path) -> PathBuf {
    dir.join(".git/omp-build-registration.json")
}
fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_secs()
}

#[test]
fn real_hook_refuses_active_registration_with_actionable_identity() {
    let dir = fresh_repo("active");
    let store_path = store_for(&dir);
    RegistrationStore::empty()
        .save_atomic(&store_path)
        .expect("initialize explicit empty store");
    install_hook(&dir);

    stage_file(&dir, "clean.rs", "fn main() {}\n");
    let allowed = run_git_with_store(
        &dir,
        &store_path,
        &["commit", "--quiet", "-m", "chore: clean [test]"],
    );
    assert!(
        allowed.status.success(),
        "known-good commit refused: {}",
        String::from_utf8_lossy(&allowed.stderr)
    );

    let repo = dir.canonicalize().expect("canonical repo");
    let mut store = RegistrationStore::load(&store_path).expect("load store");
    let started_at_unix = now_unix();
    store
        .register(BuildRegistration {
            build_id: "build-live-42".to_owned(),
            repo: repo.display().to_string(),
            head: current_head(&dir),
            holder: "agent-blue".to_owned(),
            started_at_unix,
            expires_at_unix: started_at_unix + 1_800,
        })
        .expect("register build");
    store.save_atomic(&store_path).expect("save active store");

    stage_file(&dir, "blocked.rs", "fn blocked() {}\n");
    let head_before = current_head(&dir);
    let refused = run_git_with_store(
        &dir,
        &store_path,
        &["commit", "--quiet", "-m", "feat: blocked [test]"],
    );
    let stderr = String::from_utf8_lossy(&refused.stderr);
    assert!(!refused.status.success(), "active build must refuse commit");
    assert!(
        stderr.contains("COMMIT_FENCE_REFUSED"),
        "missing refusal marker: {stderr}"
    );
    assert!(
        stderr.contains("build-live-42"),
        "missing build id: {stderr}"
    );
    assert!(stderr.contains("agent-blue"), "missing holder: {stderr}");
    assert!(
        stderr.contains(&head_before),
        "missing current HEAD: {stderr}"
    );
    assert_eq!(
        current_head(&dir),
        head_before,
        "HEAD read-back must remain stable"
    );
}

#[test]
fn mutation_live_registration_predicate_is_red_and_restores_green() {
    let dir = fresh_repo("mutation");
    let store_path = store_for(&dir);
    RegistrationStore::empty()
        .save_atomic(&store_path)
        .expect("initialize mutation store");
    let repo = dir.canonicalize().expect("canonical repo");
    let now = now_unix();
    let mut store = RegistrationStore::load(&store_path).expect("load mutation store");
    store
        .register(BuildRegistration {
            build_id: "mutation-live".to_owned(),
            repo: repo.display().to_string(),
            head: current_head(&dir),
            holder: "agent-mutation".to_owned(),
            started_at_unix: now,
            expires_at_unix: now + 1_800,
        })
        .expect("register live build");
    store.save_atomic(&store_path).expect("save mutation store");
    let refused = check(&store_path, &repo.display().to_string(), &current_head(&dir), now)
        .expect("live registration should be readable");
    assert!(matches!(refused, FenceVerdict::Refused { .. }), "live registration must refuse: {refused:?}");
    let mut restored = RegistrationStore::load(&store_path).expect("reload mutation store");
    restored
        .release("mutation-live", &repo.display().to_string(), "agent-mutation", now)
        .expect("release live build");
    restored.save_atomic(&store_path).expect("save restored store");
    let clear = check(&store_path, &repo.display().to_string(), &current_head(&dir), now)
        .expect("restored store should be readable");
    assert!(clear.is_clear(), "restoring the registration must return clear: {clear:?}");
    fs::remove_dir_all(dir).ok();
}

#[test]
fn real_hook_allows_commit_with_valid_empty_store() {
    let dir = fresh_repo("good");
    let store_path = store_for(&dir);
    RegistrationStore::empty()
        .save_atomic(&store_path)
        .expect("initialize explicit empty store");
    install_hook(&dir);
    stage_file(&dir, "good.rs", "fn good() {}\n");

    let output = run_git_with_store(
        &dir,
        &store_path,
        &["commit", "--quiet", "-m", "chore: good [test]"],
    );
    assert!(
        output.status.success(),
        "valid empty store must allow commit: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn real_hook_treats_missing_store_as_error() {
    let dir = fresh_repo("missing");
    let store_path = dir.join(".git/missing-registration.json");
    install_hook(&dir);
    stage_file(&dir, "missing.rs", "fn missing() {}\n");

    let output = run_git_with_store(
        &dir,
        &store_path,
        &["commit", "--quiet", "-m", "feat: missing store [test]"],
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "missing store must fail closed");
    assert!(
        stderr.contains("COMMIT_FENCE_ERROR"),
        "missing typed error: {stderr}"
    );
    assert!(
        stderr.contains("registration_store_missing"),
        "missing-store reason absent: {stderr}"
    );
}
#[test]
fn ci_shaped_init_then_check_clears_and_active_registration_refuses() {
    let dir = fresh_repo("ci-shaped");
    let repo = dir.canonicalize().expect("canonical repo");
    let repo_arg = repo.display().to_string();

    let init = run_fence(&["init".to_owned(), "--repo".to_owned(), repo_arg.clone()]);
    assert!(
        init.status.success(),
        "fresh-checkout init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );
    assert!(
        store_for(&dir).is_file(),
        "init must create the default store"
    );

    let clear = run_fence(&[
        "check".to_owned(),
        "--repo".to_owned(),
        repo_arg.clone(),
        "--head".to_owned(),
        "ci-head".to_owned(),
        "--now".to_owned(),
        "200".to_owned(),
    ]);
    assert!(
        clear.status.success(),
        "initialized clean checkout must clear: {}",
        String::from_utf8_lossy(&clear.stderr)
    );

    let store_path = store_for(&dir);
    let mut store = RegistrationStore::load(&store_path).expect("load initialized store");
    store
        .register(BuildRegistration {
            build_id: "build-ci-shaped".to_owned(),
            repo: repo_arg.clone(),
            head: "registered-head".to_owned(),
            holder: "ci-test".to_owned(),
            started_at_unix: 100,
            expires_at_unix: 500,
        })
        .expect("register active build");
    store
        .save_atomic(&store_path)
        .expect("save active registration");

    let fenced = run_fence(&[
        "check".to_owned(),
        "--repo".to_owned(),
        repo_arg,
        "--head".to_owned(),
        "ci-head".to_owned(),
        "--now".to_owned(),
        "200".to_owned(),
    ]);
    let stderr = String::from_utf8_lossy(&fenced.stderr);
    assert_eq!(
        fenced.status.code(),
        Some(1),
        "active fence must refuse: {stderr}"
    );
    assert!(
        stderr.contains("COMMIT_FENCE_REFUSED"),
        "missing fence verdict: {stderr}"
    );
    assert!(
        stderr.contains("build-ci-shaped"),
        "missing build identity: {stderr}"
    );
}

#[test]
fn cli_registration_expiry_and_release_are_durable() {
    let dir = fresh_repo("cli");
    let repo = dir.canonicalize().expect("canonical repo");
    let store = dir.join("build-registration.json");
    let repo_arg = repo.display().to_string();
    let store_arg = store.display().to_string();
    let init = run_fence(&[
        "init".to_owned(),
        "--repo".to_owned(),
        repo_arg.clone(),
        "--store".to_owned(),
        store_arg.clone(),
    ]);
    assert!(
        init.status.success(),
        "init failed: {}",
        String::from_utf8_lossy(&init.stderr)
    );

    let head = current_head(&dir);
    let register = run_fence(&[
        "register".to_owned(),
        "--repo".to_owned(),
        repo_arg.clone(),
        "--store".to_owned(),
        store_arg.clone(),
        "--build-id".to_owned(),
        "build-cli-1".to_owned(),
        "--holder".to_owned(),
        "agent-blue".to_owned(),
        "--head".to_owned(),
        head.clone(),
        "--now".to_owned(),
        "100".to_owned(),
        "--ttl-secs".to_owned(),
        "1".to_owned(),
    ]);
    assert!(
        register.status.success(),
        "register failed: {}",
        String::from_utf8_lossy(&register.stderr)
    );

    let expired_check = run_fence(&[
        "check".to_owned(),
        "--repo".to_owned(),
        repo_arg.clone(),
        "--store".to_owned(),
        store_arg.clone(),
        "--head".to_owned(),
        head,
        "--now".to_owned(),
        "101".to_owned(),
    ]);
    assert!(
        expired_check.status.success(),
        "expired registration must clear the fence: {}",
        String::from_utf8_lossy(&expired_check.stderr)
    );

    let mut loaded = RegistrationStore::load(&store).expect("load registered store");
    let event = loaded
        .release("build-cli-1", &repo_arg, "agent-blue", 200)
        .expect("release expired registration");
    loaded.save_atomic(&store).expect("save release event");
    let reread = RegistrationStore::load(&store).expect("read released store");
    assert!(reread.registrations.is_empty());
    assert_eq!(reread.events, vec![event]);
}

/// THE CALLER/CALLEE CONTRACT, pinned as a test because it was pinned in a
/// CI workflow instead and nothing checked that the two agreed.
///
/// Measured 2026-09-02, run `33585450134` on tree `4b398e4`:
/// `.github/workflows/gate.yml:180` ran `cargo run -p commit-build-fence
/// -- .` and the job died with `COMMIT_FENCE_ERROR reason=unknown_command
/// command=.` exit 2. Three sibling gates in the SAME workflow —
/// `no-shell-gate`, `pre-delete-citation-check`, `omp-inventory-map` — do
/// take a bare positional repo root, so the argv was written to the
/// convention next door rather than to this binary's.
///
/// The gate was RIGHT to refuse and this test locks that in: a bare `.`
/// must stay exit 2. What it must ALSO do is let the documented default
/// command be used with its own documented options, which it could not.
#[test]
fn cli_contract_refuses_a_positional_path_and_accepts_the_documented_default() {
    let dir = fresh_repo("cli-contract");
    let repo = dir.canonicalize().expect("canonical repo");
    let repo_arg = repo.display().to_string();
    let store = store_for(&dir);
    let store_arg = store.display().to_string();

    // The exact argv the failing CI run used. Still refused, still exit 2,
    // and the refusal now names the flag that WOULD have worked.
    let positional = run_fence(&[".".to_owned()]);
    assert_eq!(
        positional.status.code(),
        Some(2),
        "a bare positional path must stay a hard refusal"
    );
    let stderr = String::from_utf8_lossy(&positional.stderr);
    assert!(
        stderr.contains("reason=unknown_command command=."),
        "the refusal must name the token it rejected: {stderr}"
    );
    assert!(
        stderr.contains("--repo"),
        "the refusal must name the flag that names a repo, or a caller can \
         see they are wrong without seeing what is right: {stderr}"
    );

    // A leading KNOWN flag selects the default `check` command. Before the
    // fix this exited 2 with `unknown_command command=--repo`, which made
    // every documented option of the default command unreachable.
    RegistrationStore::init(&store).expect("init store");
    let defaulted = run_fence(&["--repo".to_owned(), repo_arg.clone()]);
    assert!(
        defaulted.status.success(),
        "`--repo PATH` with no subcommand must run check: {}",
        String::from_utf8_lossy(&defaulted.stderr)
    );

    // ...and it really ran CHECK, not a no-op: an active registration on
    // the current HEAD must make the same argv REFUSE. Without this leg a
    // silent no-op would pass the assertion above.
    let head = current_head(&dir);
    let mut loaded = RegistrationStore::load(&store).expect("load store");
    loaded
        .register(BuildRegistration {
            build_id: "build-default-cmd".to_owned(),
            repo: repo_arg.clone(),
            head: head.clone(),
            holder: "agent-default".to_owned(),
            started_at_unix: now_unix(),
            expires_at_unix: now_unix() + 600,
        })
        .expect("register active build");
    loaded.save_atomic(&store).expect("save registration");
    let blocked = run_fence(&["--repo".to_owned(), repo_arg.clone()]);
    assert_eq!(
        blocked.status.code(),
        Some(1),
        "the defaulted command must be a real check: {}",
        String::from_utf8_lossy(&blocked.stderr)
    );
    assert!(
        String::from_utf8_lossy(&blocked.stderr).contains("COMMIT_FENCE_REFUSED"),
        "an active registration must refuse through the defaulted command"
    );

    // `--store=PATH` proves the `=` form is recognised by the same leg.
    let equals_form = run_fence(&[
        format!("--store={store_arg}"),
        "--repo".to_owned(),
        repo_arg,
    ]);
    assert_eq!(
        equals_form.status.code(),
        Some(1),
        "`--flag=value` must select the default command too: {}",
        String::from_utf8_lossy(&equals_form.stderr)
    );

    // NEGATIVE CONTROL: the flag leg must not swallow a typo. A gate that
    // passes on a misspelled argument is worse than one that refuses a
    // valid argv, because it manufactures confidence.
    let typo = run_fence(&["--rpeo".to_owned(), ".".to_owned()]);
    assert_eq!(
        typo.status.code(),
        Some(2),
        "a misspelled flag must NOT be treated as the default command: {}",
        String::from_utf8_lossy(&typo.stdout)
    );

    // `--help` is a command, exits 0, and lists every accepted subcommand.
    let help = run_fence(&["--help".to_owned()]);
    assert!(help.status.success(), "--help must exit 0");
    let text = String::from_utf8_lossy(&help.stdout);
    for accepted in ["check", "init", "register", "release"] {
        assert!(
            text.contains(accepted),
            "usage must name the {accepted} command: {text}"
        );
    }
}
