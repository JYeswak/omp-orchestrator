//! ARTIFACT PROVENANCE GATE — every preserved inventory artifact must exist and
//! still hash to the value the plan cites.
//!
//! # The measured failure
//!
//! 2026-09-01. `04-diagrams.md` cited `/tmp/inv.txt` five times as the source for
//! its diagrams. The file existed, so the citations "worked" — but `/tmp` is
//! cleared on reboot, which means every provenance claim in that section had a
//! lifetime measured in days, after which nobody, including its author, could check
//! any of them.
//!
//! `GradeDiagrams` filed the *staleness* (the diagrams use a 16:50 capture while the
//! brief cites a 23:01 one, 5.6× larger). The larger problem was underneath it: a
//! citation to a temp path is a citation to nothing, on a delay.
//!
//! # What this enforces
//!
//! Each row below names an artifact preserved under `.flywheel/inventory-artifacts/`
//! and the SHA-256 prefix the plan quotes for its *uncompressed* content. The gate
//! decompresses and re-hashes. A missing artifact fails; a drifted hash fails.
//!
//! # Why the hash and not just existence
//!
//! Existence alone lets a file be replaced by a different capture under the same
//! name, which is precisely the substitution this repo has already been burned by:
//! a wiring scan matched a vendored `serde_json` copy instead of the real tree, and
//! a lint measurement read a 13-hour-old binary because `find … | head -1` returned
//! whichever path sorted first. Name-only identity is not identity.
//!
//! # What it cannot do
//!
//! It does not prove the artifact is the *right* capture for the claim built on it —
//! only that the bytes are the ones the plan hashed. §4.7 states plainly that the
//! diagrams reflect the 16:50 snapshot and are not current; this gate keeps that
//! statement honest, it does not make the diagrams fresh.

use std::io::Read;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crates/<name> has a workspace root two levels up")
        .to_path_buf()
}

/// (preserved gz file, sha256 prefix of the UNCOMPRESSED bytes, what cites it)
const PRESERVED: &[(&str, &str, &str)] = &[
    (
        "inv.txt.gz",
        "86491732a5581a6d",
        "04-diagrams.md — the 16:50 capture the diagrams are actually built from",
    ),
    (
        "omp-inventory-map-2026-08-31.json.gz",
        "876809f0779a81b3",
        "00-brief.md §3.2 — the 23:01 capture, 981 census rows",
    ),
];

fn sha256_hex_prefix(bytes: &[u8], n: usize) -> String {
    // Minimal SHA-256. Vendoring a crate for a gate that must run in the smallest
    // possible dependency set is not worth the supply-chain surface, and this repo
    // already records an unmeasured cargo shim in its own dependency path.
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];
    let mut msg = bytes.to_vec();
    let bitlen = (bytes.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bitlen.to_be_bytes());
    for chunk in msg.chunks(64) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([chunk[4 * i], chunk[4 * i + 1], chunk[4 * i + 2], chunk[4 * i + 3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut v = h;
        for i in 0..64 {
            let s1 = v[4].rotate_right(6) ^ v[4].rotate_right(11) ^ v[4].rotate_right(25);
            let ch = (v[4] & v[5]) ^ ((!v[4]) & v[6]);
            let t1 = v[7]
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = v[0].rotate_right(2) ^ v[0].rotate_right(13) ^ v[0].rotate_right(22);
            let maj = (v[0] & v[1]) ^ (v[0] & v[2]) ^ (v[1] & v[2]);
            let t2 = s0.wrapping_add(maj);
            v[7] = v[6];
            v[6] = v[5];
            v[5] = v[4];
            v[4] = v[3].wrapping_add(t1);
            v[3] = v[2];
            v[2] = v[1];
            v[1] = v[0];
            v[0] = t1.wrapping_add(t2);
        }
        for i in 0..8 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    let mut s = String::new();
    for x in h {
        s.push_str(&format!("{x:08x}"));
    }
    s.chars().take(n).collect()
}

fn gunzip(path: &Path) -> Option<Vec<u8>> {
    // Shell out to gunzip rather than vendor a decompressor. This is a test, the
    // input is a file we wrote, and `subprocess-contract` is not needed for a
    // bounded read under the harness's own deadline.
    let out = std::process::Command::new("gzip")
        .args(["-dc", path.to_str()?])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let mut v = Vec::new();
    out.stdout.as_slice().read_to_end(&mut v).ok()?;
    Some(v)
}

#[test]
fn every_preserved_artifact_exists_and_matches_its_cited_hash() {
    let root = repo_root();
    let dir = root.join(".flywheel/inventory-artifacts");
    assert!(
        !PRESERVED.is_empty(),
        "ANTI-VACUITY: no preserved artifacts declared — the registry is empty, which \
         is not the same as verified"
    );
    let mut problems = Vec::new();
    let mut checked = 0usize;
    for (file, want, cited_by) in PRESERVED {
        let p = dir.join(file);
        if !p.is_file() {
            problems.push(format!("{file} MISSING (cited by {cited_by})"));
            continue;
        }
        let Some(bytes) = gunzip(&p) else {
            problems.push(format!("{file} could not be decompressed"));
            continue;
        };
        checked += 1;
        let got = sha256_hex_prefix(&bytes, want.len());
        if got != *want {
            problems.push(format!(
                "{file} hash drift: cited {want}, got {got} (cited by {cited_by})"
            ));
        }
    }
    assert!(
        checked > 0,
        "ANTI-VACUITY: zero artifacts were actually hashed — every row failed before \
         the comparison, so this proves nothing about content"
    );
    assert!(
        problems.is_empty(),
        "{} preserved artifact problem(s):\n{:#?}\n\n\
         A plan citing /tmp is a plan citing nothing after the next reboot. These \
         copies are the provenance; if one drifts, the claim built on it is unmoored.",
        problems.len(),
        problems
    );
}
