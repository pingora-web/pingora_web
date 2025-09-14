use std::any::TypeId;
use std::collections::HashMap;

use crate::core::data::AppData;
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, Uri};
use serde::de::DeserializeOwned;

#[derive(Debug)]
pub struct Request {
    pub inner: http::Request<Bytes>,
    pub params: HashMap<String, String>,
    pub app_data: Option<std::sync::Arc<AppData>>, // App-level shared data
    pub extensions: HashMap<TypeId, std::sync::Arc<dyn std::any::Any + Send + Sync>>, // request-level data
}

// impl Clone for Request {
//     fn clone(&self) -> Self {
//         // Clone the http::Request manually since http::Request<B> doesn't impl Clone
//         let mut builder = http::Request::builder()
//             .method(self.inner.method())
//             .uri(self.inner.uri())
//             .version(self.inner.version());
//
//         // Clone headers
//         for (name, value) in self.inner.headers() {
//             builder = builder.header(name, value);
//         }
//
//         let cloned_inner = builder
//             .body(self.inner.body().clone())
//             .expect("Failed to clone request");
//
//         Self {
//             inner: cloned_inner,
//             params: self.params.clone(),
//             app_data: self.app_data.clone(),
//             extensions: self.extensions.clone(),
//         }
//     }
// }

impl Request {
    pub fn new<M: Into<Method>, S: AsRef<str>>(method: M, path: S) -> Self {
        let inner = http::Request::builder()
            .method(method.into())
            .uri(path.as_ref())
            .body(Bytes::new())
            .expect("Failed to build request");

        Self {
            inner,
            params: HashMap::new(),
            app_data: None,
            extensions: HashMap::new(),
        }
    }

    pub fn header<K, V>(mut self, k: K, v: V) -> Self
    where
        K: TryInto<http::HeaderName>,
        V: TryInto<HeaderValue>,
        K::Error: std::fmt::Debug,
        V::Error: std::fmt::Debug,
    {
        if let (Ok(key), Ok(value)) = (k.try_into(), v.try_into()) {
            self.inner.headers_mut().insert(key, value);
        }
        self
    }

    pub fn with_body<B: Into<Bytes>>(mut self, body: B) -> Self {
        *self.inner.body_mut() = body.into();
        self
    }

    // Convenience accessors for the inner http::Request
    pub fn method(&self) -> &Method {
        self.inner.method()
    }

    pub fn uri(&self) -> &Uri {
        self.inner.uri()
    }

    pub fn path(&self) -> &str {
        self.inner.uri().path()
    }

    pub fn headers(&self) -> &HeaderMap<HeaderValue> {
        self.inner.headers()
    }

    pub fn headers_mut(&mut self) -> &mut HeaderMap<HeaderValue> {
        self.inner.headers_mut()
    }

    pub fn body(&self) -> &Bytes {
        self.inner.body()
    }

    pub fn with_params(mut self, params: HashMap<String, String>) -> Self {
        self.params = params;
        self
    }

    pub fn with_app_data(mut self, app_data: std::sync::Arc<AppData>) -> Self {
        self.app_data = Some(app_data);
        self
    }

    pub fn param(&self, name: &str) -> Option<&str> {
        self.params.get(name).map(|s| s.as_str())
    }

    pub fn param_or<'a>(&'a self, name: &str, default: &'a str) -> &'a str {
        self.param(name).unwrap_or(default)
    }

