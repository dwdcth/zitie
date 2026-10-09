use sea_orm::entity::prelude::*;

/// 生成历史:某用户生成过的字帖快照(按 hash 去重,最近优先)。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "generations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub user_id: String,
    /// 生成参数的内容寻址哈希(与 user_id 组合唯一:同参数复用同一条历史)
    pub hash: String,
    /// 完整参数快照(canonical JSON),用于一键恢复
    #[sea_orm(column_type = "Text")]
    pub params: String,
    /// 列表预览:页首标题或文本前若干字
    pub preview: String,
    pub pages: i32,
    #[sea_orm(column_type = "BigInteger")]
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::UserId",
        to = "super::user::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    User,
}

impl Related<super::user::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::User.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
