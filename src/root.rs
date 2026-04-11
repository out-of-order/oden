#![allow(dead_code)]

use gpui::{
    Context, Entity, FocusHandle, InteractiveElement, IntoElement, ParentElement, Render, Styled,
    Subscription, div, px,
};
use gpui_component::{
    ActiveTheme, Icon,
    button::{Button, ButtonVariants},
};

use crate::{
    actions::{self, GraphMode, ListMode, SearchMode, Settings},
    icons::IconName,
    state::{AppMode, AppState},
};

pub struct AppRoot {
    pub app_state: Entity<AppState>,
    pub focus: FocusHandle,
    pub _state_sub: Subscription,
}

impl AppRoot {
    pub fn new(app_state: Entity<AppState>, cx: &mut Context<Self>) -> Self {
        Self {
            _state_sub: cx.observe(&app_state, |_, _, cx| {
                cx.notify();
            }),
            app_state,
            focus: cx.focus_handle(),
        }
    }

    fn nav_button(&self, icon: IconName, mode: AppMode, tooltip: &'static str) -> Button {
        let focus = self.focus.clone();
        Button::new(tooltip)
            .icon(Icon::new(icon))
            .ghost()
            .on_click(move |_event, window, cx| {
                focus.focus(window);
                match mode {
                    AppMode::List => window.dispatch_action(Box::new(actions::ListMode), cx),
                    AppMode::Search => window.dispatch_action(Box::new(actions::SearchMode), cx),
                    AppMode::Settings => window.dispatch_action(Box::new(actions::Settings), cx),
                    AppMode::Graph => window.dispatch_action(Box::new(actions::GraphMode), cx),
                }
            })
            .tooltip(tooltip)
    }

    fn render_sidebar(&self, cx: &mut gpui::Context<Self>) -> impl IntoElement + use<> {
        let border_color = cx.theme().border;
        let sidebar_bg = cx.theme().sidebar;

        let icon_rail = vec![
            self.nav_button(IconName::List, AppMode::List, "List Mode"),
            self.nav_button(IconName::Search, AppMode::Search, "Search Mode"),
            self.nav_button(IconName::Graph, AppMode::Graph, "Graph Mode"),
        ];

        let focus = self.focus.clone();

        div()
            .relative()
            .h_full()
            .w(px(56.0))
            .flex_shrink_0()
            .flex_col()
            .border_color(border_color)
            .bg(sidebar_bg)
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
                            .icon(Icon::new(IconName::Settings))
                            .on_click(move |_event, window, cx| {
                                focus.focus(window);
                                window.dispatch_action(Box::new(actions::Settings), cx)
                            }),
                    ),
            )
    }

    fn update_mode(&mut self, mode: AppMode, cx: &mut Context<Self>) {
        self.app_state.update(cx, |app_state, cx| {
            if app_state.mode != mode {
                app_state.mode = mode;
                cx.notify();
            }
        })
    }
}

impl Render for AppRoot {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let bg = cx.theme().background;
        let sidebar = self.render_sidebar(cx);
        let mode = self.app_state.read(cx).mode.to_string();
        div()
            .track_focus(&self.focus)
            .on_action(cx.listener(|this, _action: &ListMode, _window, cx| {
                this.update_mode(AppMode::List, cx)
            }))
            .on_action(cx.listener(|this, _action: &SearchMode, _window, cx| {
                this.update_mode(AppMode::Search, cx)
            }))
            .on_action(cx.listener(|this, _action: &GraphMode, _window, cx| {
                this.update_mode(AppMode::Graph, cx)
            }))
            .on_action(cx.listener(|this, _action: &Settings, _window, cx| {
                this.update_mode(AppMode::Settings, cx)
            }))
            .flex()
            .flex_row()
            .h_full()
            .w_full()
            .bg(bg)
            .child(sidebar)
            .child(div().p_4().child(mode))
    }
}

#[cfg(test)]
mod tests {
    use gpui::{AppContext, TestAppContext, WindowHandle, WindowOptions};

    use crate::{
        actions,
        root::AppRoot,
        state::{AppMode, AppState},
    };

    fn setup(cx: &mut TestAppContext) -> (WindowHandle<AppRoot>, gpui::Entity<AppState>) {
        cx.update(|cx| {
            gpui_component::init(cx);
            let window = cx
                .open_window(WindowOptions::default(), |_window, cx| {
                    let app_state = cx.new(|_| AppState::init());
                    cx.new(|cx| AppRoot::new(app_state, cx))
                })
                .unwrap();
            let app_state = window.root(cx).unwrap().read(cx).app_state.clone();
            (window, app_state)
        })
    }

    #[gpui::test]
    fn test_icon_rail_navigation(cx: &mut TestAppContext) {
        let (window, app_state) = setup(cx);

        let cases: Vec<(Box<dyn gpui::Action>, AppMode)> = vec![
            (Box::new(actions::SearchMode), AppMode::Search),
            (Box::new(actions::GraphMode), AppMode::Graph),
            (Box::new(actions::Settings), AppMode::Settings),
            (Box::new(actions::ListMode), AppMode::List),
        ];

        for (action, expected_mode) in cases {
            window
                .update(cx, |root, window, cx| {
                    root.focus.focus(window);
                    window.dispatch_action(action, cx);
                })
                .unwrap();
            cx.update(|cx| {
                assert_eq!(
                    app_state.read(cx).mode,
                    expected_mode,
                    "failed for {expected_mode}"
                );
            });
        }
    }
}
