//! GET /api/pdf/{hash}.pdf:内容寻址的 PDF 端点。
//! 磁盘命中直接返回;未命中读 params 惰性渲染(spawn_blocking + 同哈希并发去重)。
//! immutable + ETag:浏览器一次拉取后永不再请求同参数的字帖。

use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::canon;
use crate::error::AppError;
use crate::layout;
use crate::layout::model::GenerateParams;
use crate::render;
use crate::web::SharedState;

pub async fn get_pdf(
    State(state): State<SharedState>,
    Path(filename): Path<String>,
    headers: axum::http::HeaderMap,
    axum::extract::Query(query): axum::extract::Query<PdfQuery>,
) -> Response {
    let Some(hash) = filename.strip_suffix(".pdf") else {
        return AppError::NotFound("PDF 不存在".into()).into_response();
    };
    if !canon::is_valid_hash(hash) {
        return AppError::NotFound("PDF 不存在".into()).into_response();
    }

    // 协商缓存
    let etag = format!("\"{hash}\"");
    if let Some(if_none_match) = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_owned())
    {
        if if_none_match
            .split(',')
            .map(str::trim)
            .any(|t| t == etag || t == "*")
        {
            return (
                StatusCode::NOT_MODIFIED,
                [(header::ETAG, HeaderValue::from_str(&etag).unwrap())],
            )
                .into_response();
        }
    }

    // 磁盘命中
    if let Some(bytes) = state.cache.get_pdf(hash) {
        return pdf_response(bytes, &etag, query.dl());
    }

    // 惰性渲染
    let Some(params_json) = state.cache.load_params(hash) else {
        return AppError::NotFound("生成内容不存在或已过期,请重新生成".into()).into_response();
    };
    let params: GenerateParams = match serde_json::from_str(&params_json) {
        Ok(p) => p,
        Err(_) => return AppError::NotFound("生成内容已失效,请重新生成".into()).into_response(),
    };

    // 同哈希去重:并发的多个请求只渲染一次(锁在阻塞线程内持有)
    let lock = state.cache.render_lock(hash);
    {
        // 双检:可能已被其他请求写入
        if let Some(bytes) = state.cache.get_pdf(hash) {
            return pdf_response(bytes, &etag, query.dl());
        }

        let st = state.clone();
        let hash_owned = hash.to_string();
        let render = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, AppError> {
            let _guard = lock.lock().unwrap_or_else(|p| p.into_inner());
            // 锁内再查一次磁盘
            if let Some(bytes) = st.cache.get_pdf(&hash_owned) {
                return Ok(bytes);
            }
            let entry = st
                .fonts
                .get(&params.font_id)
                .ok_or_else(|| AppError::NotFound("字体已失效,请重新生成字帖".into()))?;
            let ir = layout::layout_document(&params, &entry)
                .map_err(|e| AppError::invalid(e.to_string()))?;
            render::render_pdf(&ir, &st.fonts).map_err(|e| AppError::Internal(anyhow::anyhow!(e)))
        })
        .await;

        match render {
            Ok(Ok(bytes)) => {
                state.cache.store_pdf(hash, &bytes);
                let dl = query.dl();
                pdf_response(bytes, &etag, dl)
            }
            Ok(Err(e)) => e.into_response(),
            Err(join) => AppError::Internal(anyhow::anyhow!("渲染任务失败:{join}")).into_response(),
        }
    }
}

#[derive(serde::Deserialize)]
pub struct PdfQuery {
    dl: Option<String>,
}

impl PdfQuery {
    fn dl(&self) -> bool {
        matches!(self.dl.as_deref(), Some("1") | Some("true"))
    }
}

fn pdf_response(bytes: Vec<u8>, etag: &str, download: bool) -> Response {
    let disposition = if download {
        format!(
            "attachment; filename=\"zitie-{etag_trim}.pdf\"",
            etag_trim = etag.trim_matches('"')
        )
    } else {
        "inline; filename=\"zitie.pdf\"".to_string()
    };
    let mut resp = bytes.into_response();
    let h = resp.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/pdf"),
    );
    h.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=31536000, immutable"),
    );
    h.insert(header::ETAG, HeaderValue::from_str(etag).unwrap());
    h.insert(
        header::CONTENT_DISPOSITION,
        HeaderValue::from_str(&disposition).unwrap(),
    );
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    resp
}
