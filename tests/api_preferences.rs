mod common;

use axum::http::StatusCode;
use common::{req, test_app};

#[tokio::test]
async fn first_visit_creates_user_with_cookie() {
    let mut app = test_app().await;
    let (status, body, cookie) = req(&mut app, "GET", "/api/preferences", None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(cookie.as_deref().unwrap_or("").starts_with("zitie_uid="), "应下发 zitie_uid cookie: {cookie:?}");
    // 新用户返回默认偏好
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["paper"], "mizi");
    assert_eq!(v["trace_count"], 3);
}

#[tokio::test]
async fn preferences_roundtrip() {
    let mut app = test_app().await;
    let (_, _, cookie) = req(&mut app, "GET", "/api/preferences", None, None).await;
    let cookie = cookie.unwrap();

    let prefs = serde_json::json!({
        "text": "永和",
        "paper": "sixian",
        "mode": "chaoxie",
        "trace_count": 5,
        "copy_count": 2,
        "font_id": "shortstack"
    });
    let (status, _, _) = req(&mut app, "PUT", "/api/preferences", Some(prefs), Some(&cookie)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, body, _) = req(&mut app, "GET", "/api/preferences", None, Some(&cookie)).await;
    assert_eq!(status, StatusCode::OK);
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["paper"], "sixian");
    assert_eq!(v["mode"], "chaoxie");
    assert_eq!(v["trace_count"], 5);
    assert_eq!(v["font_id"], "shortstack");
    assert_eq!(v["text"], "永和");
}

#[tokio::test]
async fn different_cookies_are_different_users() {
    let mut app = test_app().await;
    let (_, _, c1) = req(&mut app, "GET", "/api/preferences", None, None).await;
    let (_, _, c2) = req(&mut app, "GET", "/api/preferences", None, None).await;
    assert_ne!(c1, c2, "两个匿名会话应有不同身份");

    // 用户 1 写偏好,用户 2 不受影响
    let c1 = c1.unwrap();
    let c2 = c2.unwrap();
    req(&mut app, "PUT", "/api/preferences", Some(serde_json::json!({"text": "甲"})), Some(&c1)).await;
    let (_, body, _) = req(&mut app, "GET", "/api/preferences", None, Some(&c2)).await;
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["text"], "", "用户 2 不应看到用户 1 的偏好");
}

#[tokio::test]
async fn invalid_cookie_regenerates_user() {
    let mut app = test_app().await;
    let fake = "zitie_uid=0badcafe-0000-0000-0000-000000000000";
    let (status, _, cookie) = req(&mut app, "GET", "/api/preferences", None, Some(fake)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(cookie.is_some(), "无效 cookie 应重建身份");
}
