use sea_orm::{EnumIter, entity::prelude::*};

#[derive(Debug, Clone, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "links")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub source_item_id: Uuid,
    pub target_item_id: Uuid,
}

#[derive(Clone, Copy, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::item::Entity",
        from = "Column::SourceItemId",
        to = "super::item::Column::Id"
    )]
    SourceItem,

    #[sea_orm(
        belongs_to = "super::item::Entity",
        from = "Column::TargetItemId",
        to = "super::item::Column::Id"
    )]
    TargetItem,
}

impl ActiveModelBehavior for ActiveModel {}
