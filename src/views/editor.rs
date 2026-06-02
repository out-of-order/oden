use gpui::{AppContext, Context, Entity, Render, Styled, Window, px};
use gpui_component::input::{Input, InputState};
pub struct EditorView {
    input_state: Entity<InputState>,
}

impl EditorView {
    pub fn new(cx: &mut Context<Self>, window: &mut Window) -> Self {
        let input_state = cx.new(|cx| {
            InputState::new(window, cx)
                .multi_line(true)
                .code_editor("markdown")
                .searchable(true)
                .line_number(true)
        });
        EditorView { input_state }
    }
}

impl Render for EditorView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl gpui::IntoElement {
        Input::new(&self.input_state)
            .border(px(0.0))
            .font_family("JetBrainsMono Nerd Font Mono")
    }
}
