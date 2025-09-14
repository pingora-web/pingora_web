use super::ResponseError;
use crate::core::PingoraWebHttpResponse;

/// Main error type for the web framework, similar to actix_web::Error
///
/// This type wraps any ResponseError. Request-specific context (like
/// request-id) should be handled by middleware or the app layer.
#[derive(Debug)]
pub struct WebError {
    inner: Box<dyn ResponseError>,
}

impl WebError {
    /// Create a new WebError from any ResponseError
    #[track_caller]
    pub fn new<T: ResponseError + 'static>(err: T) -> Self {
        Self {
            inner: Box::new(err),
        }
    }

    /// Get a reference to the underlying ResponseError
    pub fn as_response_error(&self) -> &dyn ResponseError {
        &*self.inner
    }

    /// Convert this error into an HTTP response
    pub fn into_response(self) -> PingoraWebHttpResponse {
        // Log the error
        tracing::error!(
            status_code = %self.inner.status_code(),
            error = %self.inner,
            "Web error occurred",
        );

        // Generate the response
        self.inner.error_response()
    }
}

impl std::fmt::Display for WebError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.inner)
    }
}

impl std::error::Error for WebError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(self.inner.as_ref())
    }
}

// Specific From implementations for common error types
impl From<crate::core::request::FormParseError> for WebError {
    #[track_caller]
    fn from(err: crate::core::request::FormParseError) -> Self {
        Self::new(err)
    }
}

impl From<std::io::Error> for WebError {
    #[track_caller]
    fn from(err: std::io::Error) -> Self {
        Self::new(err)
    }
}

impl From<serde_json::Error> for WebError {
    #[track_caller]
    fn from(err: serde_json::Error) -> Self {
        Self::new(err)
    }
}

impl From<crate::error::SimpleError> for WebError {
    #[track_caller]
    fn from(err: crate::error::SimpleError) -> Self {
        Self::new(err)
    }
}

// Implement ResponseError for WebError to allow nested errors
impl ResponseError for WebError {
    fn status_code(&self) -> http::StatusCode {
        self.inner.status_code()
    }

    fn error_response(&self) -> PingoraWebHttpResponse {
        self.inner.error_response()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::SimpleError;
    use http::StatusCode;

    #[test]
    fn test_web_error_creation() {
        let simple_err = SimpleError::new(StatusCode::BAD_REQUEST, "Test error".to_string());
        let web_err = WebError::new(simple_err);

        assert_eq!(
            web_err.as_response_error().status_code(),
            StatusCode::BAD_REQUEST
        );
        assert_eq!(web_err.to_string(), "Test error");
    }

    // no request-id coupling inside WebError

    #[test]
    fn test_web_error_from_conversion() {
        let simple_err = SimpleError::new(StatusCode::BAD_REQUEST, "Test error".to_string());
        let web_err: WebError = simple_err.into();

        assert_eq!(
            web_err.as_response_error().status_code(),
            StatusCode::BAD_REQUEST
        );
    }
}
