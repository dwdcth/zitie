pub mod m20260101_000001_init;
pub mod m20260102_000002_history;

use sea_orm_migration::MigratorTrait;

/// 嵌入式迁移:启动时在 main 中执行 `Migrator::up(&db, None)`,
/// 单二进制部署无需外部 CLI。
pub struct Migrator;

impl MigratorTrait for Migrator {
    fn migrations() -> Vec<Box<dyn sea_orm_migration::MigrationTrait>> {
        vec![
            Box::new(m20260101_000001_init::Migration),
            Box::new(m20260102_000002_history::Migration),
        ]
    }
}
