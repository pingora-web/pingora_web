# 🚀 pingora_web - 基于 Pingora 的极简高性能 Web 框架

[![CI](https://github.com/pingora-web/pingora_web/actions/workflows/ci.yml/badge.svg)](https://github.com/pingora-web/pingora_web/actions/workflows/ci.yml)
[![Crates.io](https://img.shields.io/crates/v/pingora_web.svg)](https://crates.io/crates/pingora_web)
[![Documentation](https://docs.rs/pingora_web/badge.svg)](https://docs.rs/pingora_web)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Downloads](https://img.shields.io/crates/d/pingora_web.svg)](https://crates.io/crates/pingora_web)
[![Stars](https://img.shields.io/github/stars/pingora-web/pingora_web.svg)](https://github.com/pingora-web/pingora_web)

**🔥 快速上手 | 基于 Pingora | 新手友好** 🦀

[English](README.md) | [中文](README_zh.md)

基于 Cloudflare Pingora 代理基础设施构建的 Web 框架，设计快速、可靠且易于使用。

## ✨ 特性

### 核心功能
- 🛣️ **路径路由** 支持参数 (`/users/{id}`)
- 🧅 **中间件系统** 洋葱模型 (类似 Express.js)
- 🏷️ **请求ID追踪** 自动生成 `x-request-id` 头部
- 📝 **结构化日志** 与 tracing 集成
- 📦 **JSON支持** 自动序列化
- 📁 **静态文件服务** 带MIME类型检测
- 🌊 **流式响应** 处理大数据传输

### 基于 Pingora
- ⚡ **高性能** - 利用 Cloudflare 的生产级代理
- 🗜️ **HTTP压缩** - 内置 gzip 支持
- 🛡️ **请求限制** - 超时、体积和头部约束
- 🚨 **异常恢复** - 自动错误处理
- 🔗 **HTTP/1.1 & HTTP/2** 通过 Pingora 支持

## 🚀 快速开始

### 1. 创建新项目
```bash
cargo new my_api && cd my_api
```

### 2. 添加依赖到 `Cargo.toml`
```toml
[dependencies]
pingora_web = "0.1"
pingora = "0.6"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
async-trait = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
```

### 3. Hello World (5行代码 - 类似 Express/Gin)

```rust
use pingora_web::{App, StatusCode, PingoraWebHttpResponse, WebError, PingoraHttpRequest};

fn main() {
    let mut app = App::default();
    app.get_fn("/", |_req: PingoraHttpRequest| -> Result<PingoraWebHttpResponse, WebError> {
        Ok(PingoraWebHttpResponse::text(StatusCode::OK, "你好世界!"))
    });
    app.listen("0.0.0.0:8080").unwrap();
}
```

### 4. 带参数路由 (新手友好)

```rust
use pingora_web::{App, StatusCode, PingoraWebHttpResponse, WebError, PingoraHttpRequest};

fn main() {
    let mut app = App::default();
    app.get_fn("/", |_req: PingoraHttpRequest| -> Result<PingoraWebHttpResponse, WebError> {
        Ok(PingoraWebHttpResponse::text(StatusCode::OK, "你好世界!"))
    });
    app.get_fn("/hi/{name}", |req: PingoraHttpRequest| -> Result<PingoraWebHttpResponse, WebError> {
        let name = req.param("name").unwrap_or("世界");
        Ok(PingoraWebHttpResponse::text(StatusCode::OK, format!("你好 {}", name)))
    });
    app.listen("0.0.0.0:8080").unwrap();
}
```

### 5. 完整功能示例

```rust
use async_trait::async_trait;
use pingora_web::{App, Handler, StatusCode, TracingMiddleware, ResponseCompressionBuilder, WebError, PingoraHttpRequest, PingoraWebHttpResponse};
use std::sync::Arc;

struct Hello;
#[async_trait]
impl Handler for Hello {
    async fn handle(&self, req: PingoraHttpRequest) -> Result<PingoraWebHttpResponse, WebError> {
        let name = req.param("name").unwrap_or("世界");
        Ok(PingoraWebHttpResponse::text(StatusCode::OK, format!("你好 {}", name)))
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let mut app = App::default();
    app.get("/hi/{name}", Arc::new(Hello));
    app.use_middleware(TracingMiddleware::new());
    app.add_http_module(ResponseCompressionBuilder::enable(6));

    // 一行代码启动服务器
    app.listen("0.0.0.0:8080").unwrap();
}
```

### 6. 运行服务器
```bash
cargo run
```

访问 `http://localhost:8080/hi/世界` 查看效果！

### 高级用法 (复杂设置)

如果需要更多服务器配置控制：

```rust
use async_trait::async_trait;
use pingora_web::{App, Handler, StatusCode, WebError, PingoraHttpRequest, PingoraWebHttpResponse};
use pingora::server::Server;
use std::sync::Arc;

struct Hello;
#[async_trait]
impl Handler for Hello {
    async fn handle(&self, req: PingoraHttpRequest) -> Result<PingoraWebHttpResponse, WebError> {
        let name = req.param("name").unwrap_or("世界");
        Ok(PingoraWebHttpResponse::text(StatusCode::OK, format!("你好 {}", name)))
    }
}

fn main() {
    let mut app = App::default();
    app.get("/hi/{name}", Arc::new(Hello));
    let app = app;

    // 高级：转换为服务以获得更多控制
    let mut service = app.to_service("my-web-app");
    service.add_tcp("0.0.0.0:8080");
    service.add_tcp("[::]:8080"); // IPv6 支持

    let mut server = Server::new(None).unwrap();
    server.bootstrap();
    server.add_service(service);

    // 添加监控端点
    let mut prometheus_service = pingora::services::listening::Service::prometheus_http_service();
    prometheus_service.add_tcp("127.0.0.1:9090");
    server.add_service(prometheus_service);

    server.run_forever();
}
```


## 📖 文档

- **[架构与设计理念](ARCHITECTURE.md)** - 详细的设计决策和架构模式说明

## 🎯 使用场景

专为以下应用场景设计：
- **REST API** 和微服务架构
- **Web 应用程序** 后端服务
- **实时应用** 需要低延迟的系统
- **高并发应用** 需要处理大量连接的场景

## 📚 完整示例

### JSON API 响应

```rust
use serde::Serialize;

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    message: String,
    data: Vec<String>,
}

struct JsonHandler;
#[async_trait]
impl Handler for JsonHandler {
    async fn handle(&self, _req: PingoraHttpRequest) -> Result<PingoraWebHttpResponse, WebError> {
        let response = ApiResponse {
            success: true,
            message: "来自 JSON API 的问候".to_string(),
            data: vec!["项目1".to_string(), "项目2".to_string()],
        };
        Ok(PingoraWebHttpResponse::json(StatusCode::OK, response))
    }
}

// 添加到路由:
// router.get("/api/data", Arc::new(JsonHandler));
```

### 静态文件服务

```rust
use std::sync::Arc;
use pingora_web::App;
use pingora_web::utils::ServeDir;

fn setup_app() -> App {
    let mut app = App::default();
    // 从 ./public 目录提供静态文件
    app.get("/static/{path}", Arc::new(ServeDir::new("./public")));
    // 或从当前目录提供
    app.get("/assets/{path}", Arc::new(ServeDir::new(".")));
    app
}
```

使用其他 HTTP 方法时，可通过 `add` 指定方法：

```rust
use std::sync::Arc;
use pingora_web::{App, Method, Handler, PingoraHttpRequest, PingoraWebHttpResponse, WebError};

struct PutHandler;
#[async_trait::async_trait]
impl Handler for PutHandler {
    async fn handle(&self, _req: PingoraHttpRequest) -> Result<PingoraWebHttpResponse, WebError> {
        Ok(PingoraWebHttpResponse::no_content())
    }
}

fn main() {
    let mut app = App::default();
    app.add(Method::PUT, "/resource/{id}", Arc::new(PutHandler));
}
```

<!-- 移除 SSE 章节：当前版本不提供内置 SSE 封装 -->

## 🔧 开发指南

### 环境要求

- Rust 1.75 或更高版本
- Git

### 构建项目

```bash
git clone https://github.com/pingora-web/pingora_web.git
cd pingora_web
cargo build
```

### 运行测试

```bash
cargo test
```

### 代码质量检查

本项目使用多种工具确保代码质量：

```bash
# 格式化代码
cargo fmt

# 代码检查
cargo clippy --all-targets --all-features -- -D warnings

# 安全审计
cargo audit
```

### 运行示例

```bash
cargo run --example pingora_example
```

然后访问：
- `http://localhost:8080/` - 基础响应
- `http://localhost:8080/foo` - 静态路由
- `http://localhost:8080/hi/你的名字` - 带参数路由
- `http://localhost:8080/json` - JSON 响应
- `http://localhost:8080/assets/README.md` - 静态文件服务

## 🚀 发布流程

本项目通过 GitHub Actions 实现自动化发布：

1. **创建新标签**: `git tag v0.1.1 && git push origin v0.1.1`
2. **GitHub Actions 将自动**:
   - 运行所有测试和质量检查
   - 创建包含自动生成说明的 GitHub Release
   - 发布到 [crates.io](https://crates.io)
   - 验证发布成功

### 配置自动发布

要启用自动发布到 crates.io：

1. 从 [crates.io/me](https://crates.io/me) 获取 API token
2. 在仓库设置中添加名为 `CARGO_REGISTRY_TOKEN` 的 secret
3. 推送版本标签即可触发发布工作流

## 🤝 贡献指南

1. Fork 本仓库
2. 创建功能分支
3. 进行修改
4. 运行测试和质量检查
5. 提交 Pull Request

所有 Pull Request 都会通过 GitHub Actions 自动测试。

## 🔗 相关链接

- **文档**: [docs.rs/pingora_web](https://docs.rs/pingora_web)
- **源码**: [github.com/pingora-web/pingora_web](https://github.com/pingora-web/pingora_web)
- **包管理**: [crates.io/crates/pingora_web](https://crates.io/crates/pingora_web)
- **问题反馈**: [GitHub Issues](https://github.com/pingora-web/pingora_web/issues)

## 📄 许可证

本项目采用双许可证：
- MIT
- Apache-2.0

任选其一。

---

**⭐ 如果这个项目对你有帮助，请给我们一个星标！**
