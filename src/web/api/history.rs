//! 生成历史:最近生成的字帖列表与删除(点击条目可恢复参数,由前端完成)。

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder, QuerySelect};

use crate::entities::generation;
use crate::error::{AppError, AppResult};
use crate::web::middleware::CurrentUser;
use crate::web::SharedState;

#[derive(serde::Serialize)]
pub struct HistoryItem {
    pub id: String,
    pub hash: String,
    pub url: String,
    pub preview: String,
    pub pages: i32,
    pub created_at: i64,
}

pub async fn list(State(state): State<SharedState>, user: CurrentUser) -> AppResult<Json<Vec<HistoryItem>>> {
    let rows = generation::Entity::find()
        .filter(generation::Column::UserId.eq(&user.id))
        .order_by_desc(generation::Column::CreatedAt)
        .limit(50)
        .all(&state.db)
        .await
        .map_err(anyhow::Error::new)?;
    Ok(Json(
        rows.into_iter()
            .map(|r| HistoryItem {
                url: format!("/api/pdf/{}.pdf", r.hash),
                id: r.id,
                hash: r.hash,
                preview: r.preview,
                pages: r.pages,
                created_at: r.created_at,
            })
            .collect(),
    ))
}

/// 删除历史条目(仅本人;PDF 缓存不受影响)。
pub async fn remove(
    State(state): State<SharedState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let row = generation::Entity::find_by_id(&id)
        .one(&state.db)
        .await
        .map_err(anyhow::Error::new)?;
    let Some(row) = row else {
        return Ok(StatusCode::NO_CONTENT); // 幂等删除
    };
    if row.user_id != user.id {
        return Err(AppError::Forbidden("只能删除自己的历史记录".into()));
    }
    generation::Entity::delete_by_id(id)
        .exec(&state.db)
        .await
        .map_err(anyhow::Error::new)?;
    Ok(StatusCode::NO_CONTENT)
}

/// 取单条历史的完整参数快照(前端一键恢复用)。
pub async fn get_params(
    State(state): State<SharedState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> AppResult<Json<crate::layout::model::GenerateParams>> {
    let row = generation::Entity::find_by_id(&id)
        .one(&state.db)
        .await
        .map_err(anyhow::Error::new)?;
    let row = row.ok_or_else(|| AppError::NotFound("历史记录不存在".into()))?;
    if row.user_id != user.id {
        return Err(AppError::Forbidden("只能查看自己的历史记录".into()));
    }
    let params: crate::layout::model::GenerateParams = serde_json::from_str(&row.params)
        .map_err(|e| AppError::NotFound(format!("历史记录已损坏:{e}")))?;
    Ok(Json(params))
}
