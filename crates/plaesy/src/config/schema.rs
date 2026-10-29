//! Unified `.plaesy/config.json` schema.
//!
//! Every struct derives `Default` with zero values, matching Go's
//! `json.Unmarshal` into a zero value. The `defaults()` constructors give the
//! shipped configuration.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::delegate::DelegateConfig;

/// Root configuration for all plaesy capabilities.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UnifiedConfig {
    pub version: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reach: Option<ReachConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search: Option<SearchConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub analyze: Option<AnalyzeConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph: Option<GraphConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delegate: Option<DelegateConfig>,
}

impl UnifiedConfig {
    /// The shipped configuration: every section at its default.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            version: 1,
            reach: Some(ReachConfig::defaults()),
            search: Some(SearchConfig::defaults()),
            analyze: Some(AnalyzeConfig::defaults()),
            graph: Some(GraphConfig::defaults()),
            delegate: Some(DelegateConfig::defaults()),
        }
    }
}

/// Reach (external data sources) settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReachConfig {
    #[serde(skip_serializing_if = "String::is_empty")]
    pub cache_dir: String,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub cache_size_mb: i64,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub cache_memory_size: i64,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub default_cache_ttl: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub platforms: BTreeMap<String, ReachPlatformConfig>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub rate_limits: BTreeMap<String, RateLimitConfig>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub timeouts: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_config: Option<RetryConfig>,
}

impl ReachConfig {
    /// Shipped reach defaults.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            cache_dir: ".plaesy/reach/cache".into(),
            cache_size_mb: 100,
            cache_memory_size: 1000,
            default_cache_ttl: "24h".into(),
            platforms: BTreeMap::new(),
            rate_limits: default_rate_limits(),
            timeouts: default_timeouts(),
            retry_config: Some(RetryConfig::defaults()),
        }
    }
}

/// Per-platform reach settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ReachPlatformConfig {
    pub enabled: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub backends: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub default_backend: String,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub credentials: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub options: BTreeMap<String, String>,
}

/// Rate limit for one platform.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RateLimitConfig {
    pub requests: i64,
    pub window: String,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub burst: i64,
}

/// Retry policy for reach calls.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RetryConfig {
    pub max_retries: i64,
    pub base_delay: String,
    pub max_delay: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub retryable_errors: Vec<String>,
}

impl RetryConfig {
    /// Shipped retry defaults.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            max_retries: 3,
            base_delay: "500ms".into(),
            max_delay: "10s".into(),
            retryable_errors: [
                "timeout",
                "connection refused",
                "temporary failure",
                "rate limit",
                "503",
                "504",
            ]
            .map(String::from)
            .to_vec(),
        }
    }
}

/// Search settings: embedder, index, chunking, and retrieval.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SearchConfig {
    pub embedder: EmbedderConfig,
    pub vector_store: VectorStoreConfig,
    pub text_index: TextIndexConfig,
    pub chunker: ChunkerConfig,
    pub indexer: IndexerConfig,
    pub retriever: RetrieverConfig,
    pub reranker: RerankerConfig,
}

impl SearchConfig {
    /// Shipped search defaults.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            embedder: EmbedderConfig {
                kind: "rembed".into(),
                model: "intfloat/multilingual-e5-base".into(),
                dimension: 768,
            },
            vector_store: VectorStoreConfig {
                kind: "chromem".into(),
            },
            text_index: TextIndexConfig {
                kind: "bleve".into(),
            },
            chunker: ChunkerConfig {
                overlap_tokens: 50,
                min_chunk_tokens: 100,
                max_chunk_tokens: 256,
            },
            indexer: IndexerConfig {
                workers: 4,
                resume: true,
            },
            retriever: RetrieverConfig {
                hybrid: HybridConfig {
                    vector_weight: 0.6,
                    bm25_weight: 0.4,
                    rrf_k: 60,
                },
                top_k: 50,
                parent_child: ParentChildConfig {
                    enabled: true,
                    parent_tokens: 1000,
                },
            },
            reranker: RerankerConfig {
                enabled: false,
                model: "jina-reranker-v2-base-multilingual".into(),
                top_n: 10,
            },
        }
    }
}

/// Embedding model.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EmbedderConfig {
    #[serde(rename = "type")]
    pub kind: String,
    pub model: String,
    pub dimension: i64,
}

/// Vector store backend.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct VectorStoreConfig {
    #[serde(rename = "type")]
    pub kind: String,
}

/// Full-text index backend.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct TextIndexConfig {
    #[serde(rename = "type")]
    pub kind: String,
}

/// Document chunking sizes, in tokens.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ChunkerConfig {
    pub overlap_tokens: i64,
    pub min_chunk_tokens: i64,
    pub max_chunk_tokens: i64,
}

