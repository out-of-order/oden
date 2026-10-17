use chrono::Utc;
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, DbErr, EntityTrait,
    QueryFilter, sea_query::value::prelude::serde_json::json,
};
use std::collections::HashSet;
use uuid::Uuid;

use async_trait::async_trait;

use crate::{
    entities::{
        item::{self},
        link,
    },
    errors::{SyncLinkError, UpdateItemError},
};

#[async_trait]
pub trait ItemRepositoryTrait {
    async fn find_all(&self) -> Result<Vec<item::Model>, DbErr>;
    async fn create_item(&self) -> Result<item::Model, DbErr>;
}

#[async_trait]
pub trait LinkRepositoryTrait {
    async fn find_all(&self) -> Result<Vec<link::Model>, DbErr>;
}

#[async_trait]
pub trait ContentRepositoryTrait {
    async fn update_content(&self, id: Uuid, content: String) -> Result<(), UpdateItemError>;
}

#[async_trait]
pub trait TitleRepositoryTrait {
    async fn update_title(&self, id: Uuid, title: String) -> Result<(), UpdateItemError>;
}

#[async_trait]
pub trait SyncLinkRepositoryTrait {
    async fn sync_links(&self, from: Uuid, targets: Vec<Uuid>) -> Result<(), SyncLinkError>;
}

pub struct LinkRepository {
    db: DatabaseConnection,
}

impl LinkRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl LinkRepositoryTrait for LinkRepository {
    async fn find_all(&self) -> Result<Vec<link::Model>, DbErr> {
        link::Entity::find().all(&self.db).await
    }
}

#[async_trait]
impl SyncLinkRepositoryTrait for LinkRepository {
    async fn sync_links(&self, from: Uuid, targets: Vec<Uuid>) -> Result<(), SyncLinkError> {
        let existing: HashSet<Uuid> = link::Entity::find()
            .filter(link::Column::SourceItemId.eq(from))
            .all(&self.db)
            .await?
            .iter()
            .map(|link| link.target_item_id)
            .collect();
        let desired: HashSet<Uuid> = targets.into_iter().collect();
        let to_add = desired.difference(&existing).collect::<Vec<&Uuid>>();
        let to_remove = existing.difference(&desired).collect::<Vec<&Uuid>>();
        for target_id in to_add {
            let link_to_insert = link::ActiveModel {
                id: Set(Uuid::new_v4()),
                source_item_id: Set(from),
                target_item_id: Set(*target_id),
            };
            if let Err(e) = link_to_insert.insert(&self.db).await {
                return Err(SyncLinkError::Db(e));
            }
        }
        for target_id in to_remove {
            link::Entity::delete_many()
                .filter(link::Column::SourceItemId.eq(from))
                .filter(link::Column::TargetItemId.eq(*target_id))
                .exec(&self.db)
                .await?;
        }
        Ok(())
    }
}

pub struct ItemRepository {
    db: DatabaseConnection,
}

#[cfg(test)]
pub struct MockItemRepository {}

#[async_trait]
#[cfg(test)]
impl ItemRepositoryTrait for MockItemRepository {
    async fn find_all(&self) -> Result<Vec<item::Model>, DbErr> {
        Ok(vec![])
    }
    async fn create_item(&self) -> Result<item::Model, DbErr> {
        let now = Utc::now();
        Ok(item::Model {
            id: Uuid::from_u128(1),
            name: "Untitled".to_string(),
            content: "# Untitled".to_string(),
            kind: item::ItemKind::Note,
            tags: json!([]),
            language: None,
            created_at: now,
            modified_at: now,
        })
    }
}

impl ItemRepository {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ItemRepositoryTrait for ItemRepository {
    async fn find_all(&self) -> Result<Vec<item::Model>, DbErr> {
        item::Entity::find().all(&self.db).await
    }

    async fn create_item(&self) -> Result<item::Model, DbErr> {
        let now = Utc::now();
        let item_instance = item::ActiveModel {
            id: Set(Uuid::new_v4()),
            name: Set("Untitled".to_string()),
            content: Set("# Untitled".to_string()),
            kind: Set(item::ItemKind::Note),
            tags: Set(json!([])),
            language: Set(None),
            created_at: Set(now),
            modified_at: Set(now),
        };
        item_instance.insert(&self.db).await
    }
}

#[async_trait]
impl ContentRepositoryTrait for ItemRepository {
    async fn update_content(&self, id: Uuid, content: String) -> Result<(), UpdateItemError> {
        let item_maybe = item::Entity::find_by_id(id).one(&self.db).await?;
        if let Some(item) = item_maybe {
            let mut item: item::ActiveModel = item.into();
            item.content = Set(content);
            item.modified_at = Set(Utc::now());
            item.update(&self.db).await?;
        } else {
            return Err(UpdateItemError::NotFound);
        }
        Ok(())
    }
}

#[async_trait]
impl TitleRepositoryTrait for ItemRepository {
    async fn update_title(&self, id: Uuid, title: String) -> Result<(), UpdateItemError> {
        let item_maybe = item::Entity::find_by_id(id).one(&self.db).await?;
        if let Some(item) = item_maybe {
            let mut item: item::ActiveModel = item.into();
            item.name = Set(title);
            item.modified_at = Set(Utc::now());
            item.update(&self.db).await?;
        } else {
            return Err(UpdateItemError::NotFound);
        }
        Ok(())
    }
}
