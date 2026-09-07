//! Uncaptured platform-scope reporter for the process-group test property.
//!
//! The four unit tests in `src/lib.rs` remain real tests on every platform. This
//! harness-free target gives the operator a durable, uncaptured verdict: Linux
//! is explicitly UNMEASURED with a distinct code, while Darwin is the only
//! platform on which the group-kill/reap property is eligible for verification.

const PROPERTY: &str = "process_group_kill_and_reap";
const TEST_COUNT: usize = 4;
const UNMEASURED_EXIT_CODE: i32 = 20;

fn main() {
    if cfg!(target_os = "macos") {
        println!(
            "VERIFIED_ON_THIS_PLATFORM property={PROPERTY} platform=darwin tests={TEST_COUNT} \
             exit_code=0"
        );
        return;
    }

    println!(
        "UNMEASURED_ON_THIS_PLATFORM property={PROPERTY} platform={} tests={TEST_COUNT} \
         exit_code={UNMEASURED_EXIT_CODE} retry_if=darwin-production-group-kill-reap-defect",
        std::env::consts::OS
    );
    std::process::exit(UNMEASURED_EXIT_CODE);
}
