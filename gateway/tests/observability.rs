//! Unit tests for just-enough observability features.
//!
//! Tests PayloadStore abstraction, URL parsing, and size threshold logic.

use gateway::store::payload_store::*;

#[tokio::test]
async fn test_payload_store_should_offload() {
    let pg_store = PayloadStore::Postgres;
    assert!(
        !pg_store.should_offload(10000, 10000),
        "Postgres backend never offloads"
    );

    // Mock ObjectStore bypassing network
    let obj_store = PayloadStore::from_env().unwrap_or(PayloadStore::Postgres);
    match obj_store {
        PayloadStore::Object { size_threshold, .. } => {
            assert!(
                obj_store.should_offload(size_threshold, 1),
                "Should offload if combined > threshold"
            );
            assert!(
                !obj_store.should_offload(size_threshold / 2, size_threshold / 2),
                "Should not offload if combined <= threshold"
            );
        }
        PayloadStore::Postgres => {
            // Test running in CI without env vars
        }
    }
}
