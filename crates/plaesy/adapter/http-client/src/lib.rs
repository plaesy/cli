//! Klien HTTP untuk langkah flow (ADR 0003: adapter/http-client). Mengimplementasikan port
//! `plaesy_service::port::http::HttpClient` di atas ureq.

use plaesy_service::port::http::{HttpClient, HttpError, HttpRequest, HttpResponse, Method};
use std::io::Read;
use std::time::Duration;

/// Klien ureq. Status HTTP non-2xx dikembalikan sebagai balasan biasa.
#[derive(Debug)]
pub struct UreqClient;

impl HttpClient for UreqClient {
    fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        let config = ureq::config::Config::builder()
            .http_status_as_error(false)
            .timeout_global(Some(request.timeout.max(Duration::from_millis(1))))
            .build();
        let agent = ureq::Agent::new_with_config(config);
        let headers = &request.headers;
        let body = request.body.clone().unwrap_or_default();
        let url = request.url.as_str();

        let outcome = match request.method {
            Method::Get => {
                let mut builder = agent.get(url);
                for (name, value) in headers {
                    builder = builder.header(name.as_str(), value.as_str());
                }
                builder.call()
            }
            Method::Delete => {
                let mut builder = agent.delete(url);
                for (name, value) in headers {
                    builder = builder.header(name.as_str(), value.as_str());
                }
                builder.call()
            }
            Method::Post => {
                let mut builder = agent.post(url);
                for (name, value) in headers {
                    builder = builder.header(name.as_str(), value.as_str());
                }
                builder.send(body.as_bytes())
            }
            Method::Put => {
                let mut builder = agent.put(url);
                for (name, value) in headers {
                    builder = builder.header(name.as_str(), value.as_str());
                }
                builder.send(body.as_bytes())
            }
        };

        let mut response = outcome.map_err(|error| HttpError(error.to_string()))?;
        let status = response.status().as_u16();
        let mut text = String::new();
        response
            .body_mut()
            .as_reader()
            .read_to_string(&mut text)
            .map_err(|error| HttpError(error.to_string()))?;
        Ok(HttpResponse { status, body: text })
    }
}

#[cfg(test)]
#[path = "http_client_test.rs"]
mod http_client_test;
