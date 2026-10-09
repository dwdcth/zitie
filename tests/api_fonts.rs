mod common;

use axum::body::Body;
use axum::http::StatusCode;
use common::{req, test_app, upload_font};
use tower::ServiceExt;

fn fixture_font() -> String {
    format!(
        "{}/assets/fonts/ShortStack-Regular.ttf",
        env!("CARGO_MANIFEST_DIR")
    )
}

#[tokio::test]
async fn upload_dedupe_and_delete() {
    let mut app = test_app().await;
    let (_, _, cookie) = req(&mut app, "GET", "/api/fonts", None, None).await;
    let cookie = cookie.unwrap();

    // 首次上传 → 201
    let (status, body) = upload_font(&mut app, &fixture_font(), &cookie).await;
    assert_eq!(status, StatusCode::CREATED, "首次上传应 201: {body:?}");
    let id: String = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // 重复上传 → 200 且同 id(sha 去重)
    let (status, body) = upload_font(&mut app, &fixture_font(), &cookie).await;
    assert_eq!(status, StatusCode::OK);
    let id2: String = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(id, id2);

    // 列表:mine 包含它
    let (_, body, _) = req(&mut app, "GET", "/api/fonts", None, Some(&cookie)).await;
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v["mine"].as_array().unwrap().iter().any(|f| f["id"] == *id));

    // 删除 → 204,再删 → 404
    let (status, _, _) = req(
        &mut app,
        "DELETE",
        &format!("/api/fonts/{id}"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _, _) = req(
        &mut app,
        "DELETE",
        &format!("/api/fonts/{id}"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn cannot_delete_builtin_or_others_font() {
    let mut app = test_app().await;
    let (_, _, alice) = req(&mut app, "GET", "/api/fonts", None, None).await;
    let (_, _, bob) = req(&mut app, "GET", "/api/fonts", None, None).await;
    let alice = alice.unwrap();
    let bob = bob.unwrap();

    // 内置不可删
    let (status, body, _) = req(
        &mut app,
        "DELETE",
        "/api/fonts/lxgw-wenkai",
        None,
        Some(&alice),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body:?}");

    // bob 上传,alice 不能删
    let (_, body) = upload_font(&mut app, &fixture_font(), &bob).await;
    let id: String = serde_json::from_slice::<serde_json::Value>(&body).unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (status, _, _) = req(
        &mut app,
        "DELETE",
        &format!("/api/fonts/{id}"),
        None,
        Some(&alice),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "不能删别人的字体");
    // bob 自己可以
    let (status, _, _) = req(
        &mut app,
        "DELETE",
        &format!("/api/fonts/{id}"),
        None,
        Some(&bob),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn upload_rejects_garbage() {
    let mut app = test_app().await;
    let (_, _, cookie) = req(&mut app, "GET", "/api/fonts", None, None).await;
    let cookie = cookie.unwrap();

    let tmp = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(tmp.path(), b"definitely not a font file").unwrap();
    let (status, body) = upload_font(&mut app, tmp.path().to_str().unwrap(), &cookie).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body:?}");
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(
        v["error"]["message"].as_str().unwrap().contains("字体"),
        "错误消息应面向用户"
    );
}

#[tokio::test]
async fn font_file_is_cacheable() {
    let app = test_app().await;
    let resp = app
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri("/api/fonts/lxgw-wenkai/file")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers().get("cache-control").unwrap(),
        "public, max-age=31536000, immutable"
    );
    assert!(resp.headers().get("etag").is_some());
    let _ = axum::body::to_bytes(resp.into_body(), 64 * 1024 * 1024)
        .await
        .unwrap();
}
