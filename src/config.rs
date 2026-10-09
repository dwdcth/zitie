use std::path::PathBuf;

/// 服务配置,全部来自环境变量,均有默认值。
#[derive(Debug, Clone)]
pub struct Config {
    pub addr: String,
    pub data_dir: PathBuf,
    pub assets_dir: PathBuf,
    /// 前端静态文件目录
    pub static_dir: PathBuf,
    /// PDF 磁盘缓存配额(MB)
    pub cache_quota_mb: u64,
    /// 是否在 cookie 上加 Secure(https 部署时开启)
    pub secure_cookie: bool,
}

impl Config {
    pub fn from_env() -> Self {
        let data_dir =
            PathBuf::from(std::env::var("ZITIE_DATA_DIR").unwrap_or_else(|_| "data".into()));
        let assets_dir =
            PathBuf::from(std::env::var("ZITIE_ASSETS_DIR").unwrap_or_else(|_| "assets".into()));
        let static_dir =
            PathBuf::from(std::env::var("ZITIE_STATIC_DIR").unwrap_or_else(|_| "static".into()));
        Self {
            addr: std::env::var("ZITIE_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
            cache_quota_mb: std::env::var("ZITIE_CACHE_QUOTA_MB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(500),
            secure_cookie: std::env::var("ZITIE_SECURE_COOKIE")
                .map(|v| v == "1" || v.eq_ignore_ascii_case("true"))
                .unwrap_or(false),
            data_dir,
            assets_dir,
            static_dir,
        }
    }

    pub fn db_path(&self) -> PathBuf {
        self.data_dir.join("zitie.db")
    }

    pub fn fonts_dir(&self) -> PathBuf {
        self.data_dir.join("fonts")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.data_dir.join("cache")
    }

    pub fn builtin_fonts_dir(&self) -> PathBuf {
        self.assets_dir.join("fonts")
    }
}
