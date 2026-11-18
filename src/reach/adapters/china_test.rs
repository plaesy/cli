//! Unit tests for `tests` split out of `china.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn an_unknown_action_names_the_backend() {
    let deadline = Deadline::after(std::time::Duration::from_secs(1));
    let mut opts = Options::new();
    opts.insert("action".into(), "delete".into());
    let err = V2exAdapter
        .execute("v2ex-api", "x", &opts, deadline)
        .expect_err("unknown action");
    assert!(err.to_string().contains("v2ex"));
}
