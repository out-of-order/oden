use std::{borrow::Cow, path::PathBuf};

use anyhow::anyhow;
use gpui::{
    App, AppContext, Application, AssetSource, Entity, Result, SharedString, WindowOptions,
};
use gpui_component::{Root, Theme, ThemeRegistry};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "./assets"]
#[include = "icons/**/*.svg"]
pub struct Assets;

impl AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        if path.is_empty() {
            return Ok(None);
        }
        Self::get(path)
            .map(|f| Some(f.data))
            .ok_or_else(|| anyhow!("could not find asset at path \"{path}\""))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter_map(|p| p.starts_with(path).then(|| p.into()))
            .collect())
    }
}

use crate::{root::AppRoot, state::AppState, store::ItemStore};

mod icons;
mod models;
mod root;
mod state;
mod store;
mod views;

fn main() {
    Application::new().with_assets(Assets).run(|cx: &mut App| {
        gpui_component::init(cx);
        ItemStore::init(cx);
        setup_theme(cx);
        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let app_state: Entity<AppState> = cx.new(|_| AppState::init());
                let view = cx.new(|_| AppRoot::new(app_state));
                cx.new(|cx| Root::new(view, window, cx))
            })
            .unwrap();
        })
        .detach();
    });
}

fn setup_theme(cx: &mut App) {
    let theme_name = SharedString::from("Tokyo Night");
    if let Err(err) = ThemeRegistry::watch_dir(PathBuf::from("./themes"), cx, move |cx| {
        if let Some(theme) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
            Theme::global_mut(cx).apply_config(&theme);
        }
    }) {
        println!("there was an error loading the theme {:?}", err)
    }
}
