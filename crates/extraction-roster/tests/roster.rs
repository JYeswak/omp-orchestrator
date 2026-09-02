use extraction_roster::{build_roster, Dependency, Metadata, Package, RosterError};
use std::fs;

fn package(name: &str, manifest_path: &str, dependencies: &[&str]) -> Package {
    Package {
        id: format!("{name} 0.1.0 path+file://{manifest_path}"),
        name: name.to_owned(),
        manifest_path: manifest_path.to_owned(),
        dependencies: dependencies
            .iter()
            .map(|dependency| Dependency {
                name: (*dependency).to_owned(),
                path: Some(format!("/source/crates/{dependency}")),
            })
            .collect(),
    }
}

fn metadata(packages: Vec<Package>) -> Metadata {
    Metadata {
        packages,
        workspace_members: Vec::new(),
    }
}

#[test]
fn metadata_diff_names_source_only_targets_and_terminal_candidates() {
    let temp = tempfile_path("roster-diff");
    fs::create_dir_all(temp.join("source/crates/terminal/src")).unwrap();
    fs::create_dir_all(temp.join("source/crates/port/src")).unwrap();
    fs::create_dir_all(temp.join("target/crates/shared/src")).unwrap();
    fs::write(
        temp.join("source/crates/terminal/src/lib.rs"),
        "const OWNER: &str = \"/Users/josh/Developer/other-repo\";\n",
    )
    .unwrap();
    fs::write(
        temp.join("source/crates/port/src/lib.rs"),
        "pub fn port() {}\n",
    )
    .unwrap();
    fs::write(
        temp.join("target/crates/shared/src/lib.rs"),
        "pub fn shared() {}\n",
    )
    .unwrap();

    let source = metadata(vec![
        package(
            "terminal",
            &temp
                .join("source/crates/terminal/Cargo.toml")
                .display()
                .to_string(),
            &[],
        ),
        package(
            "port",
            &temp
                .join("source/crates/port/Cargo.toml")
                .display()
                .to_string(),
            &[],
        ),
        package(
            "shared",
            &temp
                .join("source/crates/shared/Cargo.toml")
                .display()
                .to_string(),
            &[],
        ),
    ]);
    let target = metadata(vec![package(
        "shared",
        &temp
            .join("target/crates/shared/Cargo.toml")
            .display()
            .to_string(),
        &[],
    )]);

    let roster = build_roster(&source, &target, "omp-orchestrator", "control-plane").unwrap();
    assert_eq!(roster.source_package_count, 3);
    assert_eq!(roster.target_package_count, 1);
    assert_eq!(roster.overlap_names(), vec!["shared"]);
    assert_eq!(roster.terminal_crates, vec!["terminal"]);
    assert_eq!(roster.extraction_targets, vec!["port"]);
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn metadata_topology_counts_path_dependencies_without_text_scans() {
    let source = metadata(vec![
        package("leaf", "/source/crates/leaf/Cargo.toml", &[]),
        package(
            "dependent",
            "/source/crates/dependent/Cargo.toml",
            &["leaf"],
        ),
    ]);
    let target = metadata(Vec::new());

    let roster = build_roster(&source, &target, "omp-orchestrator", "control-plane").unwrap();
    assert_eq!(roster.zero_path_dependency_crates, 1);
    assert_eq!(roster.path_dependency_edges, 1);
    assert_eq!(roster.leaf_names(), vec!["leaf"]);
}

#[test]
fn empty_extraction_targets_are_an_error() {
    let temp = tempfile_path("empty-targets");
    fs::create_dir_all(temp.join("source/crates/terminal/src")).unwrap();
    fs::write(
        temp.join("source/crates/terminal/src/lib.rs"),
        "const OWNER: &str = \"/Users/josh/Developer/other-repo\";\n",
    )
    .unwrap();
    let source = metadata(vec![package(
        "terminal",
        &temp
            .join("source/crates/terminal/Cargo.toml")
            .display()
            .to_string(),
        &[],
    )]);
    let target = metadata(Vec::new());

    assert!(matches!(
        build_roster(&source, &target, "omp-orchestrator", "control-plane"),
        Err(RosterError::EmptyExtractionTargets)
    ));
    let _ = fs::remove_dir_all(temp);
}

#[test]
fn overlap_record_marks_source_identical_and_target_authority() {
    let temp = tempfile_path("roster-overlap");
    fs::create_dir_all(temp.join("source/crates/shared/src")).unwrap();
    fs::create_dir_all(temp.join("source/crates/port/src")).unwrap();
    fs::create_dir_all(temp.join("target/crates/shared/src")).unwrap();
    fs::write(temp.join("source/crates/shared/src/lib.rs"), "same\n").unwrap();
    fs::write(temp.join("source/crates/port/src/lib.rs"), "port\n").unwrap();
    fs::write(temp.join("target/crates/shared/src/lib.rs"), "same\n").unwrap();

    let source = metadata(vec![
        package(
            "shared",
            &temp
                .join("source/crates/shared/Cargo.toml")
                .display()
                .to_string(),
            &[],
        ),
        package(
            "port",
            &temp
                .join("source/crates/port/Cargo.toml")
                .display()
                .to_string(),
            &[],
        ),
    ]);
    let target = metadata(vec![package(
        "shared",
        &temp
            .join("target/crates/shared/Cargo.toml")
            .display()
            .to_string(),
        &["subprocess-contract"],
    )]);

    let roster = build_roster(&source, &target, "omp-orchestrator", "control-plane").unwrap();
    let overlap = &roster.overlaps[0];
    assert_eq!(overlap.name, "shared");
    assert!(overlap.byte_identical);
    assert_eq!(overlap.recommended_authority, "target-workspace");
    assert_eq!(
        overlap.target_only_path_dependencies,
        vec!["subprocess-contract"]
    );
    let _ = fs::remove_dir_all(temp);
}

fn tempfile_path(label: &str) -> std::path::PathBuf {
    let path =
        std::env::temp_dir().join(format!("extraction-roster-{label}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    path
}
