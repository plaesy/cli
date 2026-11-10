//! Image generation through a configured provider (OpenAI or Gemini).
//!
//! The decoded image bytes are written to the requested path. Each provider
//! reads its key from the environment.
//! Port of `scripts/internal/imagegen` (Go).

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use base64::Engine as _;
use serde::Deserialize;
use serde_json::{json, Value};
use thiserror::Error;

const DEFAULT_SIZE: &str = "1024x1024";
const OPENAI_URL: &str = "https://api.openai.com/v1/images/generations";
const GEMINI_URL: &str =
    "https://generativelanguage.googleapis.com/v1beta/models/imagen-3.0-generate-002:predict";

/// Image provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    OpenAi,
    Gemini,
}

impl Provider {
    /// Parses a provider name.
    ///
    /// # Errors
    ///
    /// [`ImageError::UnknownProvider`] for anything other than `openai` or `gemini`.
    pub fn parse(name: &str) -> Result<Self, ImageError> {
        match name {
            "openai" => Ok(Self::OpenAi),
            "gemini" => Ok(Self::Gemini),
            other => Err(ImageError::UnknownProvider(other.to_owned())),
        }
    }
}

/// Parameters for one generation.
#[derive(Debug, Clone, Default)]
pub struct Options {
    pub prompt: String,
    /// `openai` or `gemini`. Empty falls back to `PLAESY_IMAGE_PROVIDER`, then `openai`.
    pub provider: String,
    /// `WIDTHxHEIGHT`. Empty means `1024x1024`.
    pub size: String,
    pub out: PathBuf,
}

/// Errors from image generation.
#[derive(Debug, Error)]
pub enum ImageError {
    #[error("prompt and out are required")]
    MissingArgs,
    #[error("unknown provider {0:?} (supported: openai, gemini)")]
    UnknownProvider(String),
    #[error("{0} is not set. Export it, for example:\n  export {0}=...")]
    MissingKey(&'static str),
    #[error("request failed: {0}")]
    Request(String),
    #[error("{provider} image API: {message}")]
    Api {
        provider: &'static str,
        message: String,
    },
    #[error("{provider} image API: no image data returned")]
    NoImage { provider: &'static str },
    #[error("parsing {provider} response: {message}")]
    Parse {
        provider: &'static str,
        message: String,
    },
    #[error("decoding image data: {0}")]
    Decode(String),
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("{} was not written or is empty", .0.display())]
    Empty(PathBuf),
}

/// Generates an image and writes it to `opts.out`. Returns the path written.
///
/// # Errors
///
/// Missing arguments, an unknown provider, a missing key, a provider error, or I/O.
pub fn generate(opts: &Options) -> Result<PathBuf, ImageError> {
    if opts.prompt.is_empty() || opts.out.as_os_str().is_empty() {
        return Err(ImageError::MissingArgs);
    }
    let provider_name = if opts.provider.is_empty() {
        env::var("PLAESY_IMAGE_PROVIDER").unwrap_or_default()
    } else {
        opts.provider.clone()
    };
    let provider = Provider::parse(if provider_name.is_empty() {
        "openai"
    } else {
        &provider_name
    })?;
    let size = if opts.size.is_empty() {
        DEFAULT_SIZE
    } else {
        &opts.size
    };

    if let Some(parent) = opts.out.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(|source| ImageError::Io {
            context: "creating output directory".into(),
            source,
        })?;
    }

    let data = match provider {
        Provider::OpenAi => openai_image(&opts.prompt, size)?,
        Provider::Gemini => gemini_image(&opts.prompt)?,
    };
    fs::write(&opts.out, &data).map_err(|source| ImageError::Io {
        context: "writing output file".into(),
        source,
    })?;
    if fs::metadata(&opts.out).map_or(true, |m| m.len() == 0) {
        return Err(ImageError::Empty(opts.out.clone()));
    }
    Ok(opts.out.clone())
}

#[derive(Deserialize)]
struct ApiError {
    message: Option<String>,
}

#[derive(Deserialize)]
struct OpenAiResponse {
    error: Option<ApiError>,
    #[serde(default)]
    data: Vec<OpenAiImage>,
}

#[derive(Deserialize)]
struct OpenAiImage {
    #[serde(default)]
    b64_json: String,
}

