//! Services flow: hanya membangun request. Pelaksanaannya lewat `ApiClient`.

use super::super::{ApiClient, ApiRequest};

/// Request untuk menyimpan flow dengan id tertentu.
#[must_use]
pub fn save_request(flow_id: &str, body: &str) -> ApiRequest {
    ApiRequest {
        method: "PUT",
        path: format!("/v1/flows/{flow_id}"),
        body: Some(body.to_owned()),
    }
}

/// Request untuk memuat flow.
#[must_use]
pub fn load_request(flow_id: &str) -> ApiRequest {
    ApiRequest {
        method: "GET",
        path: format!("/v1/flows/{flow_id}"),
        body: None,
    }
}

/// Mengirim request simpan lewat klien yang diberikan.
///
/// # Errors
///
/// Galat dari klien.
pub fn save(client: &dyn ApiClient, flow_id: &str, body: &str) -> Result<String, String> {
    client.send(save_request(flow_id, body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct Recorder(RefCell<Vec<ApiRequest>>);

    impl ApiClient for Recorder {
        fn send(&self, request: ApiRequest) -> Result<String, String> {
            self.0.borrow_mut().push(request);
            Ok("ok".to_owned())
        }
    }

    #[test]
    fn save_builds_a_put_to_the_flow_path() {
        let client = Recorder(RefCell::new(Vec::new()));
        assert_eq!(save(&client, "flw_1", "{}"), Ok("ok".to_owned()));
        let sent = client.0.borrow();
        assert_eq!(sent[0].method, "PUT");
        assert_eq!(sent[0].path, "/v1/flows/flw_1");
    }

    #[test]
    fn load_has_no_body() {
        assert_eq!(load_request("flw_1").body, None);
    }
}
