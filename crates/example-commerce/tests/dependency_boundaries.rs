//! Integration tests for Phase 01 workspace dependency boundaries.

use std::path::{Path, PathBuf};
use std::process::Command;

const FORBIDDEN_DEPENDENCIES: &[&str] = &[
    "tokio",
    "sqlx",
    "axum",
    "tonic",
    "async-nats",
    "rdkafka",
    "postgres",
    "disruptor",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("example-commerce lives under crates/")
        .to_path_buf()
}

fn cargo_tree(package: &str) -> String {
    let output = Command::new("cargo")
        .args(["tree", "-p", package])
        .current_dir(workspace_root())
        .output()
        .expect("cargo tree runs");

    assert!(
        output.status.success(),
        "cargo tree failed for {package}: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).expect("cargo tree output is utf-8")
}

fn assert_no_forbidden_dependencies(package: &str) {
    let tree = cargo_tree(package);
    for line in tree.lines() {
        let Some(package_name) = line.split_whitespace().next() else {
            continue;
        };
        for forbidden in FORBIDDEN_DEPENDENCIES {
            assert_ne!(
                package_name, *forbidden,
                "{package} dependency tree contains forbidden dependency `{forbidden}`:\n{tree}"
            );
        }
    }
}

#[test]
fn es_core_has_no_forbidden_dependencies() {
    assert_no_forbidden_dependencies("es-core");
}

#[test]
fn es_kernel_has_no_forbidden_dependencies() {
    assert_no_forbidden_dependencies("es-kernel");
}

#[test]
fn required_workspace_members_exist() {
    let root = workspace_root();
    for member in [
        "es-core",
        "es-kernel",
        "es-runtime",
        "es-store-postgres",
        "es-projection",
        "es-outbox",
        "example-commerce",
        "adapter-http",
        "adapter-grpc",
        "app",
    ] {
        assert!(
            root.join("crates").join(member).is_dir(),
            "missing workspace member directory: {member}"
        );
    }
}
