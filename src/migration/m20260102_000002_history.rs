use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Generations::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Generations::Id).string().not_null().primary_key())
                    .col(ColumnDef::new(Generations::UserId).string().not_null())
                    .col(ColumnDef::new(Generations::Hash).string().not_null())
                    .col(ColumnDef::new(Generations::Params).text().not_null())
                    .col(ColumnDef::new(Generations::Preview).string().not_null())
                    .col(ColumnDef::new(Generations::Pages).integer().not_null())
                    .col(ColumnDef::new(Generations::CreatedAt).big_integer().not_null())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_generations_user")
                            .from(Generations::Table, Generations::UserId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::Cascade)
                            .on_update(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_generations_user_recent")
                    .table(Generations::Table)
                    .col(Generations::UserId)
                    .col(Generations::CreatedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_generations_user_hash")
                    .table(Generations::Table)
                    .col(Generations::UserId)
                    .col(Generations::Hash)
                    .unique()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Generations::Table).to_owned()).await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Generations {
    Table,
    Id,
    UserId,
    Hash,
    Params,
    Preview,
    Pages,
    CreatedAt,
}
