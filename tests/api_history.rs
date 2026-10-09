mod common;

use axum::http::StatusCode;
use common::{req, test_app};

fn params_for(text: &str) -> serde_json::Value {
    serde_json::json!({
        "text": text,
        "paper": "mizi",
        "mode": "miaohong",
        "font_id": "lxgw-wenkai"
    })
}

#[tokio::test]
async fn history_records_dedupe_and_restore() {
    let mut app = test_app().await;
    let (_, _, cookie) = req(&mut app, "GET", "/api/preferences", None, None).await;
    let cookie = cookie.unwrap();

    // 两次不同参数 + 一次重复参数
    req(
        &mut app,
        "POST",
        "/api/generate",
        Some(params_for("永和")),
        Some(&cookie),
    )
    .await;
    req(
        &mut app,
        "POST",
        "/api/generate",
        Some(params_for("山高")),
        Some(&cookie),
    )
    .await;
    req(
        &mut app,
        "POST",
        "/api/generate",
        Some(params_for("永和")),
        Some(&cookie),
    )
    .await;

    let (status, body, _) = req(&mut app, "GET", "/api/history", None, Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let items = v.as_array().unwrap();
    assert_eq!(items.len(), 2, "同参数去重: {v:?}");
    // 最近一次(重复生成的 永和)应排最前
    assert!(
        items[0]["preview"].as_str().unwrap().contains("永和"),
        "最近生成排最前: {v:?}"
    );
    assert_eq!(items[0]["pages"], 1);
    assert!(items[0]["url"].as_str().unwrap().ends_with(".pdf"));

    // 恢复:取回完整参数
    let id = items[0]["id"].as_str().unwrap().to_string();
    let (status, body, _) = req(
        &mut app,
        "GET",
        &format!("/api/history/{id}"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let params: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(params["text"], "永和");
    assert_eq!(params["paper"], "mizi");

    // 删除后再取 → 404;列表变 1
    let (status, _, _) = req(
        &mut app,
        "DELETE",
        &format!("/api/history/{id}"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (_, body, _) = req(&mut app, "GET", "/api/history", None, Some(&cookie)).await;
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v.as_array().unwrap().len(), 1);
    let (status, _, _) = req(
        &mut app,
        "GET",
        &format!("/api/history/{id}"),
        None,
        Some(&cookie),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn history_is_per_user() {
    let mut app = test_app().await;
    let (_, _, alice) = req(&mut app, "GET", "/api/preferences", None, None).await;
    let (_, _, bob) = req(&mut app, "GET", "/api/preferences", None, None).await;
    let alice = alice.unwrap();
    let bob = bob.unwrap();

    req(
        &mut app,
        "POST",
        "/api/generate",
        Some(params_for("甲乙")),
        Some(&alice),
    )
    .await;
    let (_, body, _) = req(&mut app, "GET", "/api/history", None, Some(&bob)).await;
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.as_array().unwrap().is_empty(), "bob 看不到 alice 的历史");

    // bob 删 alice 的历史 → 403
    let (_, body, _) = req(&mut app, "GET", "/api/history", None, Some(&alice)).await;
    let id: String = serde_json::from_slice::<serde_json::Value>(&body).unwrap()[0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let (status, _, _) = req(
        &mut app,
        "DELETE",
        &format!("/api/history/{id}"),
        None,
        Some(&bob),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
