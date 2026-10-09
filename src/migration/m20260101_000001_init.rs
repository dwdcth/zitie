use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Users::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Users::CreatedAt).big_integer().not_null())
                    .col(ColumnDef::new(Users::LastSeenAt).big_integer().not_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Preferences::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Preferences::UserId).string().not_null().primary_key())
                    .col(ColumnDef::new(Preferences::Data).text().not_null())
                    .col(ColumnDef::new(Preferences::UpdatedAt).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_preferences_user")
                            .from(Preferences::Table, Preferences::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Fonts::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Fonts::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Fonts::OwnerId).string().null())
                    .col(ColumnDef::new(Fonts::Family).string().not_null())
                    .col(ColumnDef::new(Fonts::Subfamily).string().not_null())
                    .col(ColumnDef::new(Fonts::FileSha256).string().not_null())
                    .col(ColumnDef::new(Fonts::FilePath).string().not_null())
                    .col(ColumnDef::new(Fonts::SizeBytes).big_integer().not_null())
                    .col(ColumnDef::new(Fonts::UnitsPerEm).integer().not_null())
                    .col(ColumnDef::new(Fonts::Ascender).double().not_null())
                    .col(ColumnDef::new(Fonts::Descender).double().not_null())
                    .col(ColumnDef::new(Fonts::XHeight).double().null())
                    .col(ColumnDef::new(Fonts::CapHeight).double().null())
                    .col(ColumnDef::new(Fonts::CreatedAt).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_fonts_owner")
                            .from(Fonts::Table, Fonts::OwnerId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        // 同一用户(含内置的 NULL 组)同一文件只存一份
        manager
            .create_index(
                Index::create()
                    .name("idx_fonts_dedupe")
                    .table(Fonts::Table)
                    .col(Fonts::OwnerId)
                    .col(Fonts::FileSha256)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Fonts::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Preferences::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Users::Table).to_owned()).await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
    CreatedAt,
    LastSeenAt,
}

#[derive(DeriveIden)]
enum Preferences {
    Table,
    UserId,
    Data,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Fonts {
    Table,
    Id,
    OwnerId,
    Family,
    Subfamily,
    FileSha256,
    FilePath,
    SizeBytes,
    UnitsPerEm,
    Ascender,
    Descender,
    XHeight,
    CapHeight,
    CreatedAt,
}
