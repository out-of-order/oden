use gpui::{AppContext, TestAppContext, WindowHandle, WindowOptions};

use crate::root::AppRoot;
use crate::state::AppState;
use crate::store::ItemStore;

pub fn setup(cx: &mut TestAppContext) -> (WindowHandle<AppRoot>, gpui::Entity<AppState>) {
    cx.update(|cx| {
        gpui_component::init(cx);
        ItemStore::init(cx);
        let window = cx
            .open_window(WindowOptions::default(), |window, cx| {
                let app_state = cx.new(|_| AppState::init());
                cx.new(|cx| AppRoot::new(app_state, window, cx))
            })
            .unwrap();
        let app_state = window.root(cx).unwrap().read(cx).app_state.clone();
        (window, app_state)
    })
}
