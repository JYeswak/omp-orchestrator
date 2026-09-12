//! The crate-contract row block, DERIVED — the generator `docs/inventories/
//! crate_contract_inventory.md` has always claimed to have and never had.
//!
//! # Why this module exists
//!
//! That document says, in its own words, "the row block is generated from Cargo
//! metadata" and "package membership comes from Cargo metadata, never a
//! hand-written roster". Measured 2026-09-12: NOTHING in the repository generates
//! it. `CRATE-CONTRACT-ROWS` appears in exactly two places — the document and the
//! test that checks it — so the block is hand-maintained by a file that says it is
//! not, and it had drifted to 72 rows against 94 workspace packages. Twenty-three
//! packages carried no row, which is what kept
//! `every_metadata_package_has_exactly_one_inventory_row` red, and with it
//! `planted_package_is_red_until_its_row_exists`, whose known-good restore runs
//! through the same document.
//!
//! This is the third hand-maintained registry beside a derivable source measured
//! in one night (`docs/gate-roster.txt` 90 rows against 94 members;
//! `HOOK_SOURCE_CRATES` 5 watched against 17 path deps, having re-acquired its own
//! predicted drift three times). A hand-maintained registry beside a derivable
//! source is not a bookkeeping lapse; it is a defect with a known period.
//!
//! # What is derived, and what is NEVER rewritten
//!
//! The derivation rules are NOT invented here — they are the document's own
//! `CRI-*` rules, implemented:
//!
//! * `CRI-ROSTER-METADATA` — membership is the workspace package set.
//! * `CRI-IO-SOURCE` — input/output cells report only markers found in the source
//!   scan; `UNDECLARED` is the honest result when evidence is absent.
//! * `CRI-TYPED-SURFACE` — public declarations are listed when the scan exposes
//!   them; a name is never turned into a semantic.
//! * `CRI-KERNEL-ROUTE` — a path dependency on `subprocess-contract` or
//!   `oracle-compare` is recorded; a raw `Command::new` is a handroll; silence
//!   infers nothing.
//!
//! ⛔ EXISTING ROWS ARE PRESERVED BYTE-FOR-BYTE. [`merge_block`] only ever INSERTS
//! rows for packages that have none. Several existing rows carry prose no scan can
//! produce ("process request supplied to kernel; kernel-routed br") — that is a
//! human's measurement, and regenerating over it would replace a record of what
//! was observed with a record of what a scanner can see. The same reason a
//! captured ledger value is never rewritten to satisfy a lint.
//!
//! # NO-CLAIM
//!
//! A derived cell reports MARKERS, not behaviour: `env` means the source names
//! `env::var`, not that the variable matters. The route cell reads manifests and
//! `Command::new` sites, so a spawn reached through a helper crate reads as
//! `UNDECLARED route` rather than as a handroll — absence of a marker is not
//! evidence of absence, and that is why the honest cell is `UNDECLARED`.

use std::collections::BTreeSet;
use std::path::Path;

/// One inventory row, in the document's column order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractRow {
    pub crate_name: String,
    pub inputs: String,
    pub outputs: String,
    pub typed: String,
    pub route: String,
}

/// Exit-code semantics live in ONE registry (`CRI-EXIT-REGISTRY`); every row
/// points at it rather than carrying a second table.
const EXIT_REGISTRY: &str = "exit codes: `docs/error_codes/exit_code_registry.md`";

/// The honest cell when the scan found no evidence.
const UNDECLARED: &str = "UNDECLARED";

/// How many public declarations a row lists before it stops being a row and
/// starts being a dump. Matches the density of the rows already in the document.
const TYPED_CELL_LIMIT: usize = 5;

impl ContractRow {
    /// The document's pipe-table line for this row.
    #[must_use]
    pub fn render(&self) -> String {
        format!(
            "| `{}` | {} | {} | {} | {} |",
            self.crate_name, self.inputs, self.outputs, self.typed, self.route
        )
    }
}

