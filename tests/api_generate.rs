mod common;

use axum::body::{Body, Bytes};
use axum::http::{Request, StatusCode};
use axum::Router;
use common::{req, test_app};
use tower::ServiceExt;

async fn generate(app: &mut Router, cookie: &str, params: serde_json::Value) -> (StatusCode, serde_json::Value) {
    let (status, body, _) = req(app, "POST", "/api/generate", Some(params), Some(cookie)).await;
    let v = serde_json::from_slice(&body).unwrap_or(serde_json::Value::Null);
    (status, v)
}

async fn get_pdf(app: &mut Router, uri: &str, cookie: Option<&str>, extra: &[(&str, &str)]) -> axum::http::Response<Body> {
    let mut builder = Request::builder().uri(uri);
    if let Some(c) = cookie {
        builder = builder.header("cookie", c);
    }
    for (k, v) in extra {
        builder = builder.header(*k, *v);
    }
    app.clone()
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

fn basic_params() -> serde_json::Value {
    serde_json::json!({
        "text": "永和九年",
        "paper": "mizi",
        "mode": "miaohong",
        "trace_count": 3,
        "copy_count": 3,
        "font_id": "lxgw-wenkai"
    })
}

#[tokio::test]
async fn generate_then_fetch_pdf_full_pipeline() {
    let mut app = test_app().await;
    let (_, _, cookie) = req(&mut app, "GET", "/api/preferences", None, None).await;
    let cookie = cookie.unwrap();

    // 生成
    let (status, v) = generate(&mut app, &cookie, basic_params()).await;
    assert_eq!(status, StatusCode::OK, "{v:?}");
    let url = v["url"].as_str().unwrap().to_string();
    assert!(url.starts_with("/api/pdf/") && url.ends_with(".pdf"));
    assert_eq!(v["pages"], 1);

    // 惰性渲染获取 PDF
    let resp = get_pdf(&mut app, &url, None, &[]).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers().get("content-type").unwrap(), "application/pdf");
    assert_eq!(
        resp.headers().get("cache-control").unwrap(),
        "public, max-age=31536000, immutable"
    );
    let etag = resp.headers().get("etag").unwrap().to_str().unwrap().to_string();
    let bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024 * 1024).await.unwrap();
    assert!(bytes.starts_with(b"%PDF"));

    // 二次请求(磁盘缓存)+ 304 协商
    let resp = get_pdf(&mut app, &url, None, &[("if-none-match", &etag)]).await;
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);

    // 下载模式
    let resp = get_pdf(&mut app, &format!("{url}?dl=1"), None, &[]).await;
    assert!(resp
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("attachment"));
    let _ = axum::body::to_bytes(resp.into_body(), 64 * 1024 * 1024).await;
}

#[tokio::test]
async fn same_params_same_hash_across_users() {
    let mut app = test_app().await;
    let (_, _, c1) = req(&mut app, "GET", "/api/preferences", None, None).await;
    let (_, _, c2) = req(&mut app, "GET", "/api/preferences", None, None).await;
    let (_, v1) = generate(&mut app, c1.as_deref().unwrap(), basic_params()).await;
    let (_, v2) = generate(&mut app, c2.as_deref().unwrap(), basic_params()).await;
    assert_eq!(v1["hash"], v2["hash"], "内容寻址:同参数同哈希,与用户无关");
}

#[tokio::test]
async fn invalid_params_rejected() {
    let mut app = test_app().await;
    let (_, _, cookie) = req(&mut app, "GET", "/api/preferences", None, None).await;
    let cookie = cookie.unwrap();

    // 描红格数越界
    let mut p = basic_params();
    p["trace_count"] = 99.into();
    let (status, v) = generate(&mut app, &cookie, p).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(v["error"]["message"].as_str().unwrap().contains("描红"));

    // 不存在的字体
    let mut p = basic_params();
    p["font_id"] = "no-such-font".into();
    let (status, _) = generate(&mut app, &cookie, p).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // 字体缺字
    let mut p = basic_params();
    p["font_id"] = "shortstack".into();
    let (status, v) = generate(&mut app, &cookie, p).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(v["error"]["message"].as_str().unwrap().contains("永"));
}

#[tokio::test]
async fn bad_pdf_paths_are_404() {
    let mut app = test_app().await;
    let fake = "a".repeat(64);
    for uri in [
        "/api/pdf/not-a-hash.pdf",
        &format!("/api/pdf/{fake}.pdf"),
        "/api/pdf/../../etc/passwd.pdf",
        "/api/pdf/no-extension",
    ] {
        let resp = get_pdf(&mut app, uri, None, &[]).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND, "{uri} 应 404");
        let _ = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await;
    }
}

#[tokio::test]
async fn empty_text_blank_sheet() {
    let mut app = test_app().await;
    let (_, _, cookie) = req(&mut app, "GET", "/api/preferences", None, None).await;
    let cookie = cookie.unwrap();
    let mut p = basic_params();
    p["text"] = "".into();
    let (status, v) = generate(&mut app, &cookie, p).await;
    assert_eq!(status, StatusCode::OK, "空文本应生成空白练习纸: {v:?}");
    let url = v["url"].as_str().unwrap();
    let resp = get_pdf(&mut app, url, None, &[]).await;
    let bytes: Bytes = axum::body::to_bytes(resp.into_body(), 64 * 1024 * 1024).await.unwrap();
    assert!(bytes.starts_with(b"%PDF"));
}
