#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use gpui::{App, AsyncApp, Global, SharedString};
use oden_core::entities::item;
use oden_core::repository::{
    ItemRepository, ItemRepositoryTrait, LinkRepository, LinkRepositoryTrait,
};
use tokio::sync::watch::Sender;
use uuid::Uuid;

#[cfg(any(test, debug_assertions))]
use crate::fixtures::mock_items;
use crate::models::Item;

pub struct ItemStore {
    pub items: HashMap<uuid::Uuid, Item>,
    pub item_content_tx: HashMap<Uuid, Sender<SharedString>>,
    pub title_input_tx: HashMap<Uuid, Sender<SharedString>>,
    pub links: HashMap<Uuid, Vec<Uuid>>,
}

impl Global for ItemStore {}

impl ItemStore {
    pub fn items(&self) -> HashMap<uuid::Uuid, Item> {
        self.items.clone()
    }

    #[cfg(any(debug_assertions, test))]
    pub fn mock_store(cx: &mut App) {
        let mut store = ItemStore {
            items: HashMap::new(),
            item_content_tx: HashMap::new(),
            links: HashMap::new(),
            title_input_tx: HashMap::new(),
        };
        for item in mock_items() {
            store.items.insert(item.id, item);
        }
        cx.set_global(store);
    }

    pub async fn init(
        cx: &mut AsyncApp,
        item_repository: &ItemRepository,
        link_repository: Arc<LinkRepository>,
    ) -> anyhow::Result<()> {
        let items: Vec<item::Model> = item_repository.find_all().await?;
        let links = link_repository.find_all().await?;
        cx.update(move |cx| {
            let mut store = ItemStore {
                items: HashMap::new(),
                item_content_tx: HashMap::new(),
                title_input_tx: HashMap::new(),
                links: HashMap::new(),
            };
            for item in items {
                store.items.insert(item.id, Item::from(item));
            }
            for link in links {
                store
                    .links
                    .entry(link.source_item_id)
                    .and_modify(|v| v.push(link.target_item_id))
                    .or_insert_with(|| vec![link.target_item_id]);
            }
            cx.set_global(store);
        });
        Ok(())
    }

    pub fn get(cx: &App) -> &Self {
        cx.global::<ItemStore>()
    }

    pub fn get_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<ItemStore>()
    }

    /// Replaces the set of outgoing links for `from` with `targets`,
    /// mirroring `SyncLinkRepositoryTrait::sync_links` but against the
    /// in-memory store instead of the database.
    pub fn sync_links(&mut self, from: Uuid, targets: Vec<Uuid>) {
        if targets.is_empty() {
            self.links.remove(&from);
        } else {
            self.links.insert(from, targets);
        }
    }
}
