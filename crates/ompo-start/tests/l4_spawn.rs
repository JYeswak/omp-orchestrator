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
