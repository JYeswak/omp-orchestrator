use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

fn rust_bin() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_composer-typed"))
}

fn rc(input: &str, extra: &[&str]) -> i32 {
    let mut child = Command::new(rust_bin())
        .args(extra)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait().unwrap().code().unwrap_or(99)
}

#[test]
fn mutation_dim_suggestion_is_not_typed() {
    let esc = "\u{1b}";
    let input = format!("Opus\n{esc}[39m❯ {esc}[2mfix kyzn{esc}[0m\n");
    let off = rc(
        &input,
        &[
            "--mutation",
            "--disable-rule",
            "dim_suggestion_is_not_typed",
        ],
    );
    assert_eq!(off, 0, "disabled dim rule treats suggestion as typed");
    println!("MUTATION dim_suggestion_is_not_typed disabled -> rc=0 (greyed suggestion classified typed)");

    let on = rc(&input, &[]);
    assert_eq!(
        on, 1,
        "rule dim_suggestion_is_not_typed: suggestion is FREE"
    );
    println!("MUTATION RED dim_suggestion_is_not_typed: rc=1 (greyed autosuggestion is not typed)");
}

#[test]
fn mutation_bright_body_is_typed() {
    let input = "❯ bought credits - resume the fleet\n";
    let off = rc(
        input,
        &["--mutation", "--disable-rule", "bright_body_is_typed"],
    );
    assert_eq!(off, 1, "disabled bright_body treats typed text as free");
    println!(
        "MUTATION bright_body_is_typed disabled -> rc=1 (typed operator text classified free)"
    );

    let on = rc(input, &[]);
    assert_eq!(on, 0);
    println!("MUTATION RED bright_body_is_typed: rc=0 (typed operator text is TYPED)");
}

/// The empty capture's verdict, asserted against the HARD-CODED behaviour it actually comes from.
///
/// RENAMED from `mutation_fail_closed_on_empty` and no longer claiming to mutate anything. The
/// `fail_closed_on_empty` RULE was deleted in this commit: it was declared, defaulted true,
/// disableable by name, and READ BY NOTHING, while `is_typed` hard-codes the empty case at
/// lib.rs:189-191. Its two siblings ARE read (:163, :171, :177, :184), which is what made the
/// dead one look alive.
///
/// PROVEN BEFORE DELETING, the way Main asked: flipping the default from `true` to `false` left
/// THIS TEST PASSING (`test mutation_fail_closed_on_empty ... ok`, 1 passed). A test carrying a
/// flag's name asserted the hard-code instead, so the name was not an assertion -- and the name
/// is precisely what made the flag look verified.
///
/// ⛔ AND THE NAME WAS BACKWARDS ANYWAY. `main.rs:3` maps 0=TYPED, 1=FREE, so "fail closed on
/// empty" yields rc=1 = FREE = ADMIT. Correct for an OCCUPANCY question (an empty capture holds
/// no typed text) and inverted for a DISPATCH question, which is the polarity now documented on
/// `is_typed` itself.
#[test]
fn an_empty_capture_is_not_typed_and_therefore_reads_free() {
    let on = rc("", &[]);
    assert_eq!(on, 1, "empty capture is not typed, so the binary reports FREE (rc=1)");
    println!("empty stdin -> rc=1 (FREE); the caller, not this crate, decides if that is safe");
}
