use bytes::Bytes;
use futures::stream::BoxStream;
use http::{HeaderMap, HeaderValue, StatusCode};
use tokio::io::AsyncReadExt;

pub struct Response {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Body,
}

impl Response {
    pub fn new(status: u16) -> Self {
        Self {
            status: StatusCode::from_u16(status).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR),
            headers: HeaderMap::new(),
            body: Body::Bytes(Bytes::new()),
        }
    }


    pub fn text<S: Into<String>>(status: u16, body: S) -> Self {
        let mut res = Self::new(status);
        res.headers.insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
        let bytes = body.into().into_bytes();
        res.body = Body::Bytes(Bytes::from(bytes));
        res
    }

    /// Construct an empty response with given status. Does not set content-type.
    pub fn empty(status: u16) -> Self {
        let mut res = Self::new(status);
        res.body = Body::Bytes(Bytes::new());
        res
    }

    /// Construct an HTML response with UTF-8 charset.
    pub fn html<S: Into<String>>(status: u16, body: S) -> Self {
        let mut res = Self::new(status);
        res.headers.insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("text/html; charset=utf-8"),
        );
        let bytes = body.into().into_bytes();
        res.body = Body::Bytes(Bytes::from(bytes));
        res
    }

    /// Construct a raw bytes response. Does not set content-type.
    pub fn bytes(status: u16, body: impl Into<Bytes>) -> Self {
        let mut res = Self::new(status);
        res.body = Body::Bytes(body.into());
        res
    }

    /// Construct a JSON response from any serializable value.
    pub fn json(status: u16, value: impl serde::Serialize) -> Self {
        let mut res = Self::new(status);
        res.headers.insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );

        match serde_json::to_vec(&value) {
            Ok(bytes) => {
                res.body = Body::Bytes(Bytes::from(bytes));
                res
            }
            Err(_) => {
                // serialization failed; return 500 with empty JSON body
                res.status = StatusCode::INTERNAL_SERVER_ERROR;
                res.body = Body::Bytes(Bytes::new());
                res
            }
        }
    }

    /// Construct a streaming file response. Will not buffer the entire file in memory.
    pub fn stream_file<P: AsRef<std::path::Path>>(status: u16, path: P) -> Self {
        let mut res = Self::new(status);
        let ct = mime_guess::from_path(path.as_ref()).first_or_octet_stream();
        let _ = res
            .headers
            .insert(http::header::CONTENT_TYPE, HeaderValue::from_str(ct.as_ref()).unwrap_or(HeaderValue::from_static("application/octet-stream")));

        // For files, we can set content-length if we know the file size
        if let Ok(meta) = std::fs::metadata(path.as_ref()) {
            let len_s = meta.len().to_string();
            let _ = res
                .headers
                .insert(http::header::CONTENT_LENGTH, HeaderValue::from_str(&len_s).unwrap_or(HeaderValue::from_static("0")));
        }

        // Build an async stream that reads the file chunk by chunk
        let pathbuf = path.as_ref().to_path_buf();
        let stream = futures::stream::unfold(
            Some((None::<tokio::fs::File>, pathbuf)),
            |state| async move {
                let (opt_file, path) = state?;
                // Open file lazily on first pull
                let mut file = match opt_file {
                    Some(f) => f,
                    None => match tokio::fs::File::open(&path).await {
                        Ok(f) => f,
                        Err(_) => return None,
                    },
                };
                let mut buf = vec![0u8; 64 * 1024];
                match file.read(&mut buf).await {
                    Ok(0) => None,
                    Ok(n) => {
                        buf.truncate(n);
                        Some((Bytes::from(buf), Some((Some(file), path))))
                    }
                    Err(_) => None,
                }
            },
        );
        res.body = Body::Stream(Box::pin(stream));
        res
    }

    /// Construct a streaming response from a boxed stream of Bytes chunks
    pub fn stream(status: u16, stream: BoxStream<'static, Bytes>) -> Self {
        let mut res = Self::new(status);
        res.body = Body::Stream(stream);
        res
    }

    pub fn set_header<K, V>(&mut self, k: K, v: V)
    where
        K: TryInto<http::HeaderName>,
        V: TryInto<HeaderValue>,
        K::Error: std::fmt::Debug,
        V::Error: std::fmt::Debug,
    {
        if let (Ok(key), Ok(value)) = (k.try_into(), v.try_into()) {
            self.headers.insert(key, value);
        }
    }

    pub fn header<K, V>(mut self, k: K, v: V) -> Self
    where
        K: TryInto<http::HeaderName>,
        V: TryInto<HeaderValue>,
        K::Error: std::fmt::Debug,
        V::Error: std::fmt::Debug,
    {
        self.set_header(k, v);
        self
    }
}

pub enum Body {
    Bytes(Bytes),
    Stream(BoxStream<'static, Bytes>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn json_builds_response() {
        let v = json!({"a": 1, "b": "x"});
        let res = Response::json(200, &v);
        assert_eq!(res.status.as_u16(), 200);
        assert_eq!(
            res.headers
                .get(http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("application/json")
        );
        match res.body {
            Body::Bytes(b) => assert_eq!(b.as_ref(), serde_json::to_vec(&v).unwrap().as_slice()),
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn html_and_empty_and_bytes() {
        let res = Response::html(200, "<h1>ok</h1>");
        assert_eq!(res.status.as_u16(), 200);
        assert_eq!(
            res.headers
                .get(http::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok()),
            Some("text/html; charset=utf-8")
        );
        // content-length should be set by App.handle(), not here
        assert!(!res.headers.contains_key(http::header::CONTENT_LENGTH));

        let res = Response::empty(204);
        assert_eq!(res.status.as_u16(), 204);
        // content-length should be set by App.handle(), not here
        assert!(!res.headers.contains_key(http::header::CONTENT_LENGTH));

        let res = Response::bytes(201, Bytes::from(vec![1, 2, 3]));
        assert_eq!(res.status.as_u16(), 201);
        // content-length should be set by App.handle(), not here
        assert!(!res.headers.contains_key("content-length"));
        match res.body {
            Body::Bytes(b) => assert_eq!(b.as_ref(), &[1, 2, 3]),
            _ => panic!("expected bytes body"),
        }
    }

    #[test]
    fn response_constructors() {
        // Test that constructors create proper bodies without setting content-length
        let res = Response::text(200, "hello world");
        assert_eq!(res.headers.get(http::header::CONTENT_TYPE).unwrap(), &HeaderValue::from_static("text/plain; charset=utf-8"));
        // content-length should be set by App.handle(), not here
        assert!(!res.headers.contains_key("content-length"));

        // Test streaming response constructor
        use futures::StreamExt;
        let stream = futures::stream::iter(vec![
            Bytes::from_static(b"chunk1"),
            Bytes::from_static(b"chunk2"),
        ]);
        let res = Response::stream(200, stream.boxed());
        // Neither content-length nor transfer-encoding should be set by constructor
        assert!(!res.headers.contains_key(http::header::CONTENT_LENGTH));
        assert!(!res.headers.contains_key(http::header::TRANSFER_ENCODING));
    }

    #[test]
    fn manual_headers_not_overridden() {
        // Test that manually set headers are preserved
        let mut res = Response::text(200, "hello");
        res.set_header("content-length", "999");
        // Manual content-length should be preserved
        assert_eq!(res.headers.get(http::header::CONTENT_LENGTH).unwrap(), &HeaderValue::from_static("999"));
    }
}