/// The crate name a row names, or `None` for any line that is not a row.
#[must_use]
pub fn row_name(line: &str) -> Option<&str> {
    line.strip_prefix("| `")
        .and_then(|rest| rest.split_once('`'))
        .map(|(name, _)| name)
}

fn marker_cell(markers: &[(&str, bool)]) -> String {
    let present: Vec<&str> = markers
        .iter()
        .filter(|(_, found)| *found)
        .map(|(label, _)| *label)
        .collect();
    if present.is_empty() {
        UNDECLARED.to_owned()
    } else {
        present.join(", ")
    }
}

/// Derive one row for `crate_name` from its manifest and sources.
///
/// `sources` is the concatenated `src/**/*.rs` text; `manifest` its `Cargo.toml`.
/// Taking both as TEXT rather than a path is deliberate: the derivation is then a
/// pure function that a fixture can drive, and it cannot read the wrong tree.
#[must_use]
pub fn derive_row(crate_name: &str, manifest: &str, sources: &str) -> ContractRow {
    let inputs = marker_cell(&[
        ("argv", sources.contains("env::args") || sources.contains("args_os")),
        ("stdin", sources.contains("stdin")),
        ("env", sources.contains("env::var")),
        (
            "files",
            sources.contains("read_to_string")
                || sources.contains("fs::read")
                || sources.contains("File::open"),
        ),
    ]);
    let outputs_found = marker_cell(&[
        (
            "stdout",
            sources.contains("println!") || sources.contains("print!") || sources.contains("stdout"),
        ),
        (
            "files written",
            sources.contains("fs::write") || sources.contains("File::create"),
        ),
        (
            "exit/status",
            sources.contains("ExitCode") || sources.contains("process::exit"),
        ),
    ]);
    let typed = {
        let mut names: Vec<String> = Vec::new();
        for keyword in ["pub struct ", "pub enum ", "pub const ", "pub fn "] {
            for (index, _) in sources.match_indices(keyword) {
                // Only top-of-line declarations: an occurrence inside a string or
                // after `impl` is not a declaration of this crate's surface.
                let line_start = sources[..index].rfind('\n').map_or(0, |nl| nl + 1);
                if sources[line_start..index].trim() != "" {
                    continue;
                }
                let rest = &sources[index + keyword.len()..];
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '_')
                    .collect();
                if !name.is_empty() && !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        if names.is_empty() {
            UNDECLARED.to_owned()
        } else {
            names.truncate(TYPED_CELL_LIMIT);
            names.join(", ")
        }
    };
    let route = if manifest.contains("subprocess-contract") {
        "routes subprocess-contract".to_owned()
    } else if manifest.contains("oracle-compare") {
        "routes oracle-compare".to_owned()
    } else if sources.contains("Command::new") {
        "handroll Command::new".to_owned()
    } else {
        "UNDECLARED route".to_owned()
    };
    ContractRow {
        crate_name: crate_name.to_owned(),
        inputs,
        outputs: format!("{outputs_found}; {EXIT_REGISTRY}"),
        typed,
        route,
    }
}

/// Read a crate's manifest and concatenated sources off disk, then derive its row.
///
/// Errors are typed rather than defaulted: a package whose manifest cannot be read
/// must not silently receive an `UNDECLARED` row, because that row would report a
/// measurement that never happened.
pub fn derive_row_from_disk(
    repo_root: &Path,
    crate_name: &str,
) -> Result<ContractRow, crate::InventoryError> {
    let dir = repo_root.join("crates").join(crate_name);
    let manifest = std::fs::read_to_string(dir.join("Cargo.toml")).map_err(|error| {
        crate::InventoryError::InvalidInput(format!(
            "crate {crate_name}: manifest unreadable: {error}"
        ))
    })?;
    let mut sources = String::new();
    let mut stack = vec![dir.join("src")];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).map_err(|error| {
                    crate::InventoryError::InvalidInput(format!(
                        "crate {crate_name}: {} unreadable: {error}",
                        path.display()
                    ))
                })?;
                sources.push_str(&text);
                sources.push('\n');
            }
        }
    }
    Ok(derive_row(crate_name, &manifest, &sources))
}

