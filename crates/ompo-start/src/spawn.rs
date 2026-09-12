#![forbid(unsafe_code)]

//! L4-SPAWN WAVE.md: generated from `tmux list-panes` output, never typed.
//! The spawn receipt retains the SHA-256 of the file bytes. A typed WAVE.md
//! cannot match that digest.

use sha2::{Digest, Sha256};
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Pane id as `tmux list-panes -F '#{pane_id}'` emits it (`%N`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneId(pub String);

/// Spawn receipt. `wave_hash` is SHA-256 of the WAVE.md bytes on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpawnReceipt {
    pub wave_path: PathBuf,
    pub wave_hash: String,
}

#[derive(Debug)]
pub enum SpawnWaveError {
    EmptyPaneList,
    Io(io::Error),
    HashMismatch { retained: String, live: String },
    TypedWave,
}

impl fmt::Display for SpawnWaveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPaneList => write!(f, "WAVE_EMPTY_PANES: list-panes produced no pane ids"),
            Self::Io(err) => write!(f, "WAVE_IO: {err}"),
            Self::HashMismatch { retained, live } => {
                write!(f, "WAVE_HASH_MISMATCH retained={retained} live={live}")
            }
            Self::TypedWave => {
                write!(f, "WAVE_TYPED: WAVE.md bytes were not generated from list-panes")
            }
        }
    }
}

impl std::error::Error for SpawnWaveError {}

impl From<io::Error> for SpawnWaveError {
    fn from(err: io::Error) -> Self {
        Self::Io(err)
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(64);
    for byte in digest {
        use fmt::Write as _;
        let _ = write!(&mut encoded, "{byte:02x}");
    }
    encoded
}

/// Parse `tmux list-panes -F '#{pane_id}'` stdout. Not a liveness source.
pub fn panes_from_list_panes(stdout: &str) -> Vec<PaneId> {
    stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(|line| PaneId(line.to_owned()))
        .collect()
}

fn render_wave(panes: &[PaneId]) -> Vec<u8> {
    let mut out = String::from("# WAVE\n# generated from tmux list-panes; never typed\n");
    for pane in panes {
        out.push_str(&pane.0);
        out.push('\n');
    }
    out.into_bytes()
}

/// Write WAVE.md from the pane list and retain the bytes hash on the receipt.
pub fn generate_wave(panes: &[PaneId], dest: &Path) -> Result<SpawnReceipt, SpawnWaveError> {
    if panes.is_empty() {
        return Err(SpawnWaveError::EmptyPaneList);
    }
    let bytes = render_wave(panes);
    if let Ok(existing) = fs::read(dest) {
        if existing != bytes {
            return Err(SpawnWaveError::TypedWave);
        }
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(dest, &bytes)?;
    Ok(SpawnReceipt {
        wave_path: dest.to_path_buf(),
        wave_hash: sha256_hex(&bytes),
    })
}

/// Re-hash WAVE.md bytes and require equality with the retained receipt hash.
pub fn verify_retained_hash(receipt: &SpawnReceipt) -> Result<(), SpawnWaveError> {
    let bytes = fs::read(&receipt.wave_path)?;
    let live = sha256_hex(&bytes);
    if live != receipt.wave_hash {
        return Err(SpawnWaveError::HashMismatch {
            retained: receipt.wave_hash.clone(),
            live,
        });
    }
    let expected = render_wave(&panes_from_list_panes(&wave_pane_lines(&bytes)));
    if bytes != expected {
        return Err(SpawnWaveError::TypedWave);
    }
    Ok(())
}

fn wave_pane_lines(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes)
        .lines()
        .filter(|line| line.starts_with('%'))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Spawn-path test seam: generate WAVE.md from list-panes stdout and retain the hash.
pub fn spawn_retain_wave_hash(
    list_panes_stdout: &str,
    dest: &Path,
) -> Result<SpawnReceipt, SpawnWaveError> {
    generate_wave(&panes_from_list_panes(list_panes_stdout), dest)
}

/// L4-SPAWN (ol44): the `ntm spawn --assign --cass-context` gate.
///
/// Spawn proceeds only when the swarm is NotLive AND the HD-0010 row is
/// decided. Both refusals are typed [`SpawnGate::Refused`] values carrying a
/// named reason — never a silent skip, never an `Ok` with nothing behind it.
/// `hd0010_decided` is the resolved decidedness of the HD-0010 ledger row
/// (see `hd0009` for the resolution shape); this gate does not read the
/// ledger itself, so it stays decidable on any lane, including a worker with
/// no checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnGate {
    /// Spawn may proceed.
    Allowed,
    /// Spawn is refused; `reason` names the bar that failed.
    Refused { reason: String },
}

/// The single authority for whether a spawn may proceed.
#[must_use]
pub fn spawn_gate(verdict: &crate::liveness::LiveVerdict, hd0010_decided: bool) -> SpawnGate {
    if verdict.is_live() {
        return SpawnGate::Refused {
            reason: "L4_SPAWN_REFUSED_LIVE — the swarm is live; spawn is the NotLive recovery path"
                .to_owned(),
        };
    }
    if !hd0010_decided {
        return SpawnGate::Refused {
            reason: "L4_SPAWN_REFUSED_HD0010_UNDECIDED — HD-0010 records no decided agent mix; refusing spawn without HD-0010"
                .to_owned(),
        };
    }
    SpawnGate::Allowed
}
