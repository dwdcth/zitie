use sea_orm::entity::prelude::*;

/// 字体:owner_id 为 NULL 表示内置字体,否则绑定到匿名用户。
/// file_path 相对数据目录,文件名为 sha256 前 16 位(天然内容寻址)。
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "fonts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub owner_id: Option<String>,
    pub family: String,
    pub subfamily: String,
    pub file_sha256: String,
    pub file_path: String,
    pub size_bytes: i64,
    pub units_per_em: i32,
    pub ascender: f64,
    pub descender: f64,
    pub x_height: Option<f64>,
    pub cap_height: Option<f64>,
    #[sea_orm(column_type = "BigInteger")]
    pub created_at: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::user::Entity",
        from = "Column::OwnerId",
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
