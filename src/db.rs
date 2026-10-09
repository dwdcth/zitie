use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::MigratorTrait;
use std::path::Path;

use crate::migration::Migrator;

/// 建立 SQLite 连接并执行嵌入式迁移。
/// 注:sqlx-sqlite 默认启用 WAL、foreign_keys 与 busy_timeout,无需额外配置。
pub async fn init(db_path: &Path) -> anyhow::Result<DatabaseConnection> {
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let url = format!("sqlite://{}?mode=rwc", db_path.display());
    let db = Database::connect(&url).await?;
    Migrator::up(&db, None).await?;
    tracing::info!(db = %db_path.display(), "database ready");
    Ok(db)
}