/// Insert a row for every package that has none, preserving every existing line.
///
/// Rows are inserted in the block's sorted position so the table stays readable;
/// existing lines are copied verbatim, including ones a scan could never produce.
/// An empty package set is an ERROR, never a clean pass — a block validated
/// against nothing is the vacuity this whole inventory exists to prevent.
pub fn merge_block(
    block: &str,
    packages: &BTreeSet<String>,
    derive: &dyn Fn(&str) -> Result<ContractRow, crate::InventoryError>,
) -> Result<String, crate::InventoryError> {
    if packages.is_empty() {
        return Err(crate::InventoryError::InvalidInput(
            "anti-vacuity: the package set is empty, so a merged block would assert \
             coverage of nothing"
                .to_owned(),
        ));
    }
    let mut lines: Vec<String> = block
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(str::to_owned)
        .collect();
    let present: BTreeSet<String> = lines
        .iter()
        .filter_map(|line| row_name(line))
        .map(str::to_owned)
        .collect();
    for name in packages.difference(&present) {
        let row = derive(name)?.render();
        // Sorted insertion by crate name, so the block does not become
        // append-ordered the moment it is repaired once.
        let at = lines
            .iter()
            .position(|line| row_name(line).is_some_and(|existing| existing > name.as_str()))
            .unwrap_or(lines.len());
        lines.insert(at, row);
    }
    Ok(format!("\n{}\n", lines.join("\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|n| (*n).to_owned()).collect()
    }

    /// The derivation reports MARKERS, both when they are there and when they are
    /// not — a cell that is never `UNDECLARED` is a cell that is guessing.
    #[test]
    fn a_cell_reports_what_the_scan_found_and_says_so_when_it_found_nothing() {
        let loud = derive_row(
            "loud",
            "[dependencies]\nsubprocess-contract = { path = \"../subprocess-contract\" }\n",
            "pub struct Thing;\nfn main() { let _ = std::env::args(); println!(\"x\"); }\n",
        );
        assert_eq!(loud.inputs, "argv");
        assert!(loud.outputs.starts_with("stdout; exit codes:"), "{}", loud.outputs);
        assert_eq!(loud.typed, "Thing");
        assert_eq!(loud.route, "routes subprocess-contract");

        let silent = derive_row("silent", "[package]\nname = \"silent\"\n", "fn helper() {}\n");
        assert_eq!(silent.inputs, UNDECLARED);
        assert_eq!(silent.typed, UNDECLARED);
        assert_eq!(silent.route, "UNDECLARED route");
        assert!(
            silent.outputs.starts_with("UNDECLARED; exit codes:"),
            "an absent output marker must read UNDECLARED, not empty: {}",
            silent.outputs
        );
    }

    /// A `Command::new` with no kernel dependency is a HANDROLL, and a route is
    /// never inferred from silence (`CRI-KERNEL-ROUTE`).
    #[test]
    fn a_raw_spawn_is_a_handroll_and_silence_infers_nothing() {
        let handroll = derive_row("h", "[package]\n", "fn go() { Command::new(\"git\"); }\n");
        assert_eq!(handroll.route, "handroll Command::new");
        let quiet = derive_row("q", "[package]\n", "pub enum E { A }\n");
        assert_eq!(quiet.route, "UNDECLARED route");
    }

    /// A declaration that FOLLOWS another token on its line is not a declaration
    /// of this crate's surface; an INDENTED one still is.
    ///
    /// This leg asserted the opposite when it was written ("an indented or
    /// trailing declaration must not enter the surface cell") and the
    /// implementation refuted it: `pub mod api { pub struct Row; }` nests real
    /// public surface behind whitespace, so excluding indentation would hide
    /// exactly the types a consumer imports. What must stay out is
    /// `impl T { pub fn m() }` — a method on the same line as its `impl`, which
    /// is an inherent item and not a top-level declaration. The predicate is
    /// therefore "nothing but whitespace before the keyword", and the premise
    /// this leg started with was the wrong one.
    #[test]
    fn a_trailing_declaration_is_excluded_and_a_nested_one_is_not() {
        let row = derive_row(
            "x",
            "[package]\n",
            "pub struct Real;\nimpl Real { pub fn method(&self) {} }\npub mod api {\n    pub struct Nested;\n}\n",
        );
        assert!(
            row.typed.contains("Real") && row.typed.contains("Nested"),
            "a nested public declaration is still public surface: {}",
            row.typed
        );
        assert!(
            !row.typed.contains("method"),
            "an inherent item sharing a line with its `impl` is not top-level surface: {}",
            row.typed
        );
    }

    /// MERGE PRESERVES. The existing lines come back byte-for-byte, including
    /// prose no scanner could produce, and only the missing package is added.
    #[test]
    fn merging_inserts_the_missing_row_and_rewrites_none() {
        let curated = "| `alpha` | argv | stdout; exit codes: `x` | A | process request supplied to kernel; kernel-routed br |";
        let block = format!("\n{curated}\n");
        let merged = merge_block(&block, &set(&["alpha", "beta"]), &|name| {
            Ok(derive_row(name, "[package]\n", "pub struct B;\n"))
        })
        .expect("merge works");
        assert!(
            merged.contains(curated),
            "the curated row must survive byte-for-byte: {merged}"
        );
        assert!(merged.contains("| `beta` |"), "the missing row must be added: {merged}");
        assert_eq!(
            merged.lines().filter(|l| l.starts_with("| `")).count(),
            2,
            "exactly one row per package: {merged}"
        );

        // IDEMPOTENCE is the property the gate rides on: a complete block merges
        // to itself, so a diff against the merge is a drift detector.
        let again = merge_block(&merged, &set(&["alpha", "beta"]), &|_| {
            panic!("a complete block must not derive anything")
        })
        .expect("merge is idempotent");
        assert_eq!(again, merged, "merging a complete block must change nothing");
    }

    /// Sorted insertion, so one repair does not turn the table into an append log.
    #[test]
    fn a_repaired_row_lands_in_sorted_position() {
        let block = "\n| `alpha` | a | b | c | d |\n| `gamma` | a | b | c | d |\n";
        let merged = merge_block(&block, &set(&["alpha", "beta", "gamma"]), &|name| {
            Ok(derive_row(name, "[package]\n", ""))
        })
        .expect("merge works");
        let order: Vec<&str> = merged.lines().filter_map(row_name).collect();
        assert_eq!(order, vec!["alpha", "beta", "gamma"], "{merged}");
    }

    /// ANTI-VACUITY: an empty package set is an ERROR. A block "validated"
    /// against nothing would report full coverage of a roster that does not exist.
    #[test]
    fn an_empty_package_set_is_an_error_not_an_untouched_block() {
        let error = merge_block("\n| `alpha` | a | b | c | d |\n", &set(&[]), &|_| {
            panic!("nothing to derive")
        })
        .expect_err("an empty package set must refuse");
        assert!(
            format!("{error:?}").contains("anti-vacuity"),
            "the refusal must name its class: {error:?}"
        );
    }

    /// An unreadable crate is a TYPED error, never a default row: a row that says
    /// `UNDECLARED` when the scan never ran reports a measurement that did not happen.
    #[test]
    fn an_unreadable_crate_is_a_typed_error_not_a_default_row() {
        let root = std::env::temp_dir().join(format!("crate-contract-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("crates")).expect("root");
        let error = derive_row_from_disk(&root, "absent-crate")
            .expect_err("a crate with no manifest must refuse");
        assert!(
            format!("{error:?}").contains("absent-crate"),
            "the refusal must name the crate: {error:?}"
        );
        std::fs::remove_dir_all(&root).expect("cleanup");
    }
}
