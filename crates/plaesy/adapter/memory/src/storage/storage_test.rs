//! Unit tests for `link_tests` split out of `mod.rs`.
//! Kept as a separate file so they are easy to find; included via `#[path]`.

use super::*;

#[test]
fn wikilinks_parse_aliases_headings_and_skip_noise() {
    let body = "See [[Alpha]] and [[beta/Gamma|the gamma]] plus [[delta#Heading]].\n\
                    ```\n[[in-fence]]\n```\n\
                    [[LLM: verify the claim]] [[{{TOKEN}}]] [[BROWNFIELD ONLY]] [[Real Note]] [[unclosed\n\
                    two on a line [[a]] [[b]] and `[[in-code]]` ignored\n";
    assert_eq!(
        extract_wikilinks(body),
        vec![
            ("Alpha".to_string(), 1),
            ("beta/Gamma".to_string(), 1),
            ("delta".to_string(), 1),
            ("Real Note".to_string(), 5),
            ("a".to_string(), 6),
            ("b".to_string(), 6),
        ]
    );
}

#[test]
fn resolution_is_case_insensitive_and_stable() {
    let paths: Vec<String> = ["notes/Alpha.md", "z/alpha.md", "notes/beta/gamma.md"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    assert_eq!(
        resolve_link("ALPHA", &paths).as_deref(),
        Some("notes/Alpha.md")
    );
    assert_eq!(
        resolve_link("beta/gamma", &paths).as_deref(),
        Some("notes/beta/gamma.md")
    );
    assert_eq!(
        resolve_link("notes/alpha.md", &paths).as_deref(),
        Some("notes/Alpha.md")
    );
    assert_eq!(resolve_link("nope", &paths), None);
}
