use installer::{
    verify_sha256, verify_sha256_before_install, InstallError, Sha256FailureClass,
};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir(PathBuf);

impl TempDir {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before UNIX_EPOCH")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "omp-installer-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create temp directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

const B03_PAYLOAD: &[u8] = b"B03 fixed buffer SHA-256 payload\n";
const B03_DIGEST: &str = "bdd2a7291457c6a5e371324772061f751ba7774d8d7057895f5d5ea8daa773f1";
const B03_LARGE_DIGEST: &str = "c4707d7fddcaeaac63ce0019ad6bc0a74fb5c309f62093bfd53d261d4c15cdd6";

#[test]
fn sha256_known_good_returns_lowercase_digest() {
    let dir = TempDir::new("sha256-known-good");
    let source = dir.path().join("artifact");
    fs::write(&source, B03_PAYLOAD).expect("write artifact");
    let expected = B03_DIGEST.to_ascii_uppercase();
    let actual = verify_sha256(&source, Some(&expected)).expect("known digest must pass");
    assert_eq!(actual, B03_DIGEST);
}

#[test]
fn sha256_streams_across_fixed_buffer_boundary() {
    let dir = TempDir::new("sha256-buffer-boundary");
    let source = dir.path().join("artifact");
    let mut payload = vec![b'A'; 16_385];
    payload[8_192] = b'B';
    fs::write(&source, payload).expect("write large artifact");
    assert_eq!(
        verify_sha256(&source, Some(B03_LARGE_DIGEST)).expect("large digest must pass"),
        B03_LARGE_DIGEST
    );
}

#[test]
fn sha256_one_byte_mutation_refuses_before_destination_write() {
    let dir = TempDir::new("sha256-mutation");
    let source = dir.path().join("artifact");
    let destination = dir.path().join("installed");
    fs::write(&source, b"B03 fixed buffer SHA-256 payloae\n").expect("write mutated artifact");
    let error = verify_sha256(&source, Some(B03_DIGEST)).expect_err("mutation must refuse");
    let message = error.to_string();
    match error {
        InstallError::Sha256Refused { class, detail } => {
            assert_eq!(class, Sha256FailureClass::Mismatch);
            assert_eq!(
                detail,
                format!("digest mismatch path={} expected={} actual=59bad0e5db3924bd2147190a7cc8f2b9c00a0c16a356c4cb778edac262f55b84", source.display(), B03_DIGEST)
            );
        }
        other => panic!("expected SHA-256 mismatch, got {other:?}"),
    }
    assert_eq!(
        message,
        format!("L0_SHA256_REFUSED: digest mismatch path={} expected={} actual=59bad0e5db3924bd2147190a7cc8f2b9c00a0c16a356c4cb778edac262f55b84", source.display(), B03_DIGEST)
    );
    assert!(!destination.exists(), "digest refusal must not write destination");
}

#[test]
fn sha256_expected_digest_validation_is_typed_and_exact() {
    let dir = TempDir::new("sha256-expected-validation");
    let source = dir.path().join("artifact");
    fs::write(&source, B03_PAYLOAD).expect("write artifact");
    let cases = [
        (None, Sha256FailureClass::MissingExpected, "L0_SHA256_REFUSED: missing expected digest"),
        (Some("  "), Sha256FailureClass::EmptyExpected, "L0_SHA256_REFUSED: empty expected digest"),
        (
            Some("abc"),
            Sha256FailureClass::MalformedExpected,
            "L0_SHA256_REFUSED: malformed expected digest length=3; expected 64 hexadecimal characters",
        ),
    ];
    for (expected, expected_class, expected_message) in cases {
        let error = verify_sha256(&source, expected).expect_err("invalid digest must refuse");
        let message = error.to_string();
        match error {
            InstallError::Sha256Refused { class, .. } => assert_eq!(class, expected_class),
            other => panic!("expected typed SHA refusal, got {other:?}"),
        }
        assert_eq!(message, expected_message);
    }
}

#[test]
fn sha256_missing_source_refuses_with_distinct_class() {
    let dir = TempDir::new("sha256-missing-source");
    let source = dir.path().join("missing");
    let error = verify_sha256(&source, Some(B03_DIGEST)).expect_err("missing source must refuse");
    let message = error.to_string();
    match error {
        InstallError::Sha256Refused { class, detail } => {
            assert_eq!(class, Sha256FailureClass::SourceMissing);
            assert_eq!(detail, format!("source missing path={}", source.display()));
        }
        other => panic!("expected missing-source refusal, got {other:?}"),
    }
    assert_eq!(
        message,
        format!("L0_SHA256_REFUSED: source missing path={}", source.display())
    );
}

#[test]
fn sha256_read_error_refuses_with_distinct_class() {
    let dir = TempDir::new("sha256-read-error");
    let error = verify_sha256(dir.path(), Some(B03_DIGEST)).expect_err("directory read must refuse");
    let message = error.to_string();
    match error {
        InstallError::Sha256Refused { class, detail } => {
            assert_eq!(class, Sha256FailureClass::ReadFailed);
            assert!(detail.starts_with(&format!("read failure path={}: ", dir.path().display())));
        }
        other => panic!("expected read-failure refusal, got {other:?}"),
    }
    assert!(message.starts_with("L0_SHA256_REFUSED: read failure"));
}

#[test]
fn sha256_verification_seam_blocks_mismatched_install_action() {
    let dir = TempDir::new("sha256-seam-mismatch");
    let source = dir.path().join("artifact");
    let destination = dir.path().join("installed");
    fs::write(&source, b"B03 fixed buffer SHA-256 payloae\n").expect("write mutated artifact");
    let mut action_called = false;
    let error = verify_sha256_before_install(&source, Some(B03_DIGEST), || {
        action_called = true;
        fs::write(&destination, b"published")
            .map_err(|error| InstallError::IoError {
                path: destination.display().to_string(),
                detail: error.to_string(),
            })
    })
    .expect_err("mismatched digest must block install action");
    assert!(matches!(
        error,
        InstallError::Sha256Refused {
            class: Sha256FailureClass::Mismatch,
            ..
        }
    ));
    assert!(!action_called, "install action ran after digest refusal");
    assert!(!destination.exists(), "mismatched digest wrote destination");
}

#[test]
fn sha256_verification_seam_allows_matching_install_action() {
    let dir = TempDir::new("sha256-seam-match");
    let source = dir.path().join("artifact");
    let destination = dir.path().join("installed");
    fs::write(&source, B03_PAYLOAD).expect("write artifact");
    let mut action_called = false;
    verify_sha256_before_install(&source, Some(B03_DIGEST), || {
        action_called = true;
        fs::write(&destination, b"published")
            .map_err(|error| InstallError::IoError {
                path: destination.display().to_string(),
                detail: error.to_string(),
            })
    })
    .expect("matching digest must allow install action");
    assert!(action_called, "matching digest did not reach install action");
    assert_eq!(fs::read(&destination).expect("published destination"), b"published");
}
