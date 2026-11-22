use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Subscription, Window,
    div,
};
use gpui_component::input::{Editor, EditorState};

use crate::appstatus::AppStatus;

const EMPTY_LOG_MESSAGE: &str = "No unresolved issues";

pub struct IssueLogsView {
    pub(crate) input_state: Entity<EditorState>,
    _app_status_sub: Subscription,
}

impl IssueLogsView {
    pub fn new(cx: &mut Context<Self>, window: &mut Window) -> Self {
        let input_state = cx.new(|cx| {
            EditorState::new(window, cx)
                // this is essential so the layout does not break
                // if a message is too long.
                .soft_wrap(true)
                .line_number(true)
        });
        Self::refresh_log(&input_state, window, cx);
        let _app_status_sub = cx.observe_global_in::<AppStatus>(window, |this, window, cx| {
            Self::refresh_log(&this.input_state, window, cx);
        });
        Self {
            input_state,
            _app_status_sub,
        }
    }

    fn refresh_log(input_state: &Entity<EditorState>, window: &mut Window, cx: &mut Context<Self>) {
        let log = cx.global::<AppStatus>().format_log();
        let log = if log.is_empty() {
            EMPTY_LOG_MESSAGE.to_string()
        } else {
            log
        };
        input_state.update(cx, |input_state, cx| {
            input_state.set_value(log, window, cx);
        });
    }
}

impl Render for IssueLogsView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .child(Editor::new(&self.input_state).readonly(true).h_full())
    }
}
