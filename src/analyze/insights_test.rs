//! Unit tests for `tests` split out of `insights.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn a_small_project_gets_the_extra_recommendation() {
    let small = generate_ai_insights("rust", 3);
    let large = generate_ai_insights("rust", 30);
    assert_eq!(small.recommendations.len(), large.recommendations.len() + 1);
}

#[test]
fn an_unknown_type_gets_the_generic_overview() {
    assert_eq!(
        generate_ai_insights("nope", 30).overview,
        "Generic project structure"
    );
}
