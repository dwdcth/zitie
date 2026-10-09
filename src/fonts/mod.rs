//! 字体注册表:内置字体(fonts.json)与用户上传字体,启动时全量载入内存。
//! PDF 渲染用的 krilla::Font 与度量随 FontEntry 缓存,请求级的整形 Face 按需构建。

pub mod metrics;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use metrics::{FontError, FontFaceInfo};
use sha2::Digest;
use std::sync::RwLock;

/// 内置字体清单 fonts.json 的条目。
#[derive(Debug, serde::Deserialize)]
pub struct BuiltinFontDef {
    pub id: String,
    pub file: String,
    pub family: String,
    pub subfamily: String,
    pub sha256: String,
}

#[derive(Debug, serde::Deserialize)]
pub struct BuiltinFontsManifest {
    pub fonts: Vec<BuiltinFontDef>,
}

/// 一个已加载的字体。
#[derive(Debug)]
pub struct FontEntry {
    pub id: String,
    /// None = 内置字体
    pub owner_id: Option<String>,
    pub family: String,
    pub subfamily: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub file_path: PathBuf,
    pub face_index: u32,
    pub info: FontFaceInfo,
    pub bytes: Arc<[u8]>,
    pub krilla_font: krilla::text::Font,
}

/// 列表接口的摘要。
#[derive(Debug, serde::Serialize)]
pub struct FontSummary {
    pub id: String,
    pub family: String,
    pub subfamily: String,
    pub builtin: bool,
}

pub struct FontStore {
    entries: RwLock<HashMap<String, Arc<FontEntry>>>,
}

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("{0}")]
    FontsDir(String),
    #[error(transparent)]
    Font(#[from] FontError),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl FontStore {
    pub fn empty() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    /// 从 assets/fonts/fonts.json 加载内置字体。
    pub fn load_builtin(dir: &Path) -> Result<Self, StoreError> {
        let manifest_path = dir.join("fonts.json");
        let manifest: BuiltinFontsManifest =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path).map_err(|e| {
                StoreError::FontsDir(format!(
                    "无法读取 {}: {e}\n内置字体清单缺失,请检查部署目录",
                    manifest_path.display()
                ))
            })?)
            .map_err(|e| StoreError::FontsDir(format!("fonts.json 解析失败: {e}")))?;

        let store = Self::empty();
        for def in &manifest.fonts {
            let path = dir.join(&def.file);
            let bytes = match std::fs::read(&path) {
                Ok(b) => b,
                Err(_) => {
                    return Err(StoreError::FontsDir(format!(
                        "内置字体 {} 缺失,请先在项目根目录运行 ./scripts/fetch-fonts.sh",
                        path.display()
                    )))
                }
            };
            let sha = hex::encode(sha2::Sha256::digest(&bytes));
            if sha != def.sha256 {
                return Err(StoreError::FontsDir(format!(
                    "内置字体 {} 校验失败(sha256 不匹配),请运行 ./scripts/fetch-fonts.sh --force 重新下载",
                    path.display()
                )));
            }
            let entry = build_entry(
                def.id.clone(),
                None,
                def.family.clone(),
                def.subfamily.clone(),
                sha,
                path,
                bytes,
            )?;
            store.insert(entry);
        }
        Ok(store)
    }

    /// 从数据库加载用户上传字体(文件应位于 data/fonts)。
    pub async fn load_uploads(
        &self,
        db: &sea_orm::DatabaseConnection,
        fonts_dir: &Path,
    ) -> Result<(), StoreError> {
        use sea_orm::EntityTrait;
        let rows = crate::entities::font::Entity::find()
            .all(db)
            .await
            .map_err(|e| StoreError::Other(anyhow::anyhow!(e)))?;
        for row in rows {
            let path = fonts_dir.join(&row.file_path);
            match std::fs::read(&path) {
                Ok(bytes) => {
                    // 已上传字体解析失败不阻断启动,跳过并告警
                    match build_entry(
                        row.id.clone(),
                        row.owner_id.clone(),
                        row.family.clone(),
                        row.subfamily.clone(),
                        row.file_sha256.clone(),
                        path,
                        bytes,
                    ) {
                        Ok(entry) => self.insert(entry),
                        Err(e) => {
                            tracing::warn!(font = %row.id, error = %e, "跳过无法解析的上传字体")
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(font = %row.id, error = %e, "上传字体文件缺失,已跳过")
                }
            }
        }
        Ok(())
    }

    pub fn insert(&self, entry: FontEntry) {
        self.entries
            .write()
            .unwrap()
            .insert(entry.id.clone(), Arc::new(entry));
    }

    pub fn remove(&self, id: &str) {
        self.entries.write().unwrap().remove(id);
    }

    pub fn get(&self, id: &str) -> Option<Arc<FontEntry>> {
        self.entries.read().unwrap().get(id).cloned()
    }

    pub fn list(&self) -> Vec<FontSummary> {
        let entries = self.entries();
        let mut out: Vec<FontSummary> = entries
            .iter()
            .map(|e| FontSummary {
                id: e.id.clone(),
                family: e.family.clone(),
                subfamily: e.subfamily.clone(),
                builtin: e.owner_id.is_none(),
            })
            .collect();
        out.sort_by(|a, b| a.family.cmp(&b.family));
        out
    }

    /// 全部字目的快照(供 API 层按 owner 过滤)。
    pub fn entries(&self) -> Vec<Arc<FontEntry>> {
        self.entries.read().unwrap().values().cloned().collect()
    }

    /// 校验并构建上传字体(不落盘不入库,由调用方完成持久化)。
    pub fn validate_and_build(
        bytes: Vec<u8>,
    ) -> Result<(FontFaceInfo, String, String, u32), FontError> {
        let font = metrics::parse_font(&bytes, 0)?;
        let info = metrics::face_info(&font)?;
        let family = metrics::read_name(&font, metrics::FontName::Family)
            .unwrap_or_else(|| "未命名字体".into());
        let subfamily = metrics::read_name(&font, metrics::FontName::Subfamily)
            .unwrap_or_else(|| "Regular".into());
        // 确保 krilla 一定能用它渲染
        if krilla::text::Font::new(bytes.to_vec().into(), 0).is_none() {
            return Err(FontError::Parse("krilla 无法加载该字体".into()));
        }
        Ok((info, family, subfamily, 0))
    }
}

/// 构建完整 FontEntry(解析 + krilla Font)。
pub fn build_entry(
    id: String,
    owner_id: Option<String>,
    family: String,
    subfamily: String,
    sha256: String,
    file_path: PathBuf,
    bytes: Vec<u8>,
) -> Result<FontEntry, FontError> {
    let size_bytes = bytes.len() as u64;
    let font = metrics::parse_font(&bytes, 0)?;
    let info = metrics::face_info(&font)?;
    let krilla_font = krilla::text::Font::new(bytes.to_vec().into(), 0)
        .ok_or_else(|| FontError::Parse("krilla 无法加载该字体".into()))?;
    Ok(FontEntry {
        id,
        owner_id,
        family,
        subfamily,
        sha256,
        size_bytes,
        file_path,
        face_index: 0,
        info,
        bytes: Arc::from(bytes),
        krilla_font,
    })
}
