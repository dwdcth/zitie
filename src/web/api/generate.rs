//! POST /api/generate:校验 → 布局(算页数)→ 参数落盘 → 写入生成历史 → 返回内容寻址 URL。
//! PDF 本体由 GET /api/pdf/{hash}.pdf 惰性渲染。

use axum::extract::State;
use axum::Json;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};
use serde::Serialize;

use crate::canon;
use crate::entities::generation;
use crate::error::{AppError, AppResult};
use crate::layout;
use crate::layout::graphemes::Token;
use crate::layout::model::GenerateParams;
use crate::web::middleware::CurrentUser;
use crate::web::SharedState;

/// 每用户保留的历史条数上限
const HISTORY_LIMIT: u64 = 50;
/// 历史预览文本长度(字素数)
const PREVIEW_CHARS: usize = 24;

#[derive(Serialize)]
pub struct GenerateResult {
    pub url: String,
    pub hash: String,
    pub pages: usize,
}

pub async fn generate(
    State(state): State<SharedState>,
    user: CurrentUser,
    Json(params): Json<GenerateParams>,
) -> AppResult<Json<GenerateResult>> {
    // 1. 参数校验
    layout::validate(&params).map_err(|e| AppError::invalid(e.to_string()))?;

    // 2. 字体存在性 + 缓字覆盖检查(正文;开启拼音时连同拼音文本一起查)
    let entry = state
        .fonts
        .get(&params.font_id)
        .ok_or_else(|| AppError::invalid("所选字体不存在或已失效,请重新选择"))?;
    {
        let shaper = layout::Shaper::new(&entry);
        let mut missing = shaper.missing_chars(&params.text, 10);
        if params.pinyin && !params.paper.is_ruled() && missing.len() < 10 {
            let pinyin_text = pinyin_text_of(&params.text);
            let more = shaper.missing_chars(&pinyin_text, 10 - missing.len());
            missing.extend(more);
        }
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

    // 4. 参数落盘(供 PDF 惰性渲染)+ 写入历史
    let hash = canon::hash_params(&params);
    let canon_json = canon::canonical_json(&params);
    state.cache.store_params(&hash, &canon_json);
    record_history(&state, &user.id, &hash, &canon_json, &params, ir.page_count()).await;

    Ok(Json(GenerateResult {
        url: format!("/api/pdf/{hash}.pdf"),
        hash,
        pages: ir.page_count(),
    }))
}

/// 拼音文本(开启拼音时的覆盖检查用)。
fn pinyin_text_of(text: &str) -> String {
    let mut out = String::new();
    for token in layout::graphemes::tokenize(text) {
        if let Token::Cluster(c) = token {
            if let Some(py) = layout::pinyin::pinyin_of(&c) {
                out.push_str(&py);
            }
        }
    }
    out
}

/// 历史落库:同 (user, hash) 只更新时间戳;超出上限裁剪最旧的。
async fn record_history(
    state: &SharedState,
    user_id: &str,
    hash: &str,
    params_json: &str,
    params: &GenerateParams,
    pages: usize,
) {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let preview = history_preview(params);

    let existing = generation::Entity::find()
        .filter(generation::Column::UserId.eq(user_id))
        .filter(generation::Column::Hash.eq(hash))
        .one(&state.db)
        .await;
    match existing {
        Ok(Some(row)) => {
            let mut m: generation::ActiveModel = row.into();
            m.created_at = Set(now);
            m.preview = Set(preview);
            m.pages = Set(pages as i32);
            if let Err(e) = m.update(&state.db).await {
                tracing::warn!(error = %e, "更新生成历史失败(忽略)");
            }
        }
        _ => {
            let row = generation::ActiveModel {
                id: Set(uuid::Uuid::new_v4().to_string()),
                user_id: Set(user_id.to_string()),
                hash: Set(hash.to_string()),
                params: Set(params_json.to_string()),
                preview: Set(preview),
                pages: Set(pages as i32),
                created_at: Set(now),
            };
            if let Err(e) = row.insert(&state.db).await {
                tracing::warn!(error = %e, "写入生成历史失败(忽略)");
            } else {
                trim_history(state, user_id).await;
            }
        }
    }
}

/// 列表预览:优先页面标题,否则取文本前若干字素。
fn history_preview(p: &GenerateParams) -> String {
    let from_title = p
        .header
        .title
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty());
    if let Some(t) = from_title {
        return t.to_string();
    }
    use unicode_segmentation::UnicodeSegmentation;
    p.text
        .graphemes(true)
        .take(PREVIEW_CHARS)
        .collect::<String>()
}

async fn trim_history(state: &SharedState, user_id: &str) {
    use sea_orm::{QueryOrder, QuerySelect};
    let outdated: Vec<String> = generation::Entity::find()
        .filter(generation::Column::UserId.eq(user_id))
        .order_by_desc(generation::Column::CreatedAt)
        .offset(HISTORY_LIMIT)
        .all(&state.db)
        .await
        .map(|rows| rows.into_iter().map(|r| r.id).collect())
        .unwrap_or_default();
    for id in outdated {
        let _ = generation::Entity::delete_by_id(id).exec(&state.db).await;
    }
}
