//! TOOLCHAIN PARITY LEGS for `pre-push-gate` — the gate must refuse to certify a
//! build CI cannot reproduce, and must still be able to say yes.
//!
//! # The measured failure these legs exist for
//!
//! 2026-09-02. `gh run list --limit 200` returned **67 runs and 67 failures**,
//! every one since 2026-09-01T05:59:47Z. Three independently-failing jobs
//! (`commit-build-fence`, `omp-inventory-map`, `porting-gate`) died at the same
//! line:
//!
//! ```text
//! error[E0554]: `#![feature]` may not be used on the stable release channel
//! 52 | #![cfg_attr(feature = "nightly-outcome-try", feature(try_trait_v2))]
//! error: could not compile `asupersync` (lib) due to 1 previous error
//! ```
//!
//! The runner had stable; this workspace is nightly. For all 67 of those pushes
//! `pre-push-gate` printed `PRE_PUSH_GATE_OK`. **The local gate certified exactly
//! what CI rejected**, and it did so honestly — it had never been asked about a
//! compiler.
//!
//! # Why these are legs and not a comment
//!
//! The fix is a refusal, and a refusal that has never been observed to fire is a
//! claim, not a mechanism. Four of the five legs inject a KNOWN-BAD state and
//! assert the gate goes red for the right reason; the fifth asserts it can still
//! go green, because a gate that only ever refuses is broken in the other
//! direction and gets uninstalled.

use std::path::{Path, PathBuf};
use std::process::Command;

const GATE: &str = env!("CARGO_BIN_EXE_pre-push-gate");

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("workspace root resolves")
}

/// The channel this repo pins — read, never hardcoded, so bumping the pin does
/// not silently turn these legs vacuous.
fn pinned_channel() -> String {
    let text = std::fs::read_to_string(repo_root().join("rust-toolchain.toml"))
        .expect("rust-toolchain.toml exists — its absence is the defect under test");
    text.lines()
        .map(str::trim)
        .filter(|l| !l.starts_with('#'))
        .find_map(|l| l.strip_prefix("channel"))
        .and_then(|r| r.trim_start().strip_prefix('='))
        .map(|v| v.trim().trim_matches('"').to_string())
        .expect("[toolchain] channel is set")
}

