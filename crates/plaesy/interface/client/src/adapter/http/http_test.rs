use super::*;
use serde_json::json;

/// Transport palsu: mencatat permintaan dan mengembalikan balasan tetap.
struct Fake {
    seen: Vec<Request>,
    reply: Result<Response, String>,
}

impl Transport for Fake {
    fn send(&mut self, request: &Request) -> Result<Response, String> {
        self.seen.push(request.clone());
        self.reply.clone()
    }
}

#[test]
fn http_slash_in_kind_is_percent_encoded() {
    let request = get_document("flow-ir/1", "checkout");
    assert_eq!(request.path, "/v1/documents/flow-ir%2F1/checkout");
    assert_eq!(request.method, "GET");
}

#[test]
fn http_put_carries_the_document_as_body() {
    let request = put_document("data-ir/1", "crm", json!({"ir": "data-ir/1"}));
    assert_eq!(request.method, "PUT");
    assert_eq!(request.body, Some(json!({"ir": "data-ir/1"})));
}

#[test]
fn http_routes_for_validate_openapi_and_codegen() {
    assert_eq!(validate(json!({})).path, "/v1/validate");
    assert_eq!(openapi(json!({})).path, "/v1/openapi");
    assert_eq!(
        codegen("js", json!({})).unwrap().path,
        "/v1/codegen/flow/js"
    );
    assert_eq!(
        codegen("go", json!({})).unwrap().path,
        "/v1/codegen/flow/go"
    );
}

#[test]
fn http_unknown_codegen_language_is_refused_before_sending() {
    assert_eq!(
        codegen("rust", json!({})).unwrap_err(),
        ClientError::UnknownLanguage("rust".to_owned())
    );
}

#[test]
fn http_success_returns_the_body() {
    let mut fake = Fake {
        seen: Vec::new(),
        reply: Ok(Response {
            status: 200,
            body: json!({"ok": true}),
        }),
    };
    let value = execute(&mut fake, &get_document("flow-ir/1", "a")).unwrap();
    assert_eq!(value, json!({"ok": true}));
    assert_eq!(fake.seen.len(), 1);
}

#[test]
fn http_not_found_and_invalid_map_to_their_errors() {
    let mut missing = Fake {
        seen: Vec::new(),
        reply: Ok(Response {
            status: 404,
            body: json!({}),
        }),
    };
    assert_eq!(
        execute(&mut missing, &get_document("x", "y")).unwrap_err(),
        ClientError::NotFound
    );

    let mut invalid = Fake {
        seen: Vec::new(),
        reply: Ok(Response {
            status: 422,
            body: json!({"errors": ["x"]}),
        }),
    };
    assert!(matches!(
        execute(&mut invalid, &validate(json!({}))).unwrap_err(),
        ClientError::Invalid(_)
    ));
}

#[test]
fn http_transport_failure_is_reported() {
    let mut broken = Fake {
        seen: Vec::new(),
        reply: Err("koneksi putus".to_owned()),
    };
    assert_eq!(
        execute(&mut broken, &list_documents("flow-ir/1")).unwrap_err(),
        ClientError::Transport("koneksi putus".to_owned())
    );
}
