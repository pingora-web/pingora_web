use async_trait::async_trait;
// use pingora::apps::http_app::HttpServer; // not needed when passing App directly
use futures::{StreamExt, stream};
use bytes::Bytes;
use pingora::server::Server;
use pingora::services::listening::Service;
use pingora_web::utils::ServeDir;
use pingora_web::{App, Handler, Request, Response, Router, TracingMiddleware, LimitsMiddleware, LimitsConfig, PanicRecoveryMiddleware, CompressionMiddleware, CompressionConfig};
use serde::Serialize;
use serde_json::Value;
use std::sync::Arc;
use tracing_subscriber::{EnvFilter, fmt::format::FmtSpan};

// 定义处理器结构体
struct RootHandler;

impl RootHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl Handler for RootHandler {
    async fn handle(&self, _req: Request) -> Response {
        tracing::info!("处理根路径请求");
        Response::text(200, "ok")
    }
}

struct FooHandler;

impl FooHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl Handler for FooHandler {
    async fn handle(&self, _req: Request) -> Response {
        tracing::info!("处理 /foo 请求");
        Response::text(200, "get_foo")
    }
}

struct FooBarHandler;

impl FooBarHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl Handler for FooBarHandler {
    async fn handle(&self, _req: Request) -> Response {
        tracing::info!("处理 /foo/bar 请求");
        Response::text(200, "foo_bar")
    }
}

// ------------- App-level shared data example -------------
#[derive(Clone)]
struct Cfg {
    banner: &'static str,
}

struct CfgHandler;
impl CfgHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl Handler for CfgHandler {
    async fn handle(&self, req: Request) -> Response {
        if let Some(cfg) = req.get_app_share_data::<Cfg>() {
            Response::text(200, cfg.banner)
        } else {
            Response::text(500, "missing cfg")
        }
    }
}

// Panic处理器，用于测试panic恢复功能
struct PanicHandler;
impl PanicHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl Handler for PanicHandler {
    async fn handle(&self, _req: Request) -> Response {
        tracing::info!("即将触发panic用于测试");
        panic!("这是一个测试panic!");
    }
}

// 慢响应处理器，用于测试超时功能
struct SlowHandler;
impl SlowHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl Handler for SlowHandler {
    async fn handle(&self, _req: Request) -> Response {
        // 延迟35秒，超过默认30秒超时设置
        tracing::info!("开始处理慢请求，将延迟35秒");
        tokio::time::sleep(std::time::Duration::from_secs(35)).await;
        Response::text(200, "这个响应永远不会被看到，因为会超时")
    }
}

// 另一种大数据来源：动态生成/网络代理 -> 流式字节流
struct GeneratedStreamHandler;
impl GeneratedStreamHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl Handler for GeneratedStreamHandler {
    async fn handle(&self, _req: Request) -> Response {
        // 模拟动态生成大数据：每 10ms 生成一块数据，共 100 块
        let mut i = 0u32;
        let s = stream::unfold((), move |_| async move {
            if i >= 100 {
                return None;
            }
            i += 1;
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            Some((Bytes::from(format!("chunk-{}\n", i).into_bytes()), ()))
        });
        Response::stream(200, s.boxed()).header("content-type", "text/plain; charset=utf-8")
    }
}

