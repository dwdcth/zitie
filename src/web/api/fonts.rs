//! 字体 API:列表 / 上传 / 删除 / 字体文件。

use axum::extract::{Multipart, Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set};

use crate::entities::font;
use crate::error::{AppError, AppResult};
use crate::fonts::{build_entry, FontStore, FontSummary};
use crate::web::middleware::CurrentUser;
use crate::web::SharedState;
use sha2::Digest;

const MAX_FONT_BYTES: usize = 20 * 1024 * 1024;

#[derive(serde::Serialize)]
pub struct FontList {
    pub builtin: Vec<FontSummary>,
    pub mine: Vec<FontSummary>,
}

pub async fn list(State(state): State<SharedState>, user: CurrentUser) -> Json<FontList> {
    let mut builtin = Vec::new();
    let mut mine = Vec::new();
    for e in state.fonts.entries() {
        let s = FontSummary {
            id: e.id.clone(),
            family: e.family.clone(),
            subfamily: e.subfamily.clone(),
            builtin: e.owner_id.is_none(),
        };
        match &e.owner_id {
            None => builtin.push(s),
            Some(owner) if owner == &user.id => mine.push(s),
            Some(_) => {}
        }
    }
    mine.sort_by(|a, b| a.family.cmp(&b.family));
    Json(FontList { builtin, mine })
}

/// 上传字体:multipart 字段 `font`,魔数 + 大小 + 双解析校验,sha 去重。
pub async fn upload(
    State(state): State<SharedState>,
    user: CurrentUser,
    mut multipart: Multipart,
) -> AppResult<(StatusCode, Json<FontSummary>)> {
    let mut font_bytes: Option<Vec<u8>> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::invalid(format!("上传请求格式错误:{e}")))?
    {
        if field.name() != Some("font") {
            continue;
        }
        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::TooLarge(format!("{e}")))?;
        if data.len() > MAX_FONT_BYTES {
            return Err(AppError::TooLarge("字体文件不能超过 20MB".into()));
        }
        font_bytes = Some(data.to_vec());
        break;
    }
    let bytes = font_bytes.ok_or_else(|| AppError::invalid("缺少 font 字段"))?;

    let (info, family, subfamily, _idx) =
        FontStore::validate_and_build(bytes.clone()).map_err(|e| AppError::invalid(e.to_string()))?;
    let sha = hex::encode(sha2::Sha256::digest(&bytes));

    // 去重:同用户同文件直接返回已有记录
    let existing = font::Entity::find()
        .filter(font::Column::OwnerId.eq(&user.id))
        .filter(font::Column::FileSha256.eq(&sha))
        .one(&state.db)
        .await
        .map_err(anyhow::Error::new)?;
    if let Some(row) = existing {
        let summary = state
            .fonts
            .get(&row.id)
            .map(|e| FontSummary {
                id: e.id.clone(),
                family: e.family.clone(),
                subfamily: e.subfamily.clone(),
                builtin: false,
            });
        if let Some(s) = summary {
            return Ok((StatusCode::OK, Json(s)));
        }
    }

    // 落盘(文件名 = sha 前 16 位,内容寻址)
    let file_name = format!("{}.ttf", &sha[..16]);
    let file_path = state.cfg.fonts_dir().join(&file_name);
    std::fs::create_dir_all(state.cfg.fonts_dir()).map_err(anyhow::Error::new)?;
    std::fs::write(&file_path, &bytes).map_err(anyhow::Error::new)?;

    let id = uuid::Uuid::new_v4().to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let row = font::ActiveModel {
        id: Set(id.clone()),
        owner_id: Set(Some(user.id.clone())),
        family: Set(family.clone()),
        subfamily: Set(subfamily.clone()),
        file_sha256: Set(sha.clone()),
        file_path: Set(file_name),
        size_bytes: Set(bytes.len() as i64),
        units_per_em: Set(info.units_per_em as i32),
        ascender: Set(info.ascender),
        descender: Set(info.descender),
        x_height: Set(info.x_height),
        cap_height: Set(info.cap_height),
        created_at: Set(now),
    };
    row.insert(&state.db).await.map_err(anyhow::Error::new)?;

    let entry = build_entry(
        id.clone(),
        Some(user.id.clone()),
        family.clone(),
        subfamily.clone(),
        sha.clone(),
        file_path,
        bytes,
    )
    .map_err(|e| AppError::invalid(e.to_string()))?;
    state.fonts.insert(entry);

    Ok((
        StatusCode::CREATED,
        Json(FontSummary { id, family, subfamily, builtin: false }),
    ))
}

/// 删除字体:仅限本人上传;内置字体 403。
pub async fn remove(
    State(state): State<SharedState>,
    user: CurrentUser,
    Path(id): Path<String>,
) -> AppResult<StatusCode> {
    let Some(entry) = state.fonts.get(&id) else {
        return Err(AppError::NotFound("字体不存在".into()));
    };
    match &entry.owner_id {
        None => return Err(AppError::Forbidden("内置字体不可删除".into())),
        Some(owner) if owner != &user.id => {
            return Err(AppError::Forbidden("只能删除自己上传的字体".into()))
        }
        Some(_) => {}
    }
    font::Entity::delete_by_id(id.clone())
        .exec(&state.db)
        .await
        .map_err(anyhow::Error::new)?;
    state.fonts.remove(&id);
    let _ = std::fs::remove_file(&entry.file_path);
    Ok(StatusCode::NO_CONTENT)
}

/// 字体文件(前端 @font-face 预览用)。内容寻址,可永久缓存。
pub async fn font_file(State(state): State<SharedState>, Path(id): Path<String>) -> Response {
    let Some(entry) = state.fonts.get(&id) else {
        return AppError::NotFound("字体不存在".into()).into_response();
    };
    match std::fs::read(&entry.file_path) {
        Ok(bytes) => {
            let mut resp = bytes.into_response();
            resp.headers_mut().insert(
                header::CONTENT_TYPE,
                HeaderValue::from_static("font/ttf"),
            );
            resp.headers_mut().insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            );
            resp.headers_mut().insert(
                header::ETAG,
                HeaderValue::from_str(&format!("\"{}\"", entry.sha256)).unwrap(),
            );
            resp
        }
        Err(_) => AppError::NotFound("字体文件缺失".into()).into_response(),
    }
}
