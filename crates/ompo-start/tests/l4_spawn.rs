//! L4-SPAWN: WAVE.md generated from list-panes; retained hash equals file bytes.
//! A typed WAVE.md fails.

use ompo_start::{
    generate_wave, sha256_hex, spawn_retain_wave_hash, verify_retained_hash, SpawnWaveError,
};

#[test]
fn spawn_path_wave_bytes_hash_equals_retained_hash() {
    let dir = tempfile::tempdir().expect("scratch");
    let wave = dir.path().join("WAVE.md");
    let list_panes = "%1408\n%1409\n%1410\n";
    let receipt = spawn_retain_wave_hash(list_panes, &wave).expect("generate");
    let bytes = std::fs::read(&wave).expect("read WAVE.md");
    assert_eq!(
        sha256_hex(&bytes),
        receipt.wave_hash,
        "WAVE.md bytes hash must equal the hash retained on the spawn receipt"
    );
    verify_retained_hash(&receipt).expect("retained hash still matches disk");
    let text = String::from_utf8(bytes).expect("utf8");
    assert!(text.contains("%1408"));
    assert!(text.contains("%1409"));
    assert!(text.contains("never typed"));
    println!("retained {}", receipt.wave_hash);
}

#[test]
fn typed_wave_md_fails() {
    let dir = tempfile::tempdir().expect("scratch");
    let wave = dir.path().join("WAVE.md");
    let receipt = spawn_retain_wave_hash("%7\n%8\n", &wave).expect("generate");
    std::fs::write(&wave, "# I typed this WAVE.md\n").expect("type over generated file");
    match verify_retained_hash(&receipt) {
        Err(SpawnWaveError::HashMismatch { retained, live }) => {
            assert_ne!(retained, live);
            println!("typed mismatch retained={retained} live={live}");
        }
        Err(SpawnWaveError::TypedWave) => {
            println!("typed WAVE.md refused");
        }
        other => panic!("typed WAVE.md must fail, got {other:?}"),
    }

    std::fs::write(&wave, "# typed before spawn\n").expect("pre-typed");
    match generate_wave(
        &ompo_start::panes_from_list_panes("%7\n"),
        &wave,
    ) {
        Err(SpawnWaveError::TypedWave) => {}
        other => panic!("pre-existing typed WAVE.md must fail generate, got {other:?}"),
    }
}

/// L4-ARTIFACT (25yo): after the write lands, the SwarmState artifact exists
/// and readback of the required keys succeeds. The retained wave hash is
/// re-verified live before embedding. A zero-byte write and a missing file
/// both FAIL readback — neither is an empty success.
#[test]
fn swarm_state_write_readback() {
    use ompo_start::{
        read_swarm_state, write_swarm_state, PackReceipt, SwarmState,
    };

    // KNOWN-GOOD control: retained receipts round-trip with their values.
    let dir = tempfile::tempdir().expect("scratch");
    let wave = dir.path().join("WAVE.md");
    let spawn = spawn_retain_wave_hash("%7\n%8\n", &wave).expect("generate");
    let pack = PackReceipt::new("%7", b"pack-bytes");
    let mail = serde_json::json!({"status": "PRESENT"});
    let path = write_swarm_state(dir.path(), &spawn, &mail, pack.pack_sha256())
        .expect("artifact writes");
    assert!(path.exists(), "the artifact must exist after the write");
    let back = read_swarm_state(&path).expect("artifact reads back");
    assert_eq!(
        back,
        SwarmState {
            wave_hash: spawn.wave_hash.clone(),
            pack_sha256: pack.pack_sha256().to_owned(),
            mail: mail.clone(),
        },
        "readback must reproduce the retained receipts exactly"
    );
    println!(
        "L4_SWARMSTATE_OK wave={} pack={}",
        back.wave_hash, back.pack_sha256
    );

    // ZERO-BYTE write FAILS readback: empty bytes are not an empty success.
    let zero = dir.path().join("zero-state.json");
    std::fs::write(&zero, b"").expect("zero bytes land");
    let error = read_swarm_state(&zero).expect_err(
        "a zero-byte artifact must FAIL readback, not succeed",
    );
    println!("L4_SWARMSTATE_ZERO refusal={error}");

    // MISSING file FAILS readback: absence is not a row either.
    let missing = dir.path().join("never-written.json");
    assert!(!missing.exists());
    let error = read_swarm_state(&missing).expect_err(
        "a missing artifact must FAIL readback, not succeed",
    );
    println!("L4_SWARMSTATE_MISSING refusal={error}");
    assert!(
        error.contains("never-written.json"),
        "the refusal must name the missing path, got {error}"
    );
}
