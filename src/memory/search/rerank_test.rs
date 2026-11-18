//! Unit tests for `tests` split out of `rerank.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn sigmoid_is_monotonic_and_bounded() {
    assert!(sigmoid(-20.0) < 1e-6);
    assert!((sigmoid(0.0) - 0.5).abs() < 1e-6);
    assert!(sigmoid(20.0) > 1.0 - 1e-6);
    assert!(sigmoid(1.0) > sigmoid(0.5));
}

fn model() -> Option<Arc<CrossEncoder>> {
    let dir = CrossEncoder::resolve_model_dir(None)?;
    Some(Arc::new(CrossEncoder::load(&dir, 256).ok()?))
}

#[tokio::test]
async fn relevant_passage_outscores_irrelevant_one() {
    let Some(ce) = model() else {
        eprintln!("SKIPPED: no cross-encoder model installed");
        return;
    };
    let docs = vec![
        "Raft elects a leader that replicates a log to followers.".to_string(),
        "Bananas are rich in potassium and grow in tropical climates.".to_string(),
    ];
    let s = ce
        .score("how does leader election work in a replicated log", docs)
        .await
        .expect("score");
    assert_eq!(s.len(), 2);
    assert!(
        s[0] > s[1] + 2.0,
        "relevant {} vs irrelevant {}",
        s[0],
        s[1]
    );
}

#[tokio::test]
async fn scoring_is_independent_of_batch_composition() {
    let Some(ce) = model() else {
        eprintln!("SKIPPED: no cross-encoder model installed");
        return;
    };
    let a = "Sorting algorithms like quicksort partition an array around a pivot.".to_string();
    let filler = "short".to_string();
    let alone = ce
        .score("quicksort pivot", vec![a.clone()])
        .await
        .expect("alone");
    let mixed = ce
        .score("quicksort pivot", vec![filler, a])
        .await
        .expect("mixed");
    assert!(
        (alone[0] - mixed[1]).abs() < 1e-3,
        "padding must not change scores: {} vs {}",
        alone[0],
        mixed[1]
    );
}
