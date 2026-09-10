use std::sync::Arc;

use gpui::{AsyncApp, Global};
use oden_core::repository::{ItemRepositoryTrait, TitleRepositoryTrait};

pub struct AppRepository {
    pub item: Arc<dyn ItemRepositoryTrait + Send + Sync>,
    pub title: Arc<dyn TitleRepositoryTrait + Send + Sync>,
}

impl Global for AppRepository {}

impl AppRepository {
    pub fn init(
        cx: &mut AsyncApp,
        item: Arc<dyn ItemRepositoryTrait + Send + Sync>,
        title: Arc<dyn TitleRepositoryTrait + Send + Sync>,
    ) {
        let repo = Self { item, title };
        cx.update(move |cx| {
            cx.set_global(repo);
        });
    }
}
