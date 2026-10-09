//! 路由装配与应用状态。

pub mod api;
pub mod middleware;

use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderValue, StatusCode};
use axum::middleware::from_fn_with_state;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};

use crate::config::Config;
use crate::fonts::FontStore;
use crate::pdfcache::PdfCache;

#[derive(Clone)]
pub struct AppState {
    pub db: sea_orm::DatabaseConnection,
    pub fonts: Arc<FontStore>,
    pub cache: Arc<PdfCache>,
    pub cfg: Arc<Config>,
}

pub type SharedState = Arc<AppState>;

pub fn router(state: SharedState) -> Router {
    // 需要匿名身份的路由(页面 + 业务 API)
    let identified = Router::new()
        .route("/", get(index))
        .route(
            "/api/preferences",
            get(api::preferences::get).put(api::preferences::put),
        )
        .route("/api/fonts", get(api::fonts::list))
        .route(
            "/api/fonts",
            post(api::fonts::upload).route_layer(axum::extract::DefaultBodyLimit::max(
                20 * 1024 * 1024 + 64 * 1024,
            )),
        )
        .route("/api/fonts/{id}", delete(api::fonts::remove))
        .route("/api/generate", post(api::generate::generate))
        .route("/api/history", get(api::history::list))
        .route(
            "/api/history/{id}",
            get(api::history::get_params).delete(api::history::remove),
        )
        .layer(from_fn_with_state(state.clone(), middleware::anon_user));

    // 不需要身份的路由(PDF 内容寻址,公开只读;避免爬虫建用户)
    let public = Router::new()
        .route("/healthz", get(healthz))
        .route("/api/fonts/{id}/file", get(api::fonts::font_file))
        .route("/api/pdf/{filename}", get(api::pdf::get_pdf));

    identified
        .merge(public)
        .route("/static/{*path}", get(serve_static).post(serve_static))
        .layer(axum::extract::DefaultBodyLimit::max(256 * 1024))
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .with_state(state)
}

async fn index(State(state): State<SharedState>) -> Response {
    match tokio::fs::read_to_string(state.cfg.static_dir.join("index.html")).await {
        Ok(html) => (
            StatusCode::OK,
            [
                (
                    header::CONTENT_TYPE,
                    HeaderValue::from_static("text/html; charset=utf-8"),
                ),
                (header::CACHE_CONTROL, HeaderValue::from_static("no-cache")),
            ],
            html,
        )
            .into_response(),
        Err(_) => crate::error::AppError::NotFound("前端文件缺失".into()).into_response(),
    }
}

async fn serve_static(
    State(state): State<SharedState>,
    axum::extract::Path(path): axum::extract::Path<String>,
) -> Response {
    // 仅允许文件名,禁止穿越
    if path.contains("..") || path.contains('\\') {
        return crate::error::AppError::invalid("非法路径").into_response();
    }
    match tokio::fs::read(state.cfg.static_dir.join(&path)).await {
        Ok(bytes) => {
            let mime = match path.rsplit('.').next() {
                Some("js") => "text/javascript; charset=utf-8",
                Some("css") => "text/css; charset=utf-8",
                Some("svg") => "image/svg+xml",
                Some("png") => "image/png",
                Some("ico") => "image/x-icon",
                _ => "application/octet-stream",
            };
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, HeaderValue::from_static(mime)),
                    (
                        header::CACHE_CONTROL,
                        HeaderValue::from_static("public, max-age=86400, must-revalidate"),
                    ),
                ],
                bytes,
            )
                .into_response()
        }
        Err(_) => crate::error::AppError::NotFound("文件不存在".into()).into_response(),
    }
}

/// 健康检查
pub async fn healthz() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}
