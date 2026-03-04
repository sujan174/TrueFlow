//! PayloadStore — Pluggable backend for request/response body storage.
//!
//! Supports two backends:
//!   - `Postgres` (default): writes bodies into `audit_log_bodies` table, as before.
//!   - `ObjectStore`: offloads bodies > `size_threshold` bytes to S3/MinIO/GCS/local
//!     filesystem, storing a reference URL in `audit_logs.payload_url`.
//!
//! ## Configuration
//!
//! Set the `PAYLOAD_STORE_URL` environment variable:
//!
//! ```text
//! # S3
//! PAYLOAD_STORE_URL=s3://my-bucket?region=us-east-1
//!
//! # MinIO (self-hosted S3-compatible)
//! PAYLOAD_STORE_URL=s3://my-bucket?endpoint=http://minio:9000&region=us-east-1
//!
//! # Local filesystem (great for dev/testing)
//! PAYLOAD_STORE_URL=file:///tmp/trueflow-payloads
//!
//! # Unset or empty → Postgres fallback (default)
//! ```
//!
//! Optionally tune the size threshold (default: 4096 bytes):
//! ```text
//! PAYLOAD_SIZE_THRESHOLD=8192
//! ```

use std::sync::Arc;

use anyhow::{Context, Result};
use object_store::{path::Path, ObjectStore};
use uuid::Uuid;

/// Minimum body size (req + resp combined) to trigger offload. Below this,
/// bodies go directly into audit_log_bodies in Postgres.
const DEFAULT_SIZE_THRESHOLD: usize = 4096;

/// The payload storage backend.
pub enum PayloadStore {
    /// Default: store bodies directly in Postgres `audit_log_bodies`.
    Postgres,

    /// Object store backend: offload large bodies to S3/MinIO/GCS/local.
    Object {
        store: Arc<dyn ObjectStore>,
        prefix: String,
        size_threshold: usize,
    },
}


impl PayloadStore {
    /// Build a `PayloadStore` from environment variables.
    ///
    /// Returns `Ok(PayloadStore::Postgres)` if `PAYLOAD_STORE_URL` is not set.
    pub fn from_env() -> Result<Self> {
        let url = match std::env::var("PAYLOAD_STORE_URL") {
            Ok(u) if !u.is_empty() => u,
            _ => return Ok(PayloadStore::Postgres),
        };

        let threshold = std::env::var("PAYLOAD_SIZE_THRESHOLD")
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(DEFAULT_SIZE_THRESHOLD);

        let (store, prefix) = build_object_store(&url)?;

        tracing::info!(
            url = %url,
            threshold_bytes = threshold,
            "PayloadStore: using object store backend"
        );

        Ok(PayloadStore::Object {
            store: Arc::from(store),
            prefix,
            size_threshold: threshold,
        })
    }

    /// Returns `true` if bodies should go to object store for this request.
    pub fn should_offload(&self, req_len: usize, resp_len: usize) -> bool {
        match self {
            PayloadStore::Postgres => false,
            PayloadStore::Object { size_threshold, .. } => {
                req_len + resp_len > *size_threshold
            }
        }
    }

    /// Offload request and response bodies to the object store.
    ///
    /// Returns the object store key (to be stored as `payload_url`).
    /// Callers should only call this when `should_offload()` returns true.
    #[allow(clippy::too_many_arguments)]
    pub async fn put(
        &self,
        request_id: Uuid,
        project_id: Uuid,
        created_at: chrono::DateTime<chrono::Utc>,
        request_body: Option<&str>,
        response_body: Option<&str>,
        request_headers: Option<&serde_json::Value>,
        response_headers: Option<&serde_json::Value>,
    ) -> Result<String> {
        let PayloadStore::Object { store, prefix, .. } = self else {
            anyhow::bail!("PayloadStore::put called on Postgres backend");
        };

        // Build a compact JSON blob with all body data
        let blob = serde_json::json!({
            "request_id": request_id,
            "project_id": project_id,
            "created_at": created_at.to_rfc3339(),
            "request_body": request_body,
            "response_body": response_body,
            "request_headers": request_headers,
            "response_headers": response_headers,
        });

        let json_bytes = serde_json::to_vec(&blob)
            .context("failed to serialize payload blob")?;

        // Compress with zstd (typically 60-80% compression on JSON)
        let compressed = zstd::encode_all(json_bytes.as_slice(), 3)
            .context("failed to compress payload")?;

        // Key: payloads/{project_id}/{YYYY-MM-DD}/{request_id}.json.zst
        let date = created_at.format("%Y-%m-%d");
        let key = format!("{}/{}/{}/{}.json.zst", prefix, project_id, date, request_id);
        let path = Path::from(key.clone());

        store
            .put(&path, compressed.into())
            .await
            .context("failed to put payload to object store")?;

        tracing::debug!(key = %key, "payload offloaded to object store");
        Ok(key)
    }

