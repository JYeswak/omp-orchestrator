use super::*;

const DIGEST: &str = "bdd2a7291457c6a5e371324772061f751ba7774d8d7057895f5d5ea8daa773f1";
const PAYLOAD: &[u8] = b"B03 fixed buffer SHA-256 payload\n";

fn install_args(digest: &str, bin_dir: &str, equals: bool) -> Vec<String> {
    let mut args = vec![
        "--install".to_owned(),
        "installer".to_owned(),
        "--bin-dir".to_owned(),
        bin_dir.to_owned(),
    ];
    if equals {
        args.push(format!("--sha256={digest}"));
    } else {
        args.extend(["--sha256".to_owned(), digest.to_owned()]);
    }
    args
}

fn control_args(flag: &str, digest: &str, equals: bool) -> Vec<String> {
    let mut args = vec![flag.to_owned()];
    if equals {
        args.push(format!("--sha256={digest}"));
    } else {
        args.extend(["--sha256".to_owned(), digest.to_owned()]);
    }
    args
}

#[test]
fn parser_preserves_dispatch_and_digest_precedence() {
    let value = parse_cli_args(install_args(DIGEST, "scratch-home", false)).expect("value form");
    let equals = parse_cli_args(install_args(DIGEST, "scratch-home", true)).expect("equals form");
    assert_eq!(value.positional, vec!["--install".to_owned(), "installer".to_owned()]);
    assert_eq!(value.bin_dir, PathBuf::from("scratch-home"));
    assert_eq!(value.expected_sha256.as_deref(), Some(DIGEST));
    assert_eq!(equals.expected_sha256, value.expected_sha256);

    let mut duplicate = install_args("bad", "scratch-home", false);
    duplicate.push(format!("--sha256={DIGEST}"));
    assert_eq!(parse_cli_args(duplicate).expect("duplicate flags").expected_sha256.as_deref(), Some(DIGEST));

    for (flag, equals_form) in [("--check", true), ("--version", false)] {
        let parsed = parse_cli_args(control_args(flag, DIGEST, equals_form)).expect("control flag");
        assert_eq!(parsed.positional, vec![flag]);
        assert_eq!(parsed.expected_sha256.as_deref(), Some(DIGEST));
    }
}

#[test]
fn parsed_digest_reaches_production_verification_action() {
    let source = std::env::temp_dir().join(format!("omp-installer-b03-cli-{}", std::process::id()));
    std::fs::write(&source, PAYLOAD).expect("write artifact");
    let parsed = parse_cli_args(install_args(DIGEST, source.to_str().expect("UTF-8 path"), false))
        .expect("production arguments");
    let mut action_called = false;
    installer::verify_sha256_before_install(&source, parsed.expected_sha256.as_deref(), || {
        action_called = true;
        Ok::<(), installer::InstallError>(())
    })
    .expect("parsed digest reaches action");
    assert!(action_called, "matching parsed digest did not reach action");
    std::fs::remove_file(source).expect("cleanup artifact");
}