/// Indexing pipeline.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct IndexerConfig {
    pub workers: i64,
    pub resume: bool,
}

/// Retrieval settings.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RetrieverConfig {
    pub hybrid: HybridConfig,
    pub top_k: i64,
    pub parent_child: ParentChildConfig,
}

/// Reciprocal-rank fusion between vector and keyword scores.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct HybridConfig {
    pub vector_weight: f64,
    pub bm25_weight: f64,
    pub rrf_k: i64,
}

/// Parent-child chunk retrieval.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ParentChildConfig {
    pub enabled: bool,
    pub parent_tokens: i64,
}

/// Cross-encoder reranking.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct RerankerConfig {
    pub enabled: bool,
    pub model: String,
    pub top_n: i64,
}

/// Analyze settings.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalyzeConfig {
    pub auto_index: bool,
    pub workers: i64,
}

impl AnalyzeConfig {
    /// Shipped analyze defaults.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            auto_index: true,
            workers: 4,
        }
    }
}

/// Dependency-graph scope.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GraphConfig {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub include_extensions: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub exclude_patterns: Vec<String>,
    #[serde(skip_serializing_if = "is_zero_i64")]
    pub max_file_size: i64,
}

impl GraphConfig {
    /// Shipped graph defaults. Files over 10 MiB are skipped.
    #[must_use]
    pub fn defaults() -> Self {
        Self {
            include_extensions: [
                ".go", ".js", ".jsx", ".ts", ".tsx", ".py", ".rs", ".rb", ".java", ".kt", ".kts",
                ".cs", ".c", ".h", ".cpp", ".hpp", ".cc", ".hh", ".cxx", ".swift", ".php", ".dart",
                ".sh", ".ps1", ".md", ".mdx", ".txt",
            ]
            .map(String::from)
            .to_vec(),
            exclude_patterns: [
                "vendor/**",
                "node_modules/**",
                ".git/**",
                "dist/**",
                "build/**",
                "**/*.min.js",
                "**/*.min.css",
                "**/testdata/**",
                "**/vendor/**",
            ]
            .map(String::from)
            .to_vec(),
            max_file_size: 10 * 1024 * 1024,
        }
    }
}

fn default_rate_limits() -> BTreeMap<String, RateLimitConfig> {
    [
        ("github", 100, 10),
        ("web", 20, 5),
        ("youtube", 10, 2),
        ("rss", 5, 1),
        ("twitter", 3, 1),
        ("reddit", 5, 1),
        ("bilibili", 10, 2),
        ("exa", 10, 2),
        ("v2ex", 20, 5),
        ("xueqiu", 20, 5),
    ]
    .into_iter()
    .map(|(name, requests, burst)| {
        (
            name.to_owned(),
            RateLimitConfig {
                requests,
                window: "1m".into(),
                burst,
            },
        )
    })
    .collect()
}

fn default_timeouts() -> BTreeMap<String, String> {
    [
        ("github", "30s"),
        ("web", "15s"),
        ("youtube", "60s"),
        ("rss", "10s"),
        ("twitter", "15s"),
        ("reddit", "15s"),
        ("bilibili", "15s"),
        ("exa", "30s"),
        ("v2ex", "10s"),
        ("xueqiu", "10s"),
    ]
    .into_iter()
    .map(|(name, timeout)| (name.to_owned(), timeout.to_owned()))
    .collect()
}

fn is_zero_i64(value: &i64) -> bool {
    *value == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_shipped_values() {
        let cfg = UnifiedConfig::defaults();
        assert_eq!(cfg.version, 1);
        let search = cfg.search.unwrap();
        assert_eq!(search.embedder.dimension, 768);
        assert_eq!(search.chunker.max_chunk_tokens, 256);
        assert_eq!(search.retriever.hybrid.rrf_k, 60);
        let reach = cfg.reach.unwrap();
        assert_eq!(reach.rate_limits["github"].requests, 100);
        assert_eq!(reach.timeouts["exa"], "30s");
        assert_eq!(cfg.graph.unwrap().max_file_size, 10 * 1024 * 1024);
    }

    #[test]
    fn missing_fields_stay_zero_like_the_go_loader() {
        let cfg: UnifiedConfig = serde_json::from_str(r#"{"version": 2}"#).unwrap();
        assert_eq!(cfg.version, 2);
        assert!(cfg.search.is_none());
    }

    #[test]
    fn type_keyword_is_written_as_type() {
        let json = serde_json::to_string(&SearchConfig::defaults().embedder).unwrap();
        assert!(json.contains(r#""type":"rembed""#), "{json}");
    }

    #[test]
    fn round_trips_through_json() {
        let cfg = UnifiedConfig::defaults();
        let json = serde_json::to_string_pretty(&cfg).unwrap();
        let back: UnifiedConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cfg);
    }
}
