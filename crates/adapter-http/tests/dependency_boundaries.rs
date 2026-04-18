//! Dependency and source-boundary tests for the HTTP adapter.

use std::fs;
use std::path::Path;

#[test]
fn dependency_boundaries_adapter_http_has_no_storage_projection_or_outbox_dependencies() {
    let manifest = read("crates/adapter-http/Cargo.toml");

    for forbidden in ["es-store-postgres", "es-projection", "es-outbox"] {
        assert!(
            !manifest.contains(forbidden),
            "adapter-http manifest must not contain forbidden dependency {forbidden}"
        );
    }
}

#[test]
fn dependency_boundaries_adapter_http_source_does_not_mutate_hot_state_directly() {
    let forbidden_patterns = [
        "ShardState",
        "AggregateCache",
        "PostgresOutboxStore",
        "PostgresProjectionStore",
        "PostgresEventStore",
        "Arc<Mutex",
        "RwLock",
        ".append(",
        ".catch_up(",
        ".mark_published(",
        ".insert_outbox_message(",
    ];

    for path in [
        "crates/adapter-http/src/lib.rs",
        "crates/adapter-http/src/commerce.rs",
        "crates/adapter-http/src/error.rs",
    ] {
        let source = read(path);
        for forbidden in forbidden_patterns {
            assert!(
                !source.contains(forbidden),
                "{path} must not contain forbidden adapter boundary marker {forbidden}"
            );
        }
    }
}

fn read(path: impl AsRef<Path>) -> String {
    let repo_root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("workspace root");
    let path = repo_root.join(path.as_ref());
    fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}