    /// Fetch bodies from the object store.
    ///
    /// Returns `(request_body, response_body)` strings.
    pub async fn get(&self, payload_url: &str) -> Result<PayloadBodies> {
        let PayloadStore::Object { store, .. } = self else {
            anyhow::bail!("PayloadStore::get called on Postgres backend");
        };

        let path = Path::from(payload_url);
        let bytes = store
            .get(&path)
            .await
            .context("failed to get payload from object store")?
            .bytes()
            .await
            .context("failed to read payload bytes")?;

        let decompressed = zstd::decode_all(bytes.as_ref())
            .context("failed to decompress payload")?;

        let blob: serde_json::Value = serde_json::from_slice(&decompressed)
            .context("failed to parse payload blob")?;

        Ok(PayloadBodies {
            request_body: blob["request_body"].as_str().map(String::from),
            response_body: blob["response_body"].as_str().map(String::from),
            request_headers: blob.get("request_headers").cloned(),
            response_headers: blob.get("response_headers").cloned(),
        })
    }
}

/// Deserialized payload from object store.
#[derive(Debug)]
pub struct PayloadBodies {
    pub request_body: Option<String>,
    pub response_body: Option<String>,
    pub request_headers: Option<serde_json::Value>,
    pub response_headers: Option<serde_json::Value>,
}

/// Parse a `PAYLOAD_STORE_URL` and return an `(ObjectStore impl, prefix)` pair.
fn build_object_store(url: &str) -> Result<(Box<dyn ObjectStore>, String)> {
    if url.starts_with("file://") {
        // Local filesystem — great for development/testing
        let path = url.trim_start_matches("file://");
        let store = object_store::local::LocalFileSystem::new_with_prefix(path)
            .context("failed to create local file system object store")?;
        return Ok((Box::new(store), "payloads".to_string()));
    }

    if url.starts_with("s3://") {
        // Parse the bucket name from s3://bucket-name?...
        let without_scheme = url.trim_start_matches("s3://");
        let bucket = without_scheme.split('?').next().unwrap_or(without_scheme);

        // Check for custom endpoint (MinIO)
        let endpoint = parse_query_param(url, "endpoint");
        let region = parse_query_param(url, "region").unwrap_or_else(|| "us-east-1".to_string());

        let mut builder = object_store::aws::AmazonS3Builder::new()
            .with_bucket_name(bucket)
            .with_region(&region);

        if let Some(ep) = endpoint {
            builder = builder.with_endpoint(&ep).with_allow_http(true);
        }

        // Credentials from env: AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY
        // (or instance metadata / IAM role in production)
        if let Ok(key) = std::env::var("AWS_ACCESS_KEY_ID") {
            if let Ok(secret) = std::env::var("AWS_SECRET_ACCESS_KEY") {
                builder = builder.with_access_key_id(key).with_secret_access_key(secret);
            }
        }

        let store = builder.build().context("failed to build S3 object store")?;
        return Ok((Box::new(store), "payloads".to_string()));
    }

    anyhow::bail!("unsupported PAYLOAD_STORE_URL scheme: {}", url)
}

fn parse_query_param(url: &str, key: &str) -> Option<String> {
    let query = url.split('?').nth(1)?;
    for part in query.split('&') {
        let mut kv = part.splitn(2, '=');
        if kv.next() == Some(key) {
            return kv.next().map(|v| urlencoding::decode(v).unwrap_or_default().into_owned());
        }
    }
    None
}
