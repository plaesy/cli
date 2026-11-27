use super::*;

fn request(method: &'static str, path: &str, body: Option<&str>) -> ApiRequest {
    ApiRequest {
        method,
        path: path.to_owned(),
        body: body.map(str::to_owned),
    }
}

#[test]
fn put_then_get_returns_the_stored_body() {
    let store = MemoryStore::default();
    assert_eq!(
        store.send(request("PUT", "/v1/documents/flow-ir%2F1/a", Some("{}"))),
        Ok("{\"ok\":true}".to_owned())
    );
    assert_eq!(
        store.send(request("GET", "/v1/documents/flow-ir%2F1/a", None)),
        Ok("{}".to_owned())
    );
}

#[test]
fn get_of_missing_document_is_an_error() {
    let store = MemoryStore::default();
    assert!(store
        .send(request("GET", "/v1/documents/x/none", None))
        .is_err());
}

#[test]
fn put_without_body_is_rejected() {
    let store = MemoryStore::default();
    assert!(store
        .send(request("PUT", "/v1/documents/x/a", None))
        .is_err());
}

#[test]
fn unsupported_method_is_rejected() {
    let store = MemoryStore::default();
    assert!(store
        .send(request("DELETE", "/v1/documents/x/a", None))
        .is_err());
}

#[test]
fn paths_with_prefix_lists_matching_paths_in_order() {
    let store = MemoryStore::default();
    let paths = [
        "/v1/documents/flow-ir%2F1/b",
        "/v1/documents/flow-ir%2F1/a",
        "/v1/documents/other/c",
    ];
    for path in paths {
        store.send(request("PUT", path, Some("{}"))).expect("put");
    }
    assert_eq!(
        store.paths_with_prefix("/v1/documents/flow-ir%2F1/"),
        vec![
            "/v1/documents/flow-ir%2F1/a".to_owned(),
            "/v1/documents/flow-ir%2F1/b".to_owned(),
        ]
    );
}
