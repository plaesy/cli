//! Hybrid-search ranking: modes and result rankers.

use super::*;

/// Which retrieval legs a query uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchMode {
    /// Keyword + semantic, fused.
    #[default]
    Hybrid,
    /// Embedding similarity only.
    Semantic,
    /// Full-text (BM25) only.
    Keyword,
}

impl SearchMode {
    /// Parse `hybrid`, `semantic` or `keyword`.
    pub fn parse(s: &str) -> Option<SearchMode> {
        match s {
            "hybrid" => Some(SearchMode::Hybrid),
            "semantic" => Some(SearchMode::Semantic),
            "keyword" => Some(SearchMode::Keyword),
            _ => None,
        }
    }
}

/// Ranker combines results from multiple search methods.
pub struct Ranker {
    config: RankerConfig,
}

/// Ranker configuration.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RankerConfig {
    /// Weight for vector search results (0-1).
    pub vector_weight: f32,

    /// Weight for keyword search results (0-1).
    pub keyword_weight: f32,

    /// Fusion algorithm.
    pub ranker_type: RankerType,
}

impl Default for RankerConfig {
    fn default() -> Self {
        RankerConfig {
            vector_weight: 0.7,
            keyword_weight: 0.3,
            ranker_type: RankerType::Weighted,
        }
    }
}

/// Type of ranking algorithm.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum RankerType {
    /// Reciprocal Rank Fusion (simple, no ML).
    RRF,

    /// Weighted score fusion: `vector_weight * cosine + keyword_weight * bm25_strength`.
    /// Keeps absolute score magnitude, so a strong semantic match is not outvoted by weak keyword hits.
    Weighted,
}

impl Ranker {
    /// Create a new ranker.
    pub fn new(config: RankerConfig) -> Self {
        Ranker { config }
    }

    /// Weighted fusion of absolute vector scores and relative keyword scores.
    fn weighted(
        vw: f32,
        kw: f32,
        vector_results: &[SearchResult],
        keyword_results: &[SearchResult],
    ) -> Vec<SearchResult> {
        let mut by_chunk: HashMap<u64, (f32, f32, SearchResult)> = HashMap::new();
        for r in keyword_results {
            by_chunk
                .entry(r.chunk_id)
                .or_insert_with(|| (0.0, r.score, r.clone()))
                .1 = r.score;
        }
        for r in vector_results {
            by_chunk
                .entry(r.chunk_id)
                .and_modify(|e| e.0 = r.score)
                .or_insert_with(|| (r.score, 0.0, r.clone()));
        }
        // Only legs that returned something count toward the normaliser.
        let mut denom = 0.0;
        if !vector_results.is_empty() {
            denom += vw;
        }
        if !keyword_results.is_empty() {
            denom += kw;
        }
        if denom <= 0.0 {
            return Vec::new();
        }
        let mut merged: Vec<SearchResult> = by_chunk
            .into_values()
            .map(|(v, k, mut r)| {
                r.score = ((vw * v + kw * k) / denom).clamp(0.0, 1.0);
                r
            })
            .collect();
        merged.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then(a.chunk_id.cmp(&b.chunk_id))
        });
        merged
    }

    /// Merge and re-rank results from multiple sources with weighted RRF.
    ///
    /// `score(d) = Σ weight_s / (60 + rank_s(d))` over the sources containing
    /// `d` (1-based ranks), normalised by the best score a document could get
    /// from the *non-empty* sources, so scores fall in `(0, 1]`. Results are
    /// de-duplicated by chunk. A result found by keyword search keeps
    /// `Fact`; one found only by vectors is `Inferred`. Ties break on chunk id.
    ///
    /// # Errors
    ///
    /// - `RankingError` if weights are negative/non-finite or both zero
    pub fn merge_and_rank(
        &self,
        vector_results: &[SearchResult],
        keyword_results: &[SearchResult],
    ) -> Result<Vec<SearchResult>> {
        let (vw, kw) = (self.config.vector_weight, self.config.keyword_weight);
        if !vw.is_finite() || !kw.is_finite() || vw < 0.0 || kw < 0.0 || vw + kw <= 0.0 {
            return Err(MemoryError::RankingError(format!(
                "invalid weights: vector={vw}, keyword={kw}"
            )));
        }
        if self.config.ranker_type == RankerType::Weighted {
            return Ok(Self::weighted(vw, kw, vector_results, keyword_results));
        }
        let mut max = 0.0f32;
        if !vector_results.is_empty() {
            max += vw / (RRF_K + 1.0);
        }
        if !keyword_results.is_empty() {
            max += kw / (RRF_K + 1.0);
        }
        if max <= 0.0 {
            return Ok(Vec::new());
        }

        let mut by_chunk: HashMap<u64, (f32, SearchResult)> = HashMap::new();
        for (rank, r) in keyword_results.iter().enumerate() {
            let s = kw / (RRF_K + rank as f32 + 1.0);
            by_chunk
                .entry(r.chunk_id)
                .and_modify(|e| e.0 += s)
                .or_insert_with(|| (s, r.clone()));
        }
        for (rank, r) in vector_results.iter().enumerate() {
            let s = vw / (RRF_K + rank as f32 + 1.0);
            by_chunk
                .entry(r.chunk_id)
                .and_modify(|e| e.0 += s)
                .or_insert_with(|| (s, r.clone()));
        }
        let mut merged: Vec<SearchResult> = by_chunk
            .into_values()
            .map(|(s, mut r)| {
                r.score = (s / max).clamp(0.0, 1.0);
                r
            })
            .collect();
        merged.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then(a.chunk_id.cmp(&b.chunk_id))
        });
        Ok(merged)
    }
}
