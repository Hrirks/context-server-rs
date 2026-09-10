//! Query result cache with LRU eviction and TTL support.
//!
//! Salvaged from the abandoned `origin/enhancements` branch and pruned of the
//! dead domain-specific key builders (security policy, feature context). The
//! valuable part — a thread-safe LRU cache with per-entry TTL — is preserved.

use lru::LruCache;
use serde_json::Value;
use std::num::NonZeroUsize;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::{debug, trace};

/// A cached value with an optional time-to-live.
#[derive(Clone)]
pub struct CacheEntry {
    data: Value,
    created_at: Instant,
    ttl: Option<Duration>,
}

impl CacheEntry {
    /// Whether this entry has outlived its TTL (if any).
    pub fn is_expired(&self) -> bool {
        match self.ttl {
            Some(ttl) => self.created_at.elapsed() > ttl,
            None => false,
        }
    }
}

/// A thread-safe query cache with LRU eviction and TTL support.
pub struct QueryCache {
    cache: Arc<RwLock<LruCache<String, CacheEntry>>>,
    max_size: usize,
}

impl QueryCache {
    /// Create a cache that holds at most `max_size` entries.
    pub fn new(max_size: usize) -> Self {
        let cap = NonZeroUsize::new(max_size).unwrap_or(NonZeroUsize::MIN);
        Self {
            cache: Arc::new(RwLock::new(LruCache::new(cap))),
            max_size,
        }
    }

    /// Return a cached value if present and not expired. Expired entries are
    /// evicted on access.
    pub fn get(&self, key: &str) -> Option<Value> {
        let mut cache = self.cache.write().unwrap();
        if cache.get(key).map(|e| e.is_expired()).unwrap_or(false) {
            trace!("Cache entry expired for key: {}", key);
            cache.pop(key);
            return None;
        }
        match cache.get(key) {
            Some(entry) => {
                debug!("Cache hit for key: {}", key);
                Some(entry.data.clone())
            }
            None => {
                trace!("Cache miss for key: {}", key);
                None
            }
        }
    }

    /// Store a value, optionally expiring after `ttl`.
    pub fn set(&self, key: String, value: Value, ttl: Option<Duration>) {
        let entry = CacheEntry {
            data: value,
            created_at: Instant::now(),
            ttl,
        };
        let mut cache = self.cache.write().unwrap();
        cache.put(key.clone(), entry);
        debug!("Cached value for key: {}", key);
    }

    /// Evict a single entry.
    pub fn invalidate(&self, key: &str) {
        let mut cache = self.cache.write().unwrap();
        if cache.pop(key).is_some() {
            debug!("Invalidated cache entry for key: {}", key);
        }
    }

    /// Evict every entry whose key contains `pattern`.
    pub fn invalidate_pattern(&self, pattern: &str) {
        let mut cache = self.cache.write().unwrap();
        let keys: Vec<String> = cache
            .iter()
            .filter(|(k, _)| k.contains(pattern))
            .map(|(k, _)| k.clone())
            .collect();
        for key in keys {
            cache.pop(key.as_str());
        }
        debug!("Invalidated cache entries matching pattern: {}", pattern);
    }

    /// Evict everything.
    pub fn clear(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
        debug!("Cleared all cache entries");
    }

    /// Snapshot of current usage.
    pub fn stats(&self) -> CacheStats {
        let cache = self.cache.read().unwrap();
        CacheStats {
            size: cache.len(),
            max_size: self.max_size,
        }
    }
}

/// Cache usage snapshot.
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub size: usize,
    pub max_size: usize,
}

impl CacheStats {
    /// Current fill ratio as a percentage.
    pub fn utilization_percent(&self) -> f64 {
        if self.max_size == 0 {
            0.0
        } else {
            (self.size as f64 / self.max_size as f64) * 100.0
        }
    }
}

/// Builders for the cache keys used by the live repositories.
pub struct CacheKeyBuilder;

impl CacheKeyBuilder {
    pub fn project(project_id: &str) -> String {
        format!("project:{project_id}")
    }

    pub fn business_rule(rule_id: &str) -> String {
        format!("rule:{rule_id}")
    }

    pub fn business_rules_by_project(project_id: &str) -> String {
        format!("rules:project:{project_id}")
    }

    pub fn architectural_decision(decision_id: &str) -> String {
        format!("decision:{decision_id}")
    }

    pub fn architectural_decisions_by_project(project_id: &str) -> String {
        format!("decisions:project:{project_id}")
    }

    pub fn performance_requirement(req_id: &str) -> String {
        format!("perf_req:{req_id}")
    }

    pub fn performance_requirements_by_project(project_id: &str) -> String {
        format!("perf_reqs:project:{project_id}")
    }

    pub fn framework_component(component_id: &str) -> String {
        format!("component:{component_id}")
    }

    pub fn framework_components_by_project(project_id: &str) -> String {
        format!("components:project:{project_id}")
    }

    pub fn all_projects() -> String {
        "projects:all".to_string()
    }

    pub fn project_invalidation_pattern(project_id: &str) -> String {
        format!("*:project:{project_id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basic_operations() {
        let cache = QueryCache::new(100);
        let key = "test_key".to_string();
        let value = serde_json::json!({"name": "test"});

        cache.set(key.clone(), value.clone(), None);
        assert_eq!(cache.get(&key), Some(value));
    }

    #[test]
    fn test_cache_expiration() {
        let cache = QueryCache::new(100);
        let key = "test_key".to_string();
        let value = serde_json::json!({"name": "test"});

        cache.set(key.clone(), value.clone(), Some(Duration::from_millis(1)));
        assert_eq!(cache.get(&key), Some(value));

        std::thread::sleep(Duration::from_millis(10));
        assert_eq!(cache.get(&key), None);
    }

    #[test]
    fn test_cache_invalidation() {
        let cache = QueryCache::new(100);
        let key = "test_key".to_string();
        let value = serde_json::json!({"name": "test"});

        cache.set(key.clone(), value, None);
        assert!(cache.get(&key).is_some());

        cache.invalidate(&key);
        assert_eq!(cache.get(&key), None);
    }

    #[test]
    fn test_cache_key_builder() {
        assert_eq!(CacheKeyBuilder::project("p1"), "project:p1");
        assert_eq!(CacheKeyBuilder::business_rule("r1"), "rule:r1");
        assert_eq!(
            CacheKeyBuilder::business_rules_by_project("p1"),
            "rules:project:p1"
        );
        assert_eq!(
            CacheKeyBuilder::project_invalidation_pattern("p1"),
            "*:project:p1"
        );
    }
}