fn openai_image(prompt: &str, size: &str) -> Result<Vec<u8>, ImageError> {
    let key = env::var("OPENAI_API_KEY").map_err(|_| ImageError::MissingKey("OPENAI_API_KEY"))?;
    let body = json!({ "model": "gpt-image-1", "prompt": prompt, "size": size, "n": 1 });
    let text = post_json(OPENAI_URL, Some(&key), &body)?;
    let parsed: OpenAiResponse = serde_json::from_str(&text).map_err(|e| ImageError::Parse {
        provider: "OpenAI",
        message: e.to_string(),
    })?;
    if let Some(message) = parsed
        .error
        .and_then(|e| e.message)
        .filter(|m| !m.is_empty())
    {
        return Err(ImageError::Api {
            provider: "OpenAI",
            message,
        });
    }
    let first = parsed
        .data
        .into_iter()
        .next()
        .filter(|img| !img.b64_json.is_empty())
        .ok_or(ImageError::NoImage { provider: "OpenAI" })?;
    decode(&first.b64_json)
}

#[derive(Deserialize)]
struct GeminiResponse {
    error: Option<ApiError>,
    #[serde(default)]
    predictions: Vec<GeminiPrediction>,
}

#[derive(Deserialize)]
struct GeminiPrediction {
    #[serde(rename = "bytesBase64Encoded", default)]
    bytes_base64_encoded: String,
}

fn gemini_image(prompt: &str) -> Result<Vec<u8>, ImageError> {
    let key = env::var("GEMINI_API_KEY").map_err(|_| ImageError::MissingKey("GEMINI_API_KEY"))?;
    let body = json!({
        "instances": [{ "prompt": prompt }],
        "parameters": { "sampleCount": 1 },
    });
    let url = format!("{GEMINI_URL}?key={key}");
    let text = post_json(&url, None, &body)?;
    let parsed: GeminiResponse = serde_json::from_str(&text).map_err(|e| ImageError::Parse {
        provider: "Gemini",
        message: e.to_string(),
    })?;
    if let Some(message) = parsed
        .error
        .and_then(|e| e.message)
        .filter(|m| !m.is_empty())
    {
        return Err(ImageError::Api {
            provider: "Gemini",
            message,
        });
    }
    let first = parsed
        .predictions
        .into_iter()
        .next()
        .filter(|p| !p.bytes_base64_encoded.is_empty())
        .ok_or(ImageError::NoImage { provider: "Gemini" })?;
    decode(&first.bytes_base64_encoded)
}

/// POSTs JSON and returns the body, including error bodies. Provider errors are
/// read from the body, so the status code is not turned into a failure here.
fn post_json(url: &str, bearer: Option<&str>, body: &Value) -> Result<String, ImageError> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .build()
        .into();
    let mut request = agent.post(url).header("Content-Type", "application/json");
    if let Some(key) = bearer {
        request = request.header("Authorization", &format!("Bearer {key}"));
    }
    let mut response = request
        .send(body.to_string())
        .map_err(|e| ImageError::Request(e.to_string()))?;
    response
        .body_mut()
        .read_to_string()
        .map_err(|e| ImageError::Request(format!("reading response: {e}")))
}

fn decode(encoded: &str) -> Result<Vec<u8>, ImageError> {
    base64::engine::general_purpose::STANDARD
        .decode(encoded)
        .map_err(|e| ImageError::Decode(e.to_string()))
}

/// Path of a generated file, for display.
#[must_use]
pub fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_names_are_exact() {
        assert_eq!(Provider::parse("openai").unwrap(), Provider::OpenAi);
        assert_eq!(Provider::parse("gemini").unwrap(), Provider::Gemini);
        assert!(matches!(
            Provider::parse("midjourney"),
            Err(ImageError::UnknownProvider(_))
        ));
    }

    #[test]
    fn missing_arguments_are_rejected_before_any_request() {
        assert!(matches!(
            generate(&Options::default()),
            Err(ImageError::MissingArgs)
        ));
    }

    #[test]
    fn base64_payload_decodes() {
        assert_eq!(decode("aGVsbG8=").unwrap(), b"hello");
        assert!(decode("%%%").is_err());
    }
}
