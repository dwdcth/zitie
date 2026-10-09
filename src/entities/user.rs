use sea_orm::entity::prelude::*;

/// 匿名用户:首次访问时由 cookie 中间件创建,id 为 UUID v4。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    /// 创建时间(unix 毫秒)
    #[sea_orm(column_type = "BigInteger")]
    pub created_at: i64,
    #[sea_orm(column_type = "BigInteger")]
    pub last_seen_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_one = "super::preference::Entity")]
    Preference,
    #[sea_orm(has_many = "super::font::Entity")]
    Font,
}

impl Related<super::preference::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Preference.def()
    }
}

impl Related<super::font::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Font.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
