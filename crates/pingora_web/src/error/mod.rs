mod response_error;
mod web_error;

pub use response_error::ResponseError;
pub use web_error::WebError;

use http::StatusCode;

/// Quick error generation functions
pub fn bad_request<T: std::fmt::Display>(msg: T) -> WebError {
    WebError::new(SimpleError::new(StatusCode::BAD_REQUEST, msg.to_string()))
}

pub fn unauthorized<T: std::fmt::Display>(msg: T) -> WebError {
    WebError::new(SimpleError::new(StatusCode::UNAUTHORIZED, msg.to_string()))
}

pub fn forbidden<T: std::fmt::Display>(msg: T) -> WebError {
    WebError::new(SimpleError::new(StatusCode::FORBIDDEN, msg.to_string()))
}

pub fn not_found<T: std::fmt::Display>(msg: T) -> WebError {
    WebError::new(SimpleError::new(StatusCode::NOT_FOUND, msg.to_string()))
}

pub fn unprocessable_entity<T: std::fmt::Display>(msg: T) -> WebError {
    WebError::new(SimpleError::new(
        StatusCode::UNPROCESSABLE_ENTITY,
        msg.to_string(),
    ))
}

pub fn internal_error<T: std::fmt::Display>(msg: T) -> WebError {
    WebError::new(SimpleError::new(
        StatusCode::INTERNAL_SERVER_ERROR,
        msg.to_string(),
    ))
}

pub fn service_unavailable<T: std::fmt::Display>(msg: T) -> WebError {
    WebError::new(SimpleError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        msg.to_string(),
    ))
}

/// Simple error implementation for quick error generation
#[derive(Debug)]
pub struct SimpleError {
    status: StatusCode,
    message: String,
}

impl SimpleError {
    pub fn new(status: StatusCode, message: String) -> Self {
        Self { status, message }
    }
}

impl std::fmt::Display for SimpleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for SimpleError {}

impl ResponseError for SimpleError {
    fn status_code(&self) -> StatusCode {
        self.status
    }
}

// Standard library error implementations
impl ResponseError for std::io::Error {
    fn status_code(&self) -> StatusCode {
        StatusCode::INTERNAL_SERVER_ERROR
    }
}

impl ResponseError for serde_json::Error {
    fn status_code(&self) -> StatusCode {
        StatusCode::BAD_REQUEST
    }
}
