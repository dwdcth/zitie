//! POST /api/generate:校验 → 布局(算页数)→ 参数落盘 → 返回内容寻址 URL。
//! PDF 本体由 GET /api/pdf/{hash}.pdf 惰性渲染。

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::canon;
use crate::error::{AppError, AppResult};
use crate::layout;
use crate::layout::model::GenerateParams;
use crate::web::middleware::CurrentUser;
use crate::web::SharedState;

#[derive(Serialize)]
pub struct GenerateResult {
    pub url: String,
    pub hash: String,
    pub pages: usize,
}

pub async fn generate(
    State(state): State<SharedState>,
    _user: CurrentUser,
    Json(params): Json<GenerateParams>,
) -> AppResult<Json<GenerateResult>> {
    // 1. 参数校验
    layout::validate(&params).map_err(|e| AppError::invalid(e.to_string()))?;

    // 2. 字体存在性 + 缓字覆盖检查
    let entry = state
        .fonts
        .get(&params.font_id)
        .ok_or_else(|| AppError::invalid("所选字体不存在或已失效,请重新选择"))?;
    {
        let shaper = layout::Shaper::new(&entry);
        let missing = shaper.missing_chars(&params.text, 10);
        if !missing.is_empty() {
            let list: Vec<String> = missing.iter().map(|c| c.to_string()).collect();
            return Err(AppError::invalid(format!(
                "所选字体不包含以下字符:{}{}",
                list.join(" "),
                if missing.len() >= 10 { " …" } else { "" }
            )));
        }
    }

    // 3. 完整布局:得到页数并复核页数上限
    let ir = layout::layout_document(&params, &entry)
        .map_err(|e| AppError::invalid(e.to_string()))?;

    // 4. 参数落盘(供 PDF 惰性渲染),返回内容寻址 URL
    let hash = canon::hash_params(&params);
    state.cache.store_params(&hash, &canon::canonical_json(&params));

    Ok(Json(GenerateResult {
        url: format!("/api/pdf/{hash}.pdf"),
        hash,
        pages: ir.page_count(),
    }))
}
