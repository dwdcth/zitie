//! 集成测试公共装配:临时 SQLite + 真实内置字体 + 完整 Router。
#![allow(dead_code)] // 各测试二进制按需取用其中部分函数

use std::sync::Arc;

use axum::body::{Body, Bytes};
use axum::http::{Request, StatusCode};
use axum::Router;
use tower::ServiceExt;

use zitie::config::Config;
use zitie::web::{router, AppState};

pub async fn test_app() -> Router {
    let tmp = tempfile::tempdir().expect("创建临时目录失败");
    // 泄漏临时目录路径,存活整个测试进程(退出时由系统清理)
    let path = tmp.keep();

    let cfg = Arc::new(Config {
        addr: String::new(),
        data_dir: path.clone(),
        assets_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("assets"),
        static_dir: std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("static"),
        cache_quota_mb: 64,
        secure_cookie: false,
    });

    let db = zitie::db::init(&cfg.db_path())
        .await
        .expect("测试数据库初始化失败");
    let fonts = Arc::new(
        zitie::fonts::FontStore::load_builtin(&cfg.builtin_fonts_dir())
            .expect("内置字体加载失败(请先运行 ./scripts/fetch-fonts.sh)"),
    );
    let cache =
        Arc::new(zitie::pdfcache::PdfCache::new(cfg.cache_dir(), 64).expect("缓存目录创建失败"));

    router(Arc::new(AppState {
        db,
        fonts,
        cache,
        cfg,
    }))
}

/// 发送 JSON/空请求,返回 (status, body, set_cookie)
pub async fn req(
    app: &mut Router,
    method: &str,
    uri: &str,
    body: Option<serde_json::Value>,
    cookie: Option<&str>,
) -> (StatusCode, Bytes, Option<String>) {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(c) = cookie {
        builder = builder.header("cookie", c);
    }
    let req = match body {
        Some(v) => builder
            .header("content-type", "application/json")
            .body(Body::from(v.to_string()))
            .unwrap(),
        None => builder.body(Body::empty()).unwrap(),
    };
    let resp = app.clone().oneshot(req).await.unwrap();
    let set_cookie = resp
        .headers()
        .get("set-cookie")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(';').next().unwrap_or("").to_string());
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), 64 * 1024 * 1024)
        .await
        .unwrap();
    (status, body, set_cookie)
}

/// multipart 上传字体文件。
pub async fn upload_font(app: &mut Router, path: &str, cookie: &str) -> (StatusCode, Bytes) {
    let file_bytes = std::fs::read(path).expect("读取字体文件失败");
    let boundary = "----zitietest";
    let body = format!(
        "--{b}\r\ncontent-disposition: form-data; name=\"font\"; filename=\"f.ttf\"\r\ncontent-type: application/octet-stream\r\n\r\n",
        b = boundary
    );
    let mut full = body.into_bytes();
    full.extend_from_slice(&file_bytes);
    full.extend_from_slice(format!("\r\n--{b}--\r\n", b = boundary).as_bytes());

    let req = Request::builder()
        .method("POST")
        .uri("/api/fonts")
        .header("cookie", cookie)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(full))
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), 32 * 1024 * 1024)
        .await
        .unwrap();
    (status, body)
}
