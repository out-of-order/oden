#![allow(dead_code)]

use std::collections::HashMap;

use gpui::{App, Global};
use gpui_component::link::Link;

use crate::fixtures::mock_items;
use crate::models::Item;

pub struct ItemStore {
    items: HashMap<uuid::Uuid, Item>,
    links: Vec<Link>,
}

impl Global for ItemStore {}

impl ItemStore {
    pub fn items(&self) -> HashMap<uuid::Uuid, Item> {
        self.items.clone()
    }
    pub fn init(cx: &mut App) {
        let mut store = ItemStore {
            items: HashMap::new(),
            links: Vec::new(),
        };
        #[cfg(debug_assertions)]
        for item in mock_items() {
            store.items.insert(item.id, item);
        }
        cx.set_global(store);
    }

    pub fn get(cx: &mut App) -> &Self {
        cx.global::<ItemStore>()
    }

    pub fn get_mut(cx: &mut App) -> &mut Self {
        cx.global_mut::<ItemStore>()
    }
}
