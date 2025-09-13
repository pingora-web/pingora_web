use crate::core::{Method, Request, Response};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

#[async_trait]
pub trait Handler: Send + Sync + 'static {
    /// Process the request and return a response
    async fn handle(&self, req: Request) -> Response;
}

pub struct Router {
    by_method: HashMap<String, matchit::Router<Arc<dyn Handler>>>,
}

impl Router {
    pub fn new() -> Self {
        Self {
            by_method: HashMap::new(),
        }
    }

    pub fn add<S: Into<String>>(&mut self, method: Method, path: S, handler: Arc<dyn Handler>) {
        let key = method.as_str().to_string();
        let r = self.by_method.entry(key).or_default();
        r.insert(path.into(), handler).expect("valid route");
    }

    pub fn get<S: Into<String>>(&mut self, path: S, handler: Arc<dyn Handler>) {
        self.add(Method::GET, path, handler)
    }

    pub fn post<S: Into<String>>(&mut self, path: S, handler: Arc<dyn Handler>) {
        self.add(Method::POST, path, handler)
    }

    pub fn put<S: Into<String>>(&mut self, path: S, handler: Arc<dyn Handler>) {
        self.add(Method::PUT, path, handler)
    }

    pub fn patch<S: Into<String>>(&mut self, path: S, handler: Arc<dyn Handler>) {
        self.add(Method::PATCH, path, handler)
    }

    pub fn delete<S: Into<String>>(&mut self, path: S, handler: Arc<dyn Handler>) {
        self.add(Method::DELETE, path, handler)
    }

    pub fn head<S: Into<String>>(&mut self, path: S, handler: Arc<dyn Handler>) {
        self.add(Method::HEAD, path, handler)
    }

    pub fn options<S: Into<String>>(&mut self, path: S, handler: Arc<dyn Handler>) {
        self.add(Method::OPTIONS, path, handler)
    }

    pub fn connect<S: Into<String>>(&mut self, path: S, handler: Arc<dyn Handler>) {
        self.add(Method::CONNECT, path, handler)
    }

    pub fn trace<S: Into<String>>(&mut self, path: S, handler: Arc<dyn Handler>) {
        self.add(Method::TRACE, path, handler)
    }
}

impl Default for Router {
    fn default() -> Self {
        Self::new()
    }
}

impl Router {
    pub fn find(
        &self,
        method: &Method,
        path: &str,
    ) -> Option<(Arc<dyn Handler>, HashMap<String, String>)> {
        // Try exact method first
        if let Some(r) = self.by_method.get(method.as_str())
            && let Ok(m) = r.at(path)
        {
            let mut params = HashMap::new();
            for (k, v) in m.params.iter() {
                params.insert(k.to_string(), v.to_string());
            }
            return Some((Arc::clone(m.value), params));
        }

        // Per RFC, HEAD should behave like GET without body if no explicit HEAD route is present
        if *method == Method::HEAD
            && let Some(rget) = self.by_method.get(Method::GET.as_str())
            && let Ok(m) = rget.at(path)
        {
            let mut params = HashMap::new();
            for (k, v) in m.params.iter() {
                params.insert(k.to_string(), v.to_string());
            }
            return Some((Arc::clone(m.value), params));
        }

        None
    }

    /// Return a list of methods that match the given path pattern (for 405 responses)
    pub fn allowed_methods(&self, path: &str) -> Vec<String> {
        let mut methods = Vec::new();
        for (m, r) in &self.by_method {
            if r.at(path).is_ok() {
                methods.push(m.clone());
            }
        }
        methods
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct HelloHandler;

    #[async_trait]
    impl Handler for HelloHandler {
        async fn handle(&self, req: Request) -> Response {
            let name = req.param("name").unwrap_or("world");
            Response::text(200, format!("hi {}", name))
        }
    }

    #[tokio::test]
    async fn matchit_basic_param() {
        let mut r = Router::new();
        r.get("/hi/{name}", Arc::new(HelloHandler));

        let (h, params) = r.find(&Method::GET, "/hi/alice").expect("found");
        let req = Request::new(Method::GET, "/hi/alice").with_params(params);
        let res = h.handle(req).await;
        match res.body {
            crate::core::response::Body::Bytes(b) => {
                assert_eq!(std::str::from_utf8(&b).unwrap(), "hi alice");
            }
            _ => panic!("unexpected streaming body"),
        }
    }
}
