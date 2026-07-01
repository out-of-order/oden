#![allow(dead_code)]

use chrono::{DateTime, Utc};
use gpui::SharedString;
use oden_core::entities::item::{self, ItemKind};
use serde_json::Value;

#[derive(Clone)]
pub struct Item {
    pub id: uuid::Uuid,
    pub name: SharedString,
    pub content: SharedString,
    pub kind: ItemKind,
    pub tags: Vec<SharedString>,
    pub language: Option<SharedString>,
    pub created_at: DateTime<Utc>,
    pub modified_at: DateTime<Utc>,
}

impl Item {
    pub fn from(item: item::Model) -> Self {
        Self {
            id: item.id,
            name: item.name.into(),
            content: item.content.into(),
            kind: item.kind,
            language: item.language.map(SharedString::from),
            tags: tags_from_json(item.tags),
            created_at: item.created_at,
            modified_at: item.modified_at,
        }
    }
}

fn tags_from_json(tags: Value) -> Vec<SharedString> {
    if let Value::Array(values) = tags {
        return values
            .into_iter()
            .filter_map(|v| match v {
                Value::String(s) => Some(SharedString::from(s)),
                _ => None,
            })
            .collect();
    }
    Vec::new()
}

pub struct Link {
    from: uuid::Uuid,
    to: uuid::Uuid,
}
