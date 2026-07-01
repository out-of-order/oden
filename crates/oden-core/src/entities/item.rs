use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, PartialEq, Debug, DeriveEntityModel)]
#[sea_orm(table_name = "items")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: Uuid,
    pub name: String,
    pub content: String,
    pub kind: ItemKind,
    pub tags: Json,
    #[sea_orm(nullable)]
    pub language: Option<String>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "i32", db_type = "Integer")]
pub enum ItemKind {
    #[sea_orm(num_value = 0)]
    Note,
    #[sea_orm(num_value = 1)]
    Snippet,
    #[sea_orm(num_value = 2)]
    Command,
}

#[derive(Copy, Clone, Debug, EnumIter)]
pub enum Relation {}

impl RelationTrait for Relation {
    fn def(&self) -> RelationDef {
        panic!("No relations for item::Relation")
    }
}

impl ActiveModelBehavior for ActiveModel {}
