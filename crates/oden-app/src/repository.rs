use std::sync::Arc;

use gpui::{AsyncApp, Global};
use oden_core::repository::{
    ContentRepositoryTrait, ItemRepositoryTrait, SyncLinkRepositoryTrait, TitleRepositoryTrait,
};

pub struct ItemRepository(pub Arc<dyn ItemRepositoryTrait + Send + Sync>);
pub struct ContentRepository(pub Arc<dyn ContentRepositoryTrait + Send + Sync>);
pub struct TitleRepository(pub Arc<dyn TitleRepositoryTrait + Send + Sync>);
pub struct LinkRepository(pub Arc<dyn SyncLinkRepositoryTrait + Send + Sync>);

impl Global for ItemRepository {}
impl Global for ContentRepository {}
impl Global for TitleRepository {}
impl Global for LinkRepository {}

impl ItemRepository {
    pub fn init(cx: &mut AsyncApp, item: ItemRepository) {
        cx.update(move |cx| {
            cx.set_global(item);
        })
    }
}

impl ContentRepository {
    pub fn init(cx: &mut AsyncApp, content: ContentRepository) {
        cx.update(move |cx| {
            cx.set_global(content);
        })
    }
}

impl LinkRepository {
    pub fn init(cx: &mut AsyncApp, sync_link: LinkRepository) {
        cx.update(move |cx| {
            cx.set_global(sync_link);
        })
    }
}

impl TitleRepository {
    pub fn init(cx: &mut AsyncApp, title: TitleRepository) {
        cx.update(move |cx| {
            cx.set_global(title);
        })
    }
}
