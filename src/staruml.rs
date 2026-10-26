use reqwest::redirect::Policy;
use reqwest::Client as HttpClient;
use serde::Deserialize;
use serde_json::Value;
use std::time::Duration;

const REQUEST_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Clone)]
pub struct Client {
    base_url: String,
    http: HttpClient,
}

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Transport or body-decode failure talking to StarUML.
    #[error("Failed to call API ({slug}): {source}")]
    Http {
        slug: &'static str,
        #[source]
        source: reqwest::Error,
    },
    /// StarUML answered, but the body was not a `{success, data, error}` envelope.
    #[error("Malformed response envelope from {slug}")]
    Envelope {
        slug: &'static str,
        #[source]
        source: serde_json::Error,
    },
    /// StarUML answered `success: false`.
    #[error("Failed to call API ({slug}): {message}")]
    StarUml {
        slug: &'static str,
        message: String,
    },
}

/// The response envelope every StarUML API endpoint answers with.
#[derive(Deserialize)]
struct Envelope {
    success: bool,
    #[serde(default)]
    data: Value,
    #[serde(default)]
    error: Option<String>,
}

impl Client {
    pub fn new(port: u16) -> Self {
        Self {
            base_url: format!("http://localhost:{port}"),
            http: HttpClient::builder()
                .redirect(Policy::none())
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("failed to build HTTP client"),
        }
    }

    pub async fn call(&self, slug: &'static str, body: &Value) -> Result<Value, ApiError> {
        let base_url = &self.base_url;
        let url = format!("{base_url}{slug}");
        let response = self
            .http
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|source| ApiError::Http { slug, source })?;
        let payload: Value = response
            .json()
            .await
            .map_err(|source| ApiError::Http { slug, source })?;
        parse_envelope(slug, payload)
    }
}

fn parse_envelope(slug: &'static str, payload: Value) -> Result<Value, ApiError> {
    let envelope: Envelope = serde_json::from_value(payload)
        .map_err(|source| ApiError::Envelope { slug, source })?;
    if envelope.success {
        Ok(envelope.data)
    } else {
        Err(ApiError::StarUml {
            slug,
            message: envelope.error.unwrap_or_else(|| "unknown error".into()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error as _;

    #[test]
    fn success_envelope_yields_data() {
        let data = parse_envelope("/x", serde_json::json!({"success": true, "data": {"a": 1}})).unwrap();
        assert_eq!(data, serde_json::json!({"a": 1}));
    }

    #[test]
    fn success_without_data_is_null() {
        assert!(parse_envelope("/x", serde_json::json!({"success": true})).unwrap().is_null());
    }

    #[test]
    fn failure_envelope_carries_error_text() {
        let err = parse_envelope("/x", serde_json::json!({"success": false, "error": "Diagram not found"}))
            .unwrap_err();
        assert!(matches!(err, ApiError::StarUml { ref message, .. } if message == "Diagram not found"));
    }

    #[test]
    fn malformed_envelope_preserves_source() {
        let err = parse_envelope("/x", serde_json::json!({"ok": 1})).unwrap_err();
        assert!(matches!(err, ApiError::Envelope { .. }));
        assert!(err.source().is_some());
    }
}
