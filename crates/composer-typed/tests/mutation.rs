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

#[test]
fn mutation_fail_closed_on_empty() {
    let on = rc("", &[]);
    assert_eq!(on, 1, "empty capture is not typed");
    println!("MUTATION RED fail_closed_on_empty: rc=1 on empty stdin");
}
