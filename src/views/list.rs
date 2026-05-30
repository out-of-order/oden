use gpui::{
    AppContext, Context, Entity, ParentElement, Render, SharedString, Styled, Subscription, Window,
    div, px,
};
use gpui_component::{
    ActiveTheme, Icon, IndexPath, Sizable, StyledExt,
    input::{Input, InputEvent, InputState},
    label::Label,
    list::{List, ListDelegate, ListItem, ListState},
};

use crate::models::Item;
use crate::{ItemStore, icons::IconName};

pub struct ListView {
    input_state: Entity<InputState>,
    _input_sub: Subscription,
    list_state: Entity<ListState<ItemListDelegate>>,
}

impl ListView {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let input_state =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search for anything..."));
        let items: Vec<Item> = ItemStore::get(cx).items().values().cloned().collect();
        let delegate: ItemListDelegate = ItemListDelegate {
            items: items.clone(),
            all_items: items.clone(),
            selected_index: None,
        };
        let list_state = cx.new(|cx| ListState::new(delegate, window, cx));
        let list_state_clone = list_state.clone();
        let _subscription = cx.subscribe_in(
            &input_state,
            window,
            move |_view, state, event, _window, cx| {
                if let InputEvent::Change = event {
                    let query = state.read(cx).value().clone();
                    cx.update_entity(&list_state_clone, |state, _cx| {
                        state.delegate_mut().items = state
                            .delegate()
                            .all_items
                            .iter()
                            .filter(|item| {
                                item.name
                                    .to_string()
                                    .to_lowercase()
                                    .contains(&query.to_string().to_lowercase())
                            })
                            .cloned()
                            .collect();
                    });
                    cx.notify();
                }
            },
        );
        Self {
            input_state,
            list_state,
            _input_sub: _subscription,
        }
    }
}

struct ItemListDelegate {
    items: Vec<Item>,
    all_items: Vec<Item>,
    selected_index: Option<IndexPath>,
}

fn preview_content(s: SharedString) -> SharedString {
    let truncated = &s[..s.floor_char_boundary(50.min(s.len()))];
    let truncated: Vec<_> = truncated.lines().filter(|line| !line.is_empty()).collect();
    truncated.join(" ").into()
}

impl ListDelegate for ItemListDelegate {
    type Item = ListItem;
    fn items_count(&self, _section: usize, _cx: &gpui::App) -> usize {
        self.items.len()
    }

    fn render_item(
        &mut self,
        ix: IndexPath,
        _window: &mut Window,
        cx: &mut Context<gpui_component::list::ListState<Self>>,
    ) -> Option<Self::Item> {
        let theme = cx.theme();
        let muted_color = theme.muted_foreground;
        let date_color = theme.blue;
        let border_color = theme.border;
        self.items.get(ix.row).map(|item| {
            ListItem::new(ix)
                .h_32()
                .overflow_hidden()
                .p_2()
                .flex()
                .flex_col()
                .border_b(px(1.0))
                .border_color(border_color)
                .child(Label::new(item.name.clone()))
                .child(
                    Label::new(item.created_at.date_naive().to_string())
                        .text_color(date_color)
                        .font_thin(),
                )
                .child(Label::new(preview_content(item.content.clone())).text_color(muted_color))
                .selected(Some(ix) == self.selected_index)
        })
    }
    fn set_selected_index(
        &mut self,
        ix: Option<IndexPath>,
        _window: &mut Window,
        cx: &mut Context<gpui_component::list::ListState<Self>>,
    ) {
        self.selected_index = ix;
        cx.notify();
    }
}

impl Render for ListView {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let theme = cx.theme();
        let border_color = theme.border;
        let muted_color = theme.muted_foreground;
        div()
            .w(px(224.0))
            .border_r(px(1.0))
            .border_color(border_color)
            .flex()
            .flex_col()
            .child(
                div()
                    .w_full()
                    .flex()
                    .flex_row()
                    .justify_center()
                    .items_center()
                    .border_b(px(1.0))
                    .border_color(border_color)
                    .p_2()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .justify_center()
                            .gap(px(5.0))
                            .w_full()
                            .child(Label::new(SharedString::from("All Items")).font_bold())
                            .child(Input::new(&self.input_state).prefix(
                                Icon::new(IconName::Search).small().text_color(muted_color),
                            )),
                    ),
            )
            .child(List::new(&self.list_state).flex().flex_col().gap_5())
    }
}
