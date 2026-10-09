//! 偏好读写:JSON 整行存储,serde 默认值兜底旧数据。

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use crate::entities::preference;
use crate::error::AppResult;
use crate::layout::model::GenerateParams;
use crate::web::middleware::CurrentUser;
use crate::web::SharedState;

pub async fn get(
    State(state): State<SharedState>,
    user: CurrentUser,
) -> AppResult<Json<GenerateParams>> {
    let row = preference::Entity::find_by_id(&user.id)
        .one(&state.db)
        .await
        .map_err(anyhow::Error::new)?;
    let prefs = row
        .and_then(|r| serde_json::from_str::<GenerateParams>(&r.data).ok())
        .unwrap_or_default();
    Ok(Json(prefs))
}

pub async fn put(
    State(state): State<SharedState>,
    user: CurrentUser,
    Json(prefs): Json<GenerateParams>,
) -> AppResult<StatusCode> {
    // 偏好保存是宽松的:不做生成级校验,只限制体量(全局 body limit 已挡)
    let data = serde_json::to_string(&prefs).map_err(anyhow::Error::new)?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let row = preference::ActiveModel {
        user_id: Set(user.id.clone()),
        data: Set(data),
        updated_at: Set(now),
    };
    // upsert
    let existing = preference::Entity::find_by_id(&user.id)
        .one(&state.db)
        .await
        .map_err(anyhow::Error::new)?;
    if existing.is_some() {
        row.update(&state.db).await.map_err(anyhow::Error::new)?;
    } else {
        row.insert(&state.db).await.map_err(anyhow::Error::new)?;
    }
    Ok(StatusCode::NO_CONTENT)
}
