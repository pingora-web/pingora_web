use std::any::TypeId;
use std::collections::HashMap;

use crate::core::data::AppData;
use bytes::Bytes;
use http::{HeaderMap, HeaderValue, Method, Uri};

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
}
