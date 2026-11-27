use super::*;

#[test]
fn document_url_trims_the_base_and_encodes_the_kind() {
    let config = ClientConfig::new("https://plaesy.example.com/", "prj_1");
    assert_eq!(
        config.document_url("flow-ir/1", "flw_1"),
        "https://plaesy.example.com/v1/projects/prj_1/documents/flow-ir%2F1/flw_1"
    );
}

#[test]
fn authorization_header_only_when_a_token_is_set() {
    let plain = ClientConfig::new("https://x", "prj_1");
    assert!(plain.headers().is_empty());
    let paired = plain.with_token("tok_abc");
    assert_eq!(
        paired.headers(),
        vec![("Authorization".to_owned(), "Bearer tok_abc".to_owned())]
    );
}

#[test]
fn project_is_one_encoded_path_segment() {
    assert_eq!(
        document_path("prj a/b", "data-ir/1", "dat_1"),
        "/v1/projects/prj%20a%2Fb/documents/data-ir%2F1/dat_1"
    );
}

#[test]
fn if_match_quotes_an_unquoted_etag_once() {
    assert_eq!(if_match_value("rev-3"), "\"rev-3\"");
    assert_eq!(if_match_value("\"rev-3\""), "\"rev-3\"");
}

#[test]
fn success_and_redirects_are_not_errors() {
    assert_eq!(map_status(200, None, None), None);
    assert_eq!(map_status(204, None, None), None);
    assert_eq!(map_status(304, None, None), None);
}

#[test]
fn documented_error_statuses_map_to_their_variants() {
    assert_eq!(
        map_status(401, None, None),
        Some(ContractError::Unauthorized)
    );
    assert_eq!(map_status(403, None, None), Some(ContractError::Forbidden));
    assert_eq!(map_status(413, None, None), Some(ContractError::TooLarge));
    assert_eq!(map_status(422, None, None), Some(ContractError::Invalid));
    assert_eq!(
        map_status(428, None, None),
        Some(ContractError::PreconditionRequired)
    );
    assert_eq!(
        map_status(412, Some("\"rev-4\""), None),
        Some(ContractError::Conflict {
            current_etag: Some("\"rev-4\"".to_owned())
        })
    );
}

#[test]
fn rate_limit_reads_retry_after_seconds_and_ignores_dates() {
    assert_eq!(
        map_status(429, None, Some("12")),
        Some(ContractError::RateLimited {
            retry_after_secs: Some(12)
        })
    );
    assert_eq!(
        map_status(429, None, Some("Wed, 21 Oct 2026 07:28:00 GMT")),
        Some(ContractError::RateLimited {
            retry_after_secs: None
        })
    );
}

#[test]
fn other_failures_keep_their_status() {
    assert_eq!(
        map_status(500, None, None),
        Some(ContractError::Other { status: 500 })
    );
}

#[test]
fn errors_are_shown_in_indonesian() {
    assert_eq!(
        ContractError::Forbidden.to_string(),
        "tidak punya izin untuk tindakan ini"
    );
}
