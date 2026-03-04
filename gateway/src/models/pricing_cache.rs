/// In-memory pricing cache backed by the `model_pricing` DB table.
///
/// Loaded at startup and refreshed on every upsert/delete via the API.
/// `calculate_cost_with_cache` reads from this cache first, falling back to the
/// hardcoded table only if the cache is empty (e.g., before first load).
use std::sync::Arc;
use tokio::sync::RwLock;
use rust_decimal::Decimal;

/// A single pricing entry held in memory.
#[derive(Debug, Clone)]
pub struct PricingEntry {
    pub provider: String,
    pub model_pattern: String,
    pub input_per_m: Decimal,
    pub output_per_m: Decimal,
}

/// Shared, cheaply-cloneable pricing cache.
#[derive(Clone)]
pub struct PricingCache(Arc<RwLock<Vec<PricingEntry>>>);

impl Default for PricingCache {
    fn default() -> Self {
        Self::new()
    }
}

impl PricingCache {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(Vec::new())))
    }

    /// Replace the entire cache with a fresh set of entries.
    /// Callers convert DB rows to `PricingEntry` before calling this.
    pub async fn reload(&self, entries: Vec<PricingEntry>) {
        *self.0.write().await = entries;
    }

    /// Look up pricing for a (provider, model) pair.
    /// Matches by substring: the first entry whose `model_pattern` is contained
    /// in `model` wins. Entries are checked in insertion order (DB ORDER BY).
    pub async fn lookup(&self, provider: &str, model: &str) -> Option<(Decimal, Decimal)> {
        let entries = self.0.read().await;
        for entry in entries.iter() {
            if entry.provider == provider && model.contains(entry.model_pattern.as_str()) {
                return Some((entry.input_per_m, entry.output_per_m));
            }
        }
        None
    }

    /// Return all entries (for the list API endpoint).
    pub async fn all(&self) -> Vec<PricingEntry> {
        self.0.read().await.clone()
    }

    /// Return true if the cache has been populated.
    pub async fn is_populated(&self) -> bool {
        !self.0.read().await.is_empty()
    }
}
