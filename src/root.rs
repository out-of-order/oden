use anyhow::Context;
use gpui::{Entity, ParentElement, Render, Styled, div};
use gpui_component::ActiveTheme;

use crate::state::AppState;

#[allow(dead_code)]
pub struct AppRoot {
    pub app_state: Entity<AppState>,
}

impl AppRoot {
    pub fn new(app_state: Entity<AppState>) -> Self {
        Self { app_state }
    }
}

impl Render for AppRoot {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let theme = cx.theme();
        div()
            .flex()
            .flex_row()
            .h_full()
            .w_full()
            .items_center()
            .justify_center()
            .bg(theme.background)
            .child("Hello World")
    }
}
