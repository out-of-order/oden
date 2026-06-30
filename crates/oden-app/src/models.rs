#![allow(dead_code)]

use chrono::{DateTime, Utc};
use gpui::SharedString;

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

#[derive(Clone)]
pub enum ItemKind {
    Note,
    Snippet,
    Command,
}

pub struct Link {
    from: uuid::Uuid,
    to: uuid::Uuid,
}
