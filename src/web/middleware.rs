//! 匿名用户中间件:首次访问创建用户并下发 HttpOnly cookie,
//! 之后按 cookie 识别用户(节流更新 last_seen)。

use axum::extract::{FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{header, HeaderValue};
use axum::middleware::Next;
use axum::response::Response;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use sea_orm::{ActiveModelTrait, EntityTrait, Set};

use crate::entities::user;
use crate::error::AppError;
use crate::web::SharedState;

pub const COOKIE_NAME: &str = "zitie_uid";
const COOKIE_MAX_AGE_DAYS: i64 = 400;
/// last_seen 节流间隔(毫秒)
const LAST_SEEN_THROTTLE_MS: i64 = 3600 * 1000;

#[derive(Clone, Debug)]
pub struct CurrentUser {
    pub id: String,
}

impl FromRequestParts<SharedState> for CurrentUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &SharedState,
    ) -> Result<Self, Self::Rejection> {
        parts
            .extensions
            .get::<CurrentUser>()
            .cloned()
            .ok_or_else(|| AppError::Internal(anyhow::anyhow!("缺少用户身份(中间件未生效)")))
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn valid_uuid(s: &str) -> bool {
    uuid::Uuid::parse_str(s).is_ok()
}

pub async fn anon_user(
    State(state): State<SharedState>,
    req: Request,
    next: Next,
) -> Response {
    let jar = CookieJar::from_headers(req.headers());
    let now = now_ms();

    let cookie_id = jar.get(COOKIE_NAME).map(|c| c.value().to_string());
    let mut set_cookie: Option<String> = None;

    let uid = match cookie_id.filter(|v| valid_uuid(v)) {
        Some(id) => {
            // 已有身份;节流更新 last_seen
            match user::Entity::find_by_id(&id).one(&state.db).await {
                Ok(Some(row)) => {
                    if now - row.last_seen_at > LAST_SEEN_THROTTLE_MS {
                        let mut m: user::ActiveModel = row.into();
                        m.last_seen_at = Set(now);
                        if let Err(e) = m.update(&state.db).await {
                            tracing::warn!(error = %e, "更新 last_seen 失败(忽略)");
                        }
                    }
                    id
                }
                _ => {
                    // cookie 指向不存在的用户:重建
                    let (id, cookie) = create_user(&state, now).await;
                    set_cookie = Some(cookie);
                    id
                }
            }
        }
        None => {
            let (id, cookie) = create_user(&state, now).await;
            set_cookie = Some(cookie);
            id
        }
    };

    let mut req = req;
    req.extensions_mut().insert(CurrentUser { id: uid });
    let mut resp = next.run(req).await;
    if let Some(cookie) = set_cookie {
        if let Ok(v) = HeaderValue::from_str(&cookie) {
            resp.headers_mut().append(header::SET_COOKIE, v);
        }
    }
    resp
}

async fn create_user(state: &SharedState, now: i64) -> (String, String) {
    let id = uuid::Uuid::new_v4().to_string();
    let row = user::ActiveModel {
        id: Set(id.clone()),
        created_at: Set(now),
        last_seen_at: Set(now),
    };
    if let Err(e) = row.insert(&state.db).await {
        tracing::error!(error = %e, "创建匿名用户失败");
        // 中间件不能失败(否则整站不可用);身份缺失时后续 handler 会报错
        return (id, String::new());
    }
    let mut cookie = Cookie::build((COOKIE_NAME, id.clone()))
        .http_only(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(time::Duration::days(COOKIE_MAX_AGE_DAYS))
        .build()
        .to_string();
    if state.cfg.secure_cookie {
        cookie.push_str("; Secure");
    }
    (id, cookie)
}
