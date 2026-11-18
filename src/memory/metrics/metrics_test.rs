//! Unit tests for `tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn test_timer_elapsed() {
    let timer = Timer::start();
    std::thread::sleep(Duration::from_millis(10));
    let elapsed = timer.elapsed();
    assert!(elapsed.as_millis() >= 10);
}

#[test]
fn test_timer_elapsed_ms() {
    let timer = Timer::start();
    std::thread::sleep(Duration::from_millis(10));
    let elapsed_ms = timer.elapsed_ms();
    assert!(elapsed_ms >= 10.0);
}

#[test]
fn test_metrics_default() {
    let snap = Metrics::default().snapshot();
    assert_eq!(snap.total_queries, 0);
    assert_eq!(snap.p95_search_latency_ms, 0.0);
    assert_eq!(snap.vector_search_success_rate, 0.0);
}

#[test]
fn test_percentile_nearest_rank() {
    let v: Vec<f64> = (1..=100).map(f64::from).collect();
    assert_eq!(percentile(&v, 95.0), 95.0);
    assert_eq!(percentile(&v, 99.0), 99.0);
    assert_eq!(percentile(&v, 100.0), 100.0);
    assert_eq!(percentile(&[7.0], 50.0), 7.0);
    assert_eq!(percentile(&[], 50.0), 0.0);
}

#[test]
fn test_snapshot_reflects_records() {
    let m = Metrics::new();
    for ms in 1..=100u64 {
        m.record_search_query();
        m.record_search_latency(Duration::from_millis(ms), SearchType::Hybrid);
    }
    m.record_vector_search(true);
    m.record_vector_search(false);
    m.record_error("embedding_timeout");
    m.record_error("embedding_timeout");
    m.record_document_indexed(5);
    m.set_embedding_cache_hit_ratio(0.25);
    let s = m.snapshot();
    assert_eq!(s.total_queries, 100);
    assert!((s.avg_search_latency_ms - 50.5).abs() < 0.5);
    assert!((s.p95_search_latency_ms - 95.0).abs() < 1.0);
    assert_eq!(s.vector_search_success_rate, 0.5);
    assert_eq!(s.total_errors, 2);
    assert_eq!(s.embedding_cache_hit_ratio, 0.25);
    assert_eq!(s.index_health.chunks_total, 5);
}

#[test]
fn test_latency_window_is_bounded() {
    let m = Metrics::new();
    for _ in 0..(LATENCY_WINDOW + 50) {
        m.record_search_latency(Duration::from_millis(1), SearchType::Keyword);
    }
    assert_eq!(m.search_latencies_ms.lock().len(), LATENCY_WINDOW);
}

#[test]
fn test_render_prometheus() {
    let m = Metrics::new();
    m.record_search_query();
    m.record_error("x");
    m.record_indexing_latency(Duration::from_millis(500), "write_chunks");
    let text = m.render_prometheus();
    assert!(text.contains("plaesy_memory_queries_total 1\n"));
    assert!(text.contains("plaesy_memory_errors_total{type=\"x\"} 1\n"));
    assert!(text.contains("plaesy_memory_index_operations_total{operation=\"write_chunks\"} 1\n"));
}
