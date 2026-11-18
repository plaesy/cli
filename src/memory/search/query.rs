//! `impl SearchEngine` methods: query, split out of `mod.rs`.

use super::*;

impl SearchEngine {
    fn clamp_limit(&self, limit: usize) -> usize {
        limit.clamp(1, self.config.max_limit.max(1))
    }

    async fn with_timeout<T>(
        &self,
        fut: impl std::future::Future<Output = Result<T>>,
    ) -> Result<T> {
        match tokio::time::timeout(Duration::from_secs(self.config.timeout_secs), fut).await {
            Ok(r) => r,
            Err(_) => {
                self.metrics.record_error("search_timeout");
                Err(MemoryError::ResourceLimitExceeded(format!(
                    "search exceeded {}s timeout",
                    self.config.timeout_secs
                )))
            }
        }
    }

    /// Execute a hybrid search query.
    ///
    /// Runs the semantic and keyword legs (whichever are enabled/available),
    /// fuses them with RRF, drops results under `confidence_threshold`, and
    /// returns at most `limit` (capped at `max_limit`).
    ///
    /// # Errors
    ///
    /// - `KeywordSearchError` if keyword search fails or no search method is enabled
    /// - `RankingError` if ranking fails
    /// - `ResourceLimitExceeded` on timeout
    ///
    /// A failing semantic leg is non-fatal: it is recorded in metrics and the
    /// keyword results are returned.
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.search_filtered(query, limit, &SearchFilter::default(), false)
            .await
    }

    /// Run a search in `mode`, restricted by `filter` (tier, path, trust, status).
    ///
    /// # Errors
    ///
    /// Same as [`SearchEngine::search`] / [`SearchEngine::semantic_search`].
    pub async fn search_with(
        &self,
        query: &str,
        limit: usize,
        mode: SearchMode,
        filter: &SearchFilter,
    ) -> Result<Vec<SearchResult>> {
        self.search_with_explain(query, limit, mode, filter, false)
            .await
    }

    /// Run a search in `mode`, restricted by `filter`, attaching a per-leg score
    /// breakdown to each result when `explain` is true.
    ///
    /// # Errors
    ///
    /// Same as [`SearchEngine::search`] / [`SearchEngine::semantic_search`].
    pub async fn search_with_explain(
        &self,
        query: &str,
        limit: usize,
        mode: SearchMode,
        filter: &SearchFilter,
        explain: bool,
    ) -> Result<Vec<SearchResult>> {
        match mode {
            SearchMode::Hybrid => self.search_filtered(query, limit, filter, explain).await,
            SearchMode::Semantic => self.semantic_filtered(query, limit, filter, explain).await,
            SearchMode::Keyword => self.keyword_filtered(query, limit, filter, explain).await,
        }
    }

    /// `search_with`, plus one hop along wikilinks. Returns the `limit` direct hits, followed by the
    /// best passage of each note that the top hits link to or are linked from (at most
    /// `LINK_NEIGHBOUR_LIMIT`, ranked among themselves).
    ///
    /// The link only decides which note is looked at. A linked passage keeps its own evidence label
    /// (FACT only if its own text matched the query) and its score is multiplied by
    /// `LINK_SCORE_FACTOR`. It is returned only when it matches the query at all.
    ///
    /// # Errors
    ///
    /// - Any error from the underlying search or the link table
    pub async fn search_expanded(
        &self,
        query: &str,
        limit: usize,
        mode: SearchMode,
        filter: &SearchFilter,
    ) -> Result<Vec<SearchResult>> {
        self.search_expanded_with_explain(query, limit, mode, filter, false)
            .await
    }

    /// Expanded search with optional per-leg score explanations on direct and linked hits.
    pub async fn search_expanded_with_explain(
        &self,
        query: &str,
        limit: usize,
        mode: SearchMode,
        filter: &SearchFilter,
        explain: bool,
    ) -> Result<Vec<SearchResult>> {
        let mut hits = self
            .search_with_explain(query, limit, mode, filter, explain)
            .await?;
        let mut seeds: Vec<String> = hits
            .iter()
            .take(LINK_SEED_LIMIT)
            .map(|h| h.source_path.clone())
            .collect();
        seeds.sort();
        seeds.dedup();

        let mut neighbours: Vec<String> = Vec::new();
        for path in &seeds {
            for l in self.links(path, false).await? {
                if let Some(target) = l.resolved_path {
                    neighbours.push(target);
                }
            }
            for l in self.links(path, true).await? {
                neighbours.push(l.source_path);
            }
        }
        neighbours.retain(|p| !seeds.contains(p));
        neighbours.sort();
        neighbours.dedup();
        neighbours.truncate(LINK_NEIGHBOUR_LIMIT);

        let mut seen: HashSet<u64> = hits.iter().map(|h| h.chunk_id).collect();
        let mut linked: Vec<SearchResult> = Vec::new();
        for path in neighbours {
            let scoped = SearchFilter {
                path_prefix: Some(path.clone()),
                ..filter.clone()
            };
            // `path_prefix` is a prefix match; keep only the note itself.
            let best = self
                .search_with_explain(query, LINK_NEIGHBOUR_SEARCH, mode, &scoped, explain)
                .await?
                .into_iter()
                .find(|r| r.source_path == path);
            if let Some(mut r) = best {
                if seen.insert(r.chunk_id) {
                    r.score *= LINK_SCORE_FACTOR;
                    linked.push(r);
                }
            }
        }
        linked.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        // Direct hits keep their `limit`; linked passages follow them, so they are never cut
        // by the limit that the direct hits already filled.
        hits.extend(linked);
        Ok(hits)
    }

    /// Hybrid search restricted by `filter`. When `explain` is true, each
    /// result carries a per-leg score breakdown in `leg_scores`.
    pub async fn search_filtered(
        &self,
        query: &str,
        limit: usize,
        filter: &SearchFilter,
        explain: bool,
    ) -> Result<Vec<SearchResult>> {
        let timer = Timer::start();
        self.metrics.record_search_query();
        let res = self
            .with_timeout(self.hybrid(query, limit, filter, explain))
            .await;
        self.metrics
            .record_search_latency(timer.elapsed(), SearchType::Hybrid);
        if res.is_err() {
            self.metrics.record_error("search_failed");
        }
        res
    }

    async fn hybrid(
        &self,
        query: &str,
        limit: usize,
        filter: &SearchFilter,
        explain: bool,
    ) -> Result<Vec<SearchResult>> {
        let limit = self.clamp_limit(limit);
        if query.trim().is_empty() {
            return Ok(Vec::new());
        }
        // A restrictive filter discards candidates, so look at more of them.
        let oversample = if filter.is_restrictive() { 4 } else { 1 };
        let candidates = (limit * 3 * oversample).max(20);

        let keyword = if self.config.enable_keyword_search {
            let raw = self.db.search_fts(query, candidates).await?;
            Some(self.refine(raw, filter).await?)
        } else {
            None
        };
        let vector = if self.has_vector_search() {
            let leg = match self.vector_candidates(query, candidates).await {
                Ok(raw) => self.refine(raw, filter).await,
                Err(e) => Err(e),
            };
            match leg {
                Ok(v) => {
                    self.metrics.record_vector_search(true);
                    Some(v)
                }
                Err(e) => {
                    self.metrics.record_vector_search(false);
                    self.metrics.record_error("vector_search_failed");
                    tracing::warn!("semantic search failed, using keyword only: {e}");
                    None
                }
            }
        } else {
            None
        };
        if keyword.is_none() && vector.is_none() {
            return Err(MemoryError::KeywordSearchError(
                "no search method available (keyword disabled and semantic unavailable)"
                    .to_string(),
            ));
        }
        let keyword_scores: HashMap<u64, f32> = keyword
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|r| (r.chunk_id, r.score))
            .collect();
        let vector_scores: HashMap<u64, f32> = vector
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|r| (r.chunk_id, r.score))
            .collect();
        let mut merged = self.ranker.merge_and_rank(
            vector.as_deref().unwrap_or(&[]),
            keyword.as_deref().unwrap_or(&[]),
        )?;
        let has_reranker = self.has_reranker();
        if has_reranker {
            let n = self.config.rerank_top_n.max(limit);
            merged.truncate(n);
            match self
                .rerank_results(query, merged.clone(), n, self.config.rerank_weight)
                .await
            {
                Ok(r) => merged = r,
                Err(e) => {
                    self.metrics.record_error("rerank_failed");
                    tracing::warn!("reranking failed, using fused order: {e}");
                }
            }
        }
        if explain {
            use crate::memory::storage::LegScores;
            for r in &mut merged {
                r.explain = Some(LegScores {
                    fts: keyword_scores.get(&r.chunk_id).copied(),
                    vector: vector_scores.get(&r.chunk_id).copied(),
                    fused: Some(r.score),
                    staleness_factor: Some(if r.meta.stale { 0.5 } else { 1.0 }),
                    rerank: if has_reranker { Some(r.score) } else { None },
                });
            }
        }
        merged.retain(|r| r.score >= self.config.confidence_threshold);
        merged.truncate(limit);
        Ok(merged)
    }

    async fn vector_candidates(&self, query: &str, k: usize) -> Result<Vec<SearchResult>> {
        let engine = self
            .embeddings
            .as_ref()
            .ok_or(MemoryError::FeatureNotEnabled {
                feature: "embeddings".to_string(),
            })?;
        let t = Timer::start();
        let q = engine.embed(query).await?;
        self.metrics.record_embedding_latency(t.elapsed());
        self.metrics
            .set_embedding_cache_hit_ratio(engine.cache_hit_ratio());
        let hits = self.hnsw.read().search(&q, k)?;
        let mut out = Vec::with_capacity(hits.len());
        for (id, dist) in hits {
            let score = (1.0 - dist).clamp(0.0, 1.0);
            if let Some(r) = self
                .db
                .result_for_chunk(id, score, ConfidenceLevel::Inferred)
                .await?
            {
                out.push(r);
            }
        }
        Ok(out)
    }

    /// Execute semantic search only (vector similarity).
    ///
    /// # Errors
    ///
    /// - `FeatureNotEnabled` if no embedding engine is available
    pub async fn semantic_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.semantic_filtered(query, limit, &SearchFilter::default(), false)
            .await
    }

    async fn semantic_filtered(
        &self,
        query: &str,
        limit: usize,
        filter: &SearchFilter,
        explain: bool,
    ) -> Result<Vec<SearchResult>> {
        if !self.has_vector_search() {
            return Err(MemoryError::FeatureNotEnabled {
                feature: if self.config.enable_vector_search && self.embeddings.is_some() {
                    "semantic index (run `plaesy memory index` to synchronize it)".to_string()
                } else {
                    "embeddings".to_string()
                },
            });
        }
        let timer = Timer::start();
        self.metrics.record_search_query();
        let limit = self.clamp_limit(limit);
        let fetch = if filter.is_restrictive() {
            limit * 4
        } else {
            limit
        };
        let mut res = if query.trim().is_empty() {
            Ok(Vec::new())
        } else {
            self.with_timeout(async {
                let raw = self.vector_candidates(query, fetch).await?;
                let mut v = self.refine(raw, filter).await?;
                v.truncate(limit);
                Ok(v)
            })
            .await
        };
        if explain {
            use crate::memory::storage::LegScores;
            if let Ok(v) = &mut res {
                for r in v.iter_mut() {
                    r.explain = Some(LegScores {
                        fts: None,
                        vector: Some(r.score),
                        fused: Some(r.score),
                        staleness_factor: Some(if r.meta.stale { 0.5 } else { 1.0 }),
                        rerank: None,
                    });
                }
            }
        }
        self.metrics
            .record_search_latency(timer.elapsed(), SearchType::Semantic);
        res.map(|mut v| {
            v.retain(|r| r.score >= self.config.confidence_threshold);
            v
        })
    }

    /// Execute keyword search only (full-text search).
    pub async fn keyword_search(&self, query: &str, limit: usize) -> Result<Vec<SearchResult>> {
        self.keyword_filtered(query, limit, &SearchFilter::default(), false)
            .await
    }

    async fn keyword_filtered(
        &self,
        query: &str,
        limit: usize,
        filter: &SearchFilter,
        explain: bool,
    ) -> Result<Vec<SearchResult>> {
        let timer = Timer::start();
        self.metrics.record_search_query();
        let limit = self.clamp_limit(limit);
        let fetch = if filter.is_restrictive() {
            limit * 4
        } else {
            limit
        };
        let res = self
            .with_timeout(async {
                let raw = self.db.search_fts(query, fetch).await?;
                let mut v = self.refine(raw, filter).await?;
                if explain {
                    use crate::memory::storage::LegScores;
                    for r in &mut v {
                        r.explain = Some(LegScores {
                            fts: Some(r.score),
                            vector: None,
                            fused: Some(r.score),
                            staleness_factor: Some(if r.meta.stale { 0.5 } else { 1.0 }),
                            rerank: None,
                        });
                    }
                }
                v.truncate(limit);
                Ok(v)
            })
            .await;
        self.metrics
            .record_search_latency(timer.elapsed(), SearchType::Keyword);
        res.map(|mut v| {
            v.retain(|r| r.score >= self.config.confidence_threshold);
            v
        })
    }
}
