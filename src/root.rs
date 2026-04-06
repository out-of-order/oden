use gpui::{Entity, ParentElement, Render, Styled, div, px};
use gpui_component::{
    ActiveTheme, Icon,
    button::{Button, ButtonVariants},
};

use crate::{icons::IconName, state::AppState};

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
        let icon_rail: Vec<Button> = vec![
            Button::new("list")
                .icon(Icon::new(IconName::List))
                .ghost()
                .tooltip("List Mode"),
            Button::new("Search")
                .icon(Icon::new(IconName::Search))
                .ghost()
                .tooltip("Search Mode"),
            Button::new("Graph")
                .icon(Icon::new(IconName::Graph))
                .ghost()
                .tooltip("Graph Mode"),
        ];
        div()
            .flex()
            .flex_row()
            .h_full()
            .w_full()
            .bg(theme.background)
            // toolbar
            .child(
                div()
                    .relative()
                    .h_full()
                    .w(px(56.0))
                    .flex_shrink_0()
                    .flex_col()
                    .border_color(theme.border)
                    .bg(theme.sidebar)
                    .border_r(px(1.0))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_4()
                            .p_2()
                            .children(icon_rail),
                    )
                    .child(
                        div()
                            .absolute()
                            .bottom_0()
                            .w_full()
                            .flex()
                            .flex_col()
                            .items_center()
                            .p_2()
                            .child(
                                Button::new("settings")
                                    .ghost()
                                    .tooltip("Settings")
                                    .icon(Icon::new(IconName::Settings)),
                            ),
                    ),
            )
    }
}
