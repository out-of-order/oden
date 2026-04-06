use std::path::PathBuf;

use gpui::{App, AppContext, Application, Entity, SharedString, WindowOptions};
use gpui_component::{Root, Theme, ThemeRegistry};

use crate::{root::AppRoot, state::AppState, store::ItemStore};

mod models;
mod root;
mod state;
mod store;
mod views;

fn main() {
    Application::new().run(|cx: &mut App| {
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
    // Load and watch themes from ./themes directory
    if let Err(err) = ThemeRegistry::watch_dir(PathBuf::from("./themes"), cx, move |cx| {
        if let Some(theme) = ThemeRegistry::global(cx).themes().get(&theme_name).cloned() {
            Theme::global_mut(cx).apply_config(&theme);
        }
    }) {
        println!("there was an error loading the theme {:?}", err)
    }
}