fn pinned_commit() -> String {
    let out = Command::new("rustc")
        .arg(format!("+{}", pinned_channel()))
        .arg("-vV")
        .output()
        .expect("rustc runs");
    assert!(out.status.success(), "the pinned toolchain must be installed");
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("commit-hash:").map(|s| s.trim().to_string()))
        .expect("rustc -vV reports commit-hash")
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "omp-parity-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }

    fn write(&self, rel: &str, body: &str) {
        let p = self.0.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).expect("parent");
        }
        std::fs::write(p, body).expect("write");
    }

    fn receipt(&self, extra: &str) {
        self.write(
            ".flywheel/workspace-green.receipt",
            &format!("workspace_green_receipt\nfailing_suites=0\n{extra}"),
        );
    }

    fn verify(&self) -> (i32, String) {
        // `--repo` rather than cwd: the gate resolves the root by walking up for
        // `.git`, and a scratch dir under /tmp would otherwise find nothing.
        let out = Command::new(GATE)
            .args(["--repo", self.0.to_str().unwrap()])
            .current_dir(&self.0)
            .env_remove("RUSTUP_TOOLCHAIN")
            .output()
            .expect("gate runs");
        (
            out.status.code().unwrap_or(-1),
            format!(
                "{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ),
        )
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// LEG 1 (the 67-run state): no pin at all. CI would compile with the runner
/// default and the receipt cannot speak for it.
#[test]
fn refuses_when_the_repo_pins_no_toolchain() {
    let s = Scratch::new("nopin");
    s.receipt("rustc_commit=deadbeef\n");
    let (code, out) = s.verify();
    assert_eq!(code, 1, "must refuse, got:\n{out}");
    assert!(
        out.contains("no [toolchain] channel"),
        "must name the missing pin, got:\n{out}"
    );
}

/// LEG 2: a receipt written before parity was checked. Not grandfathered — this
/// is precisely the shape of receipt that certified the 67 red runs.
#[test]
fn refuses_a_receipt_that_records_no_compiler() {
    let s = Scratch::new("legacy");
    s.write(
        "rust-toolchain.toml",
        &format!("[toolchain]\nchannel = \"{}\"\n", pinned_channel()),
    );
    s.receipt("recorded_by=pre-push-gate\n");
    let (code, out) = s.verify();
    assert_eq!(code, 1, "must refuse, got:\n{out}");
    assert!(
        out.contains("records no compiler"),
        "must name the missing compiler field, got:\n{out}"
    );
}

/// LEG 3: the suite ran, but under a different compiler than the pin now names —
/// e.g. somebody bumped the pin after recording.
#[test]
fn refuses_a_receipt_recorded_by_a_different_compiler() {
    let s = Scratch::new("mismatch");
    s.write(
        "rust-toolchain.toml",
        &format!("[toolchain]\nchannel = \"{}\"\n", pinned_channel()),
    );
    s.receipt("rustc_commit=0000000000000000000000000000000000000000\n");
    let (code, out) = s.verify();
    assert_eq!(code, 1, "must refuse, got:\n{out}");
    assert!(
        out.contains("recorded by a different compiler"),
        "must name the compiler mismatch, got:\n{out}"
    );
}

/// LEG 4: an unresolvable pin. CI installs the pin from the file, so a receipt
/// recorded without it proves nothing.
#[test]
fn refuses_a_pin_that_does_not_resolve() {
    let s = Scratch::new("badpin");
    s.write(
        "rust-toolchain.toml",
        "[toolchain]\nchannel = \"nightly-1999-01-01\"\n",
    );
    s.receipt("rustc_commit=deadbeef\n");
    let (code, out) = s.verify();
    assert_eq!(code, 1, "must refuse, got:\n{out}");
    assert!(
        out.contains("could not be resolved"),
        "must name the unresolvable pin, got:\n{out}"
    );
}

/// LEG 5 (the other direction): with a pin, a matching receipt, and a tracked
/// source older than it, the gate says OK — and its OK carries the NO-CLAIM line,
/// because a broad silent yes is what got us here.
#[test]
fn green_path_still_passes_and_states_its_limits() {
    let s = Scratch::new("green");
    for args in [
        vec!["init", "--quiet"],
        vec!["config", "user.email", "leg@test"],
        vec!["config", "user.name", "leg"],
    ] {
        let ok = Command::new("git")
            .args(&args)
            .current_dir(s.path())
            .status()
            .expect("git runs")
            .success();
        assert!(ok, "git {args:?} must succeed");
    }
    s.write("src/lib.rs", "pub fn f() {}\n");
    assert!(
        Command::new("git")
            .args(["add", "src/lib.rs"])
            .current_dir(s.path())
            .status()
            .expect("git runs")
            .success(),
        "git add must succeed"
    );
    s.write(
        "rust-toolchain.toml",
        &format!("[toolchain]\nchannel = \"{}\"\n", pinned_channel()),
    );
    // rust-toolchain.toml is untracked here on purpose: `newest_tracked_source`
    // reads `git ls-files`, so only src/lib.rs counts, and the receipt written
    // after it is genuinely newer.
    std::thread::sleep(std::time::Duration::from_millis(1100));
    s.receipt(&format!("rustc_commit={}\n", pinned_commit()));

    let (code, out) = s.verify();
    assert_eq!(code, 0, "green path must pass, got:\n{out}");
    assert!(
        out.contains("PRE_PUSH_GATE_OK"),
        "must print OK, got:\n{out}"
    );
    assert!(
        out.contains("PRE_PUSH_GATE_NO_CLAIM"),
        "OK must be accompanied by what it does NOT check, got:\n{out}"
    );
    assert!(
        out.contains("did NOT contact GitHub"),
        "the no-claim must be specific about CI, got:\n{out}"
    );
}
