use crate::core::PingoraWebHttpResponse;
use http::StatusCode;

/// Trait for converting errors into HTTP responses
///
/// This trait is inspired by actix-web's ResponseError trait and provides
/// a simple way to convert errors into appropriate HTTP responses.
pub trait ResponseError: std::error::Error + Send + Sync {
    /// Return the HTTP status code for this error.
    ///
    /// The default implementation returns 500 Internal Server Error.
    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }

    /// Generate an HTTP response for this error.
    ///
    /// The default implementation creates a simple JSON response.
    fn error_response(&self) -> PingoraWebHttpResponse {
        let error_body = serde_json::json!({
            "error": self.to_string()
        });

        PingoraWebHttpResponse::json(self.status_code(), &error_body)
    }
}