fn main() {
    // 初始化 tracing，默认 INFO 级别，可通过 RUST_LOG 覆盖
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        // 打印 span 关闭时事件，便于观察请求完成
        .with_span_events(FmtSpan::CLOSE)
        .init();

    // 创建路由器
    let mut router = Router::new();
    router.get("/", RootHandler::new());
    router.get("/foo", FooHandler::new());
    router.get("/foo/bar", FooBarHandler::new());
    router.get("/cfg", CfgHandler::new());
    router.get("/json", JsonHandler::new());
    router.post("/echo_json", EchoJsonHandler::new());

    router.get("/assets/{path}", Arc::new(ServeDir::new(".")));
    router.get("/stream-gen", GeneratedStreamHandler::new());
    router.get("/slow", SlowHandler::new());
    router.get("/panic", PanicHandler::new());
    router.get("/large-text", LargeTextHandler::new());
    router.get("/large-json", LargeJsonHandler::new());

    // 创建应用并添加中间件
    let mut app = App::new(router);
    // 提供 App 级共享数据
    app.set_app_share_data(Arc::new(Cfg {
        banner: "pingora_web",
    }));

    // 配置全局限制中间件
    let limits_config = LimitsConfig::new()
        .request_timeout(std::time::Duration::from_secs(30))  // 30秒超时
        .max_body_size(2 * 1024 * 1024)                      // 2MB 最大请求体
        .max_path_length(1024)                               // 1KB 最大路径长度
        .max_headers(50)                                     // 最多50个头部
        .max_header_size(4 * 1024);                          // 4KB 最大头部大小

    // 中间件顺序：TracingMiddleware在最外层记录所有请求
    app.use_middleware(TracingMiddleware::new());
    app.use_middleware(PanicRecoveryMiddleware::new());
    app.use_middleware(LimitsMiddleware::with_config(limits_config));

    // 配置压缩中间件：压缩级别6，最小压缩大小1KB
    let compression_config = CompressionConfig::new()
        .level(6)
        .min_size(1024);
    app.use_middleware(CompressionMiddleware::with_config(compression_config));
    // 添加请求级共享数据（插入开始时间）
    // router 已在构造时设置

    // Start the Pingora HTTP server using App (ServeHttp)
    if let Err(e) = run_server(app, "0.0.0.0:8080") {
        eprintln!("Pingora server error: {e}");
    }
}

fn run_server(app: App, addr: &str) -> std::io::Result<()> {
    let mut server = Server::new(None).map_err(|e| std::io::Error::other(e.to_string()))?;
    server.bootstrap();

    // 直接把 App 作为 HttpServerApp 传入 Service
    let mut service = Service::new("Web Service HTTP".to_string(), app);
    service.add_tcp(addr);
    server.add_services(vec![Box::new(service)]);

    server.run_forever()
}

// JSON 示例
#[derive(Serialize)]
struct Info {
    ok: bool,
    message: &'static str,
}

struct JsonHandler;
impl JsonHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl Handler for JsonHandler {
    async fn handle(&self, _req: Request) -> Response {
        Response::json(
            200,
            Info {
                ok: true,
                message: "hello",
            },
        )
    }
}

// JSON echo 示例：读取请求体 JSON 并原样返回
struct EchoJsonHandler;
impl EchoJsonHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl Handler for EchoJsonHandler {
    async fn handle(&self, req: Request) -> Response {
        match serde_json::from_slice::<Value>(req.body()) {
            Ok(v) => Response::json(200, v),
            Err(e) => Response::text(400, format!("invalid json: {}", e)),
        }
    }
}

// 大文本处理器：生成大文本内容用于测试压缩
struct LargeTextHandler;
impl LargeTextHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl Handler for LargeTextHandler {
    async fn handle(&self, _req: Request) -> Response {
        // 生成一个大的重复文本，非常适合压缩
        let large_text = "这是一段重复的文本内容，用于测试HTTP压缩功能。".repeat(200);
        Response::text(200, large_text)
    }
}

// 大JSON处理器：生成大JSON数据用于测试压缩
struct LargeJsonHandler;
impl LargeJsonHandler {
    pub fn new() -> Arc<Self> {
        Arc::new(Self)
    }
}

#[async_trait]
impl Handler for LargeJsonHandler {
    async fn handle(&self, _req: Request) -> Response {
        // 生成一个包含大量重复数据的JSON，适合压缩
        let data: Vec<serde_json::Value> = (0..100)
            .map(|i| serde_json::json!({
                "id": i,
                "name": format!("用户-{}", i),
                "description": "这是一个测试用户的详细描述信息，包含了很多重复的内容用于测试压缩效果",
                "metadata": {
                    "created_at": "2024-01-01T00:00:00Z",
                    "updated_at": "2024-01-01T00:00:00Z",
                    "status": "active",
                    "tags": ["测试", "用户", "压缩", "示例"]
                }
            }))
            .collect();

        Response::json(200, serde_json::json!({
            "users": data,
            "total": 100,
            "message": "这是一个大的JSON响应，包含重复数据用于测试压缩功能"
        }))
    }
}
