//! PDF 磁盘缓存:内容寻址(`{hash}.pdf` + `{hash}.params.json`),
//! 文件不可变,可随时删除;配额超限时按访问时间淘汰。

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

pub struct PdfCache {
    dir: PathBuf,
    quota_bytes: u64,
    /// 同哈希并发渲染去重锁
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

impl PdfCache {
    pub fn new(dir: PathBuf, quota_mb: u64) -> std::io::Result<Self> {
        std::fs::create_dir_all(&dir)?;
        Ok(Self {
            dir,
            quota_bytes: quota_mb.saturating_mul(1024 * 1024),
            locks: Mutex::new(HashMap::new()),
        })
    }

    pub fn pdf_path(&self, hash: &str) -> PathBuf {
        self.dir.join(format!("{hash}.pdf"))
    }

    pub fn params_path(&self, hash: &str) -> PathBuf {
        self.dir.join(format!("{hash}.params.json"))
    }

    /// 访问标记文件(零字节,用 mtime 记录最近访问,供淘汰排序)
    fn touch_path(&self, hash: &str) -> PathBuf {
        self.dir.join(format!("{hash}.atime"))
    }

    pub fn get_pdf(&self, hash: &str) -> Option<Vec<u8>> {
        let bytes = std::fs::read(self.pdf_path(hash)).ok()?;
        // 更新访问时间(打开即触碰 mtime,不写内容)
        if let Ok(f) = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.touch_path(hash))
        {
            let _ = f.set_modified(SystemTime::now());
        }
        Some(bytes)
    }

    pub fn load_params(&self, hash: &str) -> Option<String> {
        std::fs::read_to_string(self.params_path(hash)).ok()
    }

    /// 写入参数(生成时)。PDF 本体由首次 GET 时惰性渲染写入。
    pub fn store_params(&self, hash: &str, params_json: &str) {
        let path = self.params_path(hash);
        if path.exists() {
            return; // 内容寻址,同哈希内容相同
        }
        let tmp = self.dir.join(format!("{hash}.params.json.part"));
        if std::fs::write(&tmp, params_json).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
    }

    pub fn store_pdf(&self, hash: &str, pdf: &[u8]) {
        let path = self.pdf_path(hash);
        if path.exists() {
            return;
        }
        let tmp = self.dir.join(format!("{hash}.pdf.part"));
        if std::fs::write(&tmp, pdf).is_ok() {
            let _ = std::fs::rename(&tmp, &path);
        }
        let _ = std::fs::write(self.touch_path(hash), b"");
    }

    /// 取同哈希渲染锁:并发 GET 只有一个真正渲染,其余等它落盘。
    pub fn render_lock(&self, hash: &str) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock().unwrap();
        locks
            .entry(hash.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    }

    /// 超配额清理:按 atime 升序(最久未用先删)删除 pdf+params+atime 三件套。
    /// 在 spawn_blocking 中周期调用。
    pub fn cleanup_if_over_quota(&self) {
        let mut entries: Vec<(SystemTime, String)> = Vec::new();
        let mut total: u64 = 0;
        let Ok(rd) = std::fs::read_dir(&self.dir) else {
            return;
        };
        for entry in rd.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            let Some(hash) = name.strip_suffix(".pdf") else {
                continue;
            };
            let meta = match entry.metadata() {
                Ok(m) => m,
                Err(_) => continue,
            };
            total = total.saturating_add(meta.len());
            let atime = std::fs::metadata(self.touch_path(hash))
                .and_then(|m| m.modified())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            entries.push((atime, hash.to_string()));
        }
        if total <= self.quota_bytes {
            return;
        }
        entries.sort();
        for (_, hash) in entries {
            if total <= self.quota_bytes {
                break;
            }
            if let Ok(m) = std::fs::metadata(self.pdf_path(&hash)) {
                total = total.saturating_sub(m.len());
            }
            let _ = std::fs::remove_file(self.pdf_path(&hash));
            let _ = std::fs::remove_file(self.params_path(&hash));
            let _ = std::fs::remove_file(self.touch_path(&hash));
            tracing::debug!(hash = %hash, "缓存淘汰");
        }
    }
}
