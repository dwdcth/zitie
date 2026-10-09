use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

/// 统一错误类型:所有 handler 返回它,序列化为
/// `{"error":{"code":"...","message":"..."}}`,message 为面向用户的中文文案。
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// 请求参数不合法(400)
    #[error("{0}")]
    Invalid(String),
    /// 请求体过大(413)
    #[error("上传内容过大:{0}")]
    TooLarge(String),
    /// 资源不存在(404)
    #[error("{0}")]
    NotFound(String),
    /// 无权操作他人或内置资源(403)
    #[error("{0}")]
    Forbidden(String),
    /// 服务端内部错误(500),日志记原文,对外返回通用文案
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

impl AppError {
    pub fn invalid(msg: impl Into<String>) -> Self {
        AppError::Invalid(msg.into())
    }

    fn parts(&self) -> (&'static str, StatusCode, String) {
        match self {
            AppError::Invalid(m) => ("invalid_params", StatusCode::BAD_REQUEST, m.clone()),
            AppError::TooLarge(m) => ("too_large", StatusCode::PAYLOAD_TOO_LARGE, m.clone()),
            AppError::NotFound(m) => ("not_found", StatusCode::NOT_FOUND, m.clone()),
            AppError::Forbidden(m) => ("forbidden", StatusCode::FORBIDDEN, m.clone()),
            AppError::Internal(e) => {
                tracing::error!(error = ?e, "internal error");
                (
                    "internal",
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "服务器内部错误,请稍后重试".into(),
                )
            }
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (code, status, message) = self.parts();
        (
            status,
            Json(json!({ "error": { "code": code, "message": message } })),
        )
            .into_response()
    }
}

pub type AppResult<T> = Result<T, AppError>;