    // --- Request-level shared data (extensions) ---
    pub fn set_request_share_data<T: Send + Sync + 'static>(
        &mut self,
        value: std::sync::Arc<T>,
    ) -> Option<std::sync::Arc<T>> {
        let type_id = TypeId::of::<T>();
        let prev = self.extensions.insert(
            type_id,
            value as std::sync::Arc<dyn std::any::Any + Send + Sync>,
        );
        if let Some(prev_any) = prev {
            prev_any.downcast::<T>().ok()
        } else {
            None
        }
    }

    // --- Beginner-friendly aliases ---
    pub fn get_request_share_data<T: Send + Sync + 'static>(&self) -> Option<std::sync::Arc<T>> {
        let type_id = TypeId::of::<T>();
        if let Some(stored) = self.extensions.get(&type_id) {
            let cloned = stored.clone();
            cloned.downcast::<T>().ok()
        } else {
            None
        }
    }

    pub fn get_app_share_data<T: Send + Sync + 'static>(&self) -> Option<std::sync::Arc<T>> {
        if let Some(app_data) = &self.app_data {
            app_data.get::<T>()
        } else {
            None
        }
    }

    // (removed deprecated aliases)

    // --- Form data parsing ---

    /// Parse form data as application/x-www-form-urlencoded
    pub fn parse_form<T>(&self) -> Result<T, FormParseError>
    where
        T: DeserializeOwned,
    {
        let content_type = self
            .headers()
            .get("content-type")
            .and_then(|ct| ct.to_str().ok())
            .unwrap_or("");

        if !content_type.starts_with("application/x-www-form-urlencoded") {
            return Err(FormParseError::InvalidContentType(content_type.to_string()));
        }

        let body_str =
            std::str::from_utf8(self.body()).map_err(|e| FormParseError::Utf8Error(e))?;

        serde_urlencoded::from_str(body_str)
            .map_err(|e| FormParseError::DeserializeError(e.to_string()))
    }
}

/// Form data parsing errors
#[derive(Debug)]
pub enum FormParseError {
    InvalidContentType(String),
    Utf8Error(std::str::Utf8Error),
    DeserializeError(String),
}

impl std::fmt::Display for FormParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FormParseError::InvalidContentType(ct) => write!(f, "Invalid content type: {}", ct),
            FormParseError::Utf8Error(e) => write!(f, "UTF-8 error: {}", e),
            FormParseError::DeserializeError(e) => write!(f, "Deserialization error: {}", e),
        }
    }
}

impl std::error::Error for FormParseError {}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize, Debug, PartialEq)]
    struct LoginForm {
        username: String,
        password: String,
    }

    #[test]
    fn test_parse_form_urlencoded() {
        let req = Request::new(Method::POST, "/login")
            .header("content-type", "application/x-www-form-urlencoded")
            .with_body("username=alice&password=secret123");

        let form: LoginForm = req.parse_form().expect("parse form");
        assert_eq!(form.username, "alice");
        assert_eq!(form.password, "secret123");
    }

    #[test]
    fn test_parse_form_simple() {
        let req = Request::new(Method::POST, "/form")
            .header("content-type", "application/x-www-form-urlencoded")
            .with_body("name=John&email=john@example.com&age=30");

        let form: HashMap<String, String> = req.parse_form().expect("parse form");
        assert_eq!(form.get("name"), Some(&"John".to_string()));
        assert_eq!(form.get("email"), Some(&"john@example.com".to_string()));
        assert_eq!(form.get("age"), Some(&"30".to_string()));
    }

    #[test]
    fn test_parse_form_invalid_content_type() {
        let req = Request::new(Method::POST, "/login")
            .header("content-type", "application/json")
            .with_body(r#"{"username": "alice"}"#);

        let result: Result<LoginForm, _> = req.parse_form();
        assert!(result.is_err());
        match result.unwrap_err() {
            FormParseError::InvalidContentType(ct) => assert_eq!(ct, "application/json"),
            _ => panic!("expected InvalidContentType error"),
        }
    }

    #[test]
    fn test_urlencoded_special_characters() {
        let req = Request::new(Method::POST, "/form")
            .header("content-type", "application/x-www-form-urlencoded")
            .with_body("message=Hello%20World%21&symbol=%26%3D%3F");

        let form: HashMap<String, String> = req.parse_form().expect("parse form");
        assert_eq!(form.get("message"), Some(&"Hello World!".to_string()));
        assert_eq!(form.get("symbol"), Some(&"&=?".to_string()));
    }
}
