#![allow(dead_code)]

use gpui::{
    AnyElement, AppContext, Context, Entity, FocusHandle, InteractiveElement, IntoElement,
    ParentElement, Render, SharedString, Styled, Subscription, div, px,
};
use gpui_component::{
    ActiveTheme, Icon, TitleBar,
    button::{Button, ButtonVariants},
    label::Label,
};
use uuid::Uuid;

use crate::{
    actions::{self, GraphMode, ListMode, SearchMode, SelectItem, Settings},
    icons::IconName,
    state::{AppMode, SelectedIdState},
    views::list::ListView,
};

pub struct AppRoot {
    pub(crate) app_mode: Entity<AppMode>,
    pub(crate) selected_id_state: Entity<SelectedIdState>,
    pub(crate) list_view: Entity<ListView>,
    pub(crate) focus: FocusHandle,
    pub(crate) _state_sub: Subscription,
}

const APP_VERSION: &str = concat!("v", env!("CARGO_PKG_VERSION"));

impl AppRoot {
    pub fn new(
        app_mode: Entity<AppMode>,
        selected_id_state: Entity<SelectedIdState>,
        window: &mut gpui::Window,
        cx: &mut Context<Self>,
    ) -> Self {
        Self {
            _state_sub: cx.observe(&app_mode, |_, _, cx| {
                cx.notify();
            }),
            app_mode: app_mode.clone(),
            list_view: cx
                .new(|cx| ListView::new(window, cx, cx.focus_handle(), selected_id_state.clone())),
            selected_id_state,
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
        self.app_mode.update(cx, |app_mode, cx| {
            if *app_mode != mode {
                *app_mode = mode;
                cx.notify();
            }
        })
    }

    fn update_selected_id(&mut self, selected_id: Uuid, cx: &mut Context<Self>) {
        self.selected_id_state.update(cx, |selected_id_state, cx| {
            if selected_id_state.selected_id != Some(selected_id) {
                selected_id_state.selected_id = Some(selected_id);
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
        let muted = cx.theme().muted_foreground;
        let sidebar = self.render_sidebar(cx);
        let mode = self.app_mode.read(cx);
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
            .on_action(cx.listener(|this, action: &SelectItem, _window, cx| {
                this.update_selected_id(action.selected_id, cx);
            }))
            .flex()
            .flex_col()
            .h_full()
            .w_full()
            .bg(bg)
            .child(
                TitleBar::new()
                    .child("Oden")
                    .child(Label::new(APP_VERSION).text_color(muted)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .h_full()
                    .w_full()
                    .bg(bg)
                    .child(sidebar)
                    .child(self.render_mode(*mode)),
            )
    }
}

impl AppRoot {
    fn render_mode(&self, mode: AppMode) -> AnyElement {
        match mode {
            AppMode::List => self.list_view.clone().into_any_element(),
            _ => SharedString::from(mode.to_string()).into_any_element(),
        }
    }
}

#[cfg(test)]
mod tests {
    use gpui::TestAppContext;

    use crate::{actions, state::AppMode, testutils::setup};

    #[gpui::test]
    fn test_icon_rail_navigation(cx: &mut TestAppContext) {
        let (window, app_mode_state, _selected_id_state) = setup(cx);

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
                    *app_mode_state.read(cx),
                    expected_mode,
                    "failed for {expected_mode}"
                );
            });
        }
    }
}
