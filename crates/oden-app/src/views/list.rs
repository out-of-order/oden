use std::sync::Arc;

use gpui::{
    AnyElement, App, AppContext, AsyncApp, BorrowAppContext, Context, CursorStyle::PointingHand,
    Entity, FocusHandle, Focusable, FontWeight, Hsla, InteractiveElement, IntoElement,
    KeyDownEvent, MouseButton, ParentElement, ReadGlobal, Render, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Window, deferred, div,
    prelude::FluentBuilder, px,
};
use gpui_base::{Input, InputBase, input::InputEditorStyle};
use gpui_component::{
    ActiveTheme, Icon, IndexPath, Sizable,
    button::{Button, ButtonVariants},
    h_flex,
    input::{InputEvent, InputState},
    label::Label,
    list::{List, ListDelegate, ListItem, ListState},
    menu::ContextMenuExt,
    tag::Tag,
    v_flex,
};
use oden_core::errors::PersistenceError;
use oden_core::repository::{DeleteItemRepositoryTrait, ItemRepositoryTrait};
use uuid::Uuid;

use crate::{
    ItemStore,
    actions::{self, DeleteItem, NewItem, SelectItem},
    appstatus::{AppOperation, AppStatus, Field, Issue},
    icons::IconName,
    inputvaluewatcher::InputPersistence,
    persistence::{PersistencePerNote, PersistenceStatus},
    repository::{DeleteItemRepository, ItemRepository, TagsRepository, TitleRepository},
    state::SelectedIdState,
};
use crate::{models::Item, views::editor::EditorView};

pub(crate) struct ListView {
    _subscriptions: ListSubscriptions,
    pub(crate) focus_handle: FocusHandle,
    pub(crate) entities: ListEntities,
    editing_tag: Option<usize>,
    highlighted_suggestion: Option<usize>,
    suggestions_dismissed: bool,
}

#[derive(Clone)]
pub(crate) struct ListEntities {
    pub(crate) input_state: Entity<InputState>,
    editor: Entity<EditorView>,
    list_state: Entity<ListState<ItemListDelegate>>,
    selected_id_state: Entity<SelectedIdState>,
    pub(crate) show_list_state: Entity<bool>,
    pub(crate) title_input_state: Entity<InputState>,
    pub(crate) tags_input_state: Entity<InputState>,
}

impl ListView {
    pub(crate) fn new(
        window: &mut Window,
        cx: &mut Context<Self>,
        focus_handle: FocusHandle,
        selected_id_state: Entity<SelectedIdState>,
    ) -> Self {
        let entities = Self::build_entities(window, cx, focus_handle.clone(), selected_id_state);
        let subscriptions: ListSubscriptions =
            Self::wire_subscriptions(window, cx, entities.clone());
        Self {
            focus_handle,
            _subscriptions: subscriptions,
            entities,
            editing_tag: None,
            highlighted_suggestion: None,
            suggestions_dismissed: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn editor(&self) -> Entity<EditorView> {
        self.entities.editor.clone()
    }

    fn build_entities(
        window: &mut Window,
        cx: &mut Context<Self>,
        focus_handle: FocusHandle,
        selected_id_state: Entity<SelectedIdState>,
    ) -> ListEntities {
        let input_state = Self::build_input_state(window, cx);
        let list_state = Self::build_list_state(window, cx, focus_handle.clone());
        let editor = Self::build_editor_view(window, cx, selected_id_state.clone());
        let title_input_state = Self::build_title_input_state(window, cx);
        let tags_input_state = Self::build_tags_input_state(window, cx);
        let show_list_state = Self::build_show_list_state(cx);
        ListEntities {
            input_state,
            editor,
            list_state,
            selected_id_state,
            title_input_state,
            tags_input_state,
            show_list_state,
        }
    }

    async fn delete_item(
        cx: &mut AsyncApp,
        repository: Arc<dyn DeleteItemRepositoryTrait + Send + Sync>,
        selected_id_state: Entity<SelectedIdState>,
    ) -> anyhow::Result<()> {
        let Some(selected_id) = cx.update(|cx| selected_id_state.read(cx).selected_id) else {
            return Ok(());
        };
        repository.as_ref().delete_item(selected_id).await?;
        cx.update(|cx| {
            cx.update_global::<ItemStore, _>(|store, _app| {
                store.items.remove(&selected_id);
            })
        });
        Ok(())
    }

    async fn add_empty_item(
        cx: &mut AsyncApp,
        repository: Arc<dyn ItemRepositoryTrait + Send + Sync>,
        selected_id_state: Entity<SelectedIdState>,
    ) -> anyhow::Result<()> {
        let model = repository.as_ref().create_item().await?;
        let item = Item::from(model);
        let item_id = item.id;
        cx.update(|cx| {
            cx.update_global::<ItemStore, _>(|store, _app| {
                store.items.insert(item_id, item);
            });
            cx.update_entity(&selected_id_state, |state, cx| {
                state.selected_id = Some(item_id);
                cx.notify();
            });
        });
        Ok(())
    }

    fn build_input_state(window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        cx.new(|cx| InputState::new(window, cx).placeholder("Search for anything..."))
    }

    fn build_title_input_state(window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        cx.new(|cx| {
            let mut input_state = InputState::new(window, cx);
            input_state.set_editor_style(InputEditorStyle {
                caret: cx.theme().caret,
                ..Default::default()
            });
            input_state
        })
    }

    fn build_tags_input_state(window: &mut Window, cx: &mut Context<Self>) -> Entity<InputState> {
        cx.new(|cx| {
            let mut input_state = InputState::new(window, cx).placeholder("Add tag");
            input_state.set_editor_style(InputEditorStyle {
                caret: cx.theme().caret,
                ..Default::default()
            });
            input_state
        })
    }

    fn build_show_list_state(cx: &mut Context<Self>) -> Entity<bool> {
        cx.new(|_| true)
    }

    fn build_editor_view(
        window: &mut Window,
        cx: &mut Context<Self>,
        selected_id_state: Entity<SelectedIdState>,
    ) -> Entity<EditorView> {
        cx.new(|cx| EditorView::new(cx, window, selected_id_state))
    }

    fn build_list_state(
        window: &mut Window,
        cx: &mut Context<Self>,
        focus_handle: FocusHandle,
    ) -> Entity<ListState<ItemListDelegate>> {
        let items: Vec<Item> = ItemStore::get(cx).items().values().cloned().collect();
        let mut delegate: ItemListDelegate = ItemListDelegate {
            items: Vec::new(),
            all_items: Vec::new(),
            selected_index: None,
            focus: focus_handle.clone(),
            query: String::new(),
        };
        delegate.set_all_items(items);
        cx.new(|cx| ListState::new(delegate, window, cx))
    }

    fn wire_subscriptions(
        window: &mut Window,
        cx: &mut Context<Self>,
        entities: ListEntities,
    ) -> ListSubscriptions {
        let list_state_clone = entities.list_state.clone();
        let input_sub = cx.subscribe_in(
            &entities.input_state,
            window,
            move |_view, state, event, _window, cx| {
                if let InputEvent::Change = event {
                    let query = state.read(cx).value().clone();
                    cx.update_entity(&list_state_clone, |state, _cx| {
                        state.delegate_mut().set_query(query);
                    });
                    cx.notify();
                }
            },
        );
        let selected_id_state_clone = entities.selected_id_state.clone();
        let list_state_clone = entities.list_state.clone();
        let store_sub = cx.observe_global::<ItemStore>(move |_store, cx| {
            let all_items: Vec<Item> = ItemStore::get(cx).items().values().cloned().collect();
            let selected_id = selected_id_state_clone.read(cx).selected_id;
            cx.update_entity(&list_state_clone, |state, _cx| {
                let delegate = state.delegate_mut();
                delegate.set_all_items(all_items);
                delegate.selected_index = selected_id
                    .and_then(|id| delegate.items.iter().position(|item| item.id == id))
                    .map(IndexPath::new);
            });
            cx.notify();
        });

        let title_input_state_sub = cx.subscribe_in(
            &entities.title_input_state,
            window,
            move |view, title_input_state, event: &InputEvent, _window, cx| {
                if let InputEvent::Change = event {
                    let Some(selected_id) = view.entities.selected_id_state.read(cx).selected_id
                    else {
                        return;
                    };
                    cx.update_global::<ItemStore, ()>(|store, cx| {
                        let item = store.items.get_mut(&selected_id);
                        if let Some(item) = item {
                            item.name = title_input_state.read(cx).value();
                        }
                    });
                    let title = title_input_state.read(cx).value();
                    let store = ItemStore::get_mut(cx);
                    let needs_new_receiver = match store.title_input_tx.get(&selected_id) {
                        Some(tx) => tx.send(title.clone()).is_err(),
                        None => true,
                    };
                    if needs_new_receiver {
                        let repository = cx.global::<TitleRepository>().0.clone();
                        InputPersistence::spawn(
                            cx,
                            selected_id,
                            title.clone(),
                            Field::Title,
                            move |title: SharedString| {
                                let repository = repository.clone();
                                Box::pin(async move {
                                    repository
                                        .update_title(selected_id, title.to_string())
                                        .await
                                        .map_err(PersistenceError::from)
                                })
                            },
                        );
                    }
                }
            },
        );

        let tags_input_state_sub = cx.subscribe_in(
            &entities.tags_input_state,
            window,
            |view, tags_input_state, event: &InputEvent, window, cx| match event {
                InputEvent::Change => view.commit_comma_separated(tags_input_state, window, cx),
                InputEvent::PressEnter { .. } => view.commit_tags_input(true, window, cx),
                InputEvent::Blur => view.commit_tags_input(false, window, cx),
                InputEvent::Focus => {
                    view.suggestions_dismissed = false;
                    cx.notify();
                }
            },
        );

        let selected_id_sub = cx.observe_in(
            &entities.selected_id_state,
            window,
            move |this, selected_id_state, window, cx| {
                let selected_id = selected_id_state.read(cx).selected_id;
                let selected_index = {
                    let state = entities.list_state.read(cx);
                    selected_id
                        .and_then(|id| state.delegate().items.iter().position(|item| item.id == id))
                        .map(IndexPath::new)
                };
                entities.list_state.update(cx, |state, cx| {
                    state.set_selected_index(selected_index, window, cx);
                });
                this.editing_tag = None;
                this.highlighted_suggestion = None;
                entities.tags_input_state.update(cx, |state, cx| {
                    state.set_value("", window, cx);
                });
                entities.title_input_state.update(cx, |state, cx| {
                    selected_id.inspect(|id| {
                        let item_maybe = ItemStore::get(cx).items.get(id);
                        if let Some(item) = item_maybe {
                            let title = item.name.clone();
                            state.set_value(title, window, cx);
                        }
                    });
                });
            },
        );

        ListSubscriptions {
            _input_sub: input_sub,
            _store_sub: store_sub,
            _selected_id_sub: selected_id_sub,
            _title_input_state_sub: title_input_state_sub,
            _tags_input_state_sub: tags_input_state_sub,
        }
    }
}

struct ListSubscriptions {
    _input_sub: Subscription,
    _store_sub: Subscription,
    _selected_id_sub: Subscription,
    _title_input_state_sub: Subscription,
    _tags_input_state_sub: Subscription,
}

struct ItemListDelegate {
    items: Vec<Item>,
    all_items: Vec<Item>,
    selected_index: Option<IndexPath>,
    focus: FocusHandle,
    query: String,
}

fn tag_color(name: &str, theme: &gpui_component::Theme) -> Hsla {
    let palette = [
        theme.blue,
        theme.green,
        theme.yellow,
        theme.red,
        theme.magenta,
        theme.cyan,
    ];
    let hash = name.bytes().fold(2166136261u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(16777619)
    });
    palette[hash as usize % palette.len()]
}

fn tag_chip(name: SharedString, theme: &gpui_component::Theme) -> Tag {
    let color = tag_color(&name, theme);
    Tag::custom(color.opacity(0.15), color, color.opacity(0.4))
        .small()
        .child(name)
}

fn preview_content(s: SharedString) -> SharedString {
    let truncated = &s[..s.floor_char_boundary(50.min(s.len()))];
    let truncated: Vec<_> = truncated.lines().filter(|line| !line.is_empty()).collect();
    truncated.join(" ").into()
}

impl ItemListDelegate {
    fn set_all_items(&mut self, items: Vec<Item>) {
        self.all_items = items;
        self.recompute_items();
    }

    fn set_query(&mut self, query: impl Into<String>) {
        self.query = query.into().to_lowercase();
        self.recompute_items();
    }

    fn recompute_items(&mut self) {
        let query = self.query.as_str();
        let mut visible: Vec<Item> = self
            .all_items
            .iter()
            .filter(|item| item.name.to_string().to_lowercase().contains(query))
            .cloned()
            .collect();
        visible.sort_by_key(|i| i.name.to_lowercase());
        self.items = visible;
    }
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
        let size = cx.theme().mono_font_size;
        let focus = self.focus.clone();
        self.items.get(ix.row).map(|item| {
            let selected_id = item.id;
            ListItem::new(ix)
                .h_32()
                .overflow_hidden()
                .flex()
                .flex_col()
                .justify_between()
                .p_2()
                .border_b(px(1.0))
                .border_color(border_color)
                .child(Label::new(item.name.clone()))
                .child(
                    v_flex()
                        .child(
                            Label::new(item.created_at.date_naive().to_string())
                                .text_color(date_color)
                                .text_size(size),
                        )
                        .when(!item.tags.is_empty(), |column| {
                            column.child(
                                h_flex().gap_1().overflow_hidden().children(
                                    item.tags.iter().map(|tag| tag_chip(tag.clone(), theme)),
                                ),
                            )
                        })
                        .child(
                            Label::new(preview_content(item.content.clone()))
                                .text_color(muted_color),
                        ),
                )
                .on_click(move |_event, window, cx| {
                    focus.focus(window, cx);
                    let select_item_action = SelectItem { selected_id };
                    window.dispatch_action(Box::new(select_item_action), cx);
                })
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

impl ListView {
    fn selected_tags(&self, cx: &App) -> Option<(Uuid, Vec<SharedString>)> {
        let item_id = self.entities.selected_id_state.read(cx).selected_id?;
        let item = ItemStore::get(cx).items.get(&item_id)?;
        Some((item_id, item.tags.clone()))
    }

    fn tag_suggestions(&self, cx: &App) -> Vec<SharedString> {
        let Some((_, own_tags)) = self.selected_tags(cx) else {
            return Vec::new();
        };
        let fragment = self
            .entities
            .tags_input_state
            .read(cx)
            .value()
            .trim()
            .to_lowercase();
        let mut suggestions: Vec<SharedString> = ItemStore::get(cx)
            .items
            .values()
            .flat_map(|item| item.tags.iter())
            .filter(|tag| !own_tags.contains(tag))
            .filter(|tag| tag.to_lowercase().contains(&fragment))
            .cloned()
            .collect();
        suggestions.sort();
        suggestions.dedup();
        suggestions
    }

    fn apply_tags(&mut self, item_id: Uuid, tags: Vec<SharedString>, cx: &mut Context<Self>) {
        let changed =
            cx.update_global::<ItemStore, bool>(|store, _cx| match store.items.get_mut(&item_id) {
                Some(item) if item.tags != tags => {
                    item.tags = tags.clone();
                    true
                }
                _ => false,
            });
        if changed {
            Self::persist_tags(cx, item_id, &tags);
        }
        cx.notify();
    }

    fn persist_tags(cx: &mut Context<Self>, item_id: Uuid, tags: &[SharedString]) {
        let repository = cx.global::<TagsRepository>().0.clone();
        let tags_to_save: Vec<String> = tags.iter().map(|tag| tag.to_string()).collect();
        cx.update_global::<PersistencePerNote, ()>(|persistence_per_note, _cx| {
            persistence_per_note
                .0
                .insert((item_id, Field::Tags), PersistenceStatus::Saving);
        });
        cx.spawn(async move |_this, cx| {
            let result = repository.update_tags(item_id, tags_to_save).await;
            cx.update(|cx| match result {
                Ok(()) => {
                    cx.update_global::<PersistencePerNote, ()>(|persistence, _cx| {
                        persistence
                            .0
                            .insert((item_id, Field::Tags), PersistenceStatus::Idle);
                    });
                    cx.update_global::<AppStatus, ()>(|app_status, _cx| {
                        app_status
                            .issues
                            .remove(&AppOperation::UpdateItem(Field::Tags));
                    });
                }
                Err(error) => {
                    cx.update_global::<PersistencePerNote, ()>(|persistence, _cx| {
                        persistence
                            .0
                            .insert((item_id, Field::Tags), PersistenceStatus::Failed);
                    });
                    cx.update_global::<AppStatus, ()>(|app_status, _cx| {
                        app_status.issues.insert(
                            AppOperation::UpdateItem(Field::Tags),
                            Issue::new(error.to_string()),
                        );
                    });
                }
            });
        })
        .detach();
    }

    fn commit_tag_text(&mut self, text: &str, remove_if_empty: bool, cx: &mut Context<Self>) {
        let editing = self.editing_tag.take();
        self.highlighted_suggestion = None;
        let Some((item_id, mut tags)) = self.selected_tags(cx) else {
            return;
        };
        let text = text.trim();
        let editing = editing.filter(|index| *index < tags.len());
        match (editing, text.is_empty()) {
            (Some(index), true) if remove_if_empty => {
                tags.remove(index);
            }
            (_, true) => {}
            (Some(index), false) => {
                let duplicate = tags
                    .iter()
                    .enumerate()
                    .any(|(other, tag)| other != index && tag.as_ref() == text);
                if duplicate {
                    tags.remove(index);
                } else {
                    tags[index] = SharedString::from(text.to_string());
                }
            }
            (None, false) => {
                if !tags.iter().any(|tag| tag.as_ref() == text) {
                    tags.push(SharedString::from(text.to_string()));
                }
            }
        }
        self.apply_tags(item_id, tags, cx);
    }

    fn clear_tags_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.entities.tags_input_state.update(cx, |state, cx| {
            state.set_value("", window, cx);
        });
    }

    fn commit_tags_input(&mut self, explicit: bool, window: &mut Window, cx: &mut Context<Self>) {
        if explicit {
            let suggestions = self.tag_suggestions(cx);
            if let Some(suggestion) = self
                .highlighted_suggestion
                .and_then(|index| suggestions.get(index))
            {
                self.pick_suggestion(suggestion.clone(), window, cx);
                return;
            }
        }
        let text = self.entities.tags_input_state.read(cx).value().to_string();
        self.commit_tag_text(&text, explicit, cx);
        if !text.is_empty() {
            self.clear_tags_input(window, cx);
        }
        cx.notify();
    }

    fn commit_comma_separated(
        &mut self,
        tags_input_state: &Entity<InputState>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.highlighted_suggestion = None;
        self.suggestions_dismissed = false;
        let value = tags_input_state.read(cx).value().to_string();
        if !value.contains(',') {
            cx.notify();
            return;
        }
        let mut segments: Vec<&str> = value.split(',').collect();
        let remainder = segments.pop().unwrap_or_default().trim_start().to_string();
        for segment in segments {
            self.commit_tag_text(segment, false, cx);
        }
        tags_input_state.update(cx, |state, cx| {
            state.set_value(remainder, window, cx);
        });
    }

    fn pick_suggestion(&mut self, tag: SharedString, window: &mut Window, cx: &mut Context<Self>) {
        self.commit_tag_text(&tag, false, cx);
        self.clear_tags_input(window, cx);
        self.focus_tags_input(window, cx);
    }

    fn begin_edit_tag(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        let Some((_, tags)) = self.selected_tags(cx) else {
            return;
        };
        let Some(tag) = tags.get(index) else {
            return;
        };
        self.editing_tag = Some(index);
        self.highlighted_suggestion = None;
        let text = tag.clone();
        self.entities.tags_input_state.update(cx, |state, cx| {
            state.set_value(text, window, cx);
        });
        self.focus_tags_input(window, cx);
        cx.notify();
    }

    fn focus_tags_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        let focus_handle = self.entities.tags_input_state.read(cx).focus_handle(cx);
        focus_handle.focus(window, cx);
    }

    fn handle_tags_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let suggestions_count = self.tag_suggestions(cx).len();
        match event.keystroke.key.as_str() {
            "down" if suggestions_count > 0 => {
                self.highlighted_suggestion = Some(
                    self.highlighted_suggestion
                        .map_or(0, |index| (index + 1) % suggestions_count),
                );
                cx.notify();
            }
            "up" if suggestions_count > 0 => {
                self.highlighted_suggestion = Some(
                    self.highlighted_suggestion
                        .map_or(suggestions_count - 1, |index| {
                            (index + suggestions_count - 1) % suggestions_count
                        }),
                );
                cx.notify();
            }
            "escape" => {
                self.suggestions_dismissed = true;
                self.editing_tag = None;
                self.highlighted_suggestion = None;
                self.clear_tags_input(window, cx);
                cx.notify();
            }
            "backspace" => {
                let input_is_empty = self.entities.tags_input_state.read(cx).value().is_empty();
                if input_is_empty
                    && self.editing_tag.is_none()
                    && let Some((_, tags)) = self.selected_tags(cx)
                    && let Some(last_index) = tags.len().checked_sub(1)
                {
                    self.begin_edit_tag(last_index, window, cx);
                }
            }
            _ => {}
        }
    }

    fn render_tags_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let Some((_, tags)) = self.selected_tags(cx) else {
            return div().into_any_element();
        };
        let suggestions = self.tag_suggestions(cx);
        let input_focused = self
            .entities
            .tags_input_state
            .read(cx)
            .focus_handle(cx)
            .is_focused(window);
        let show_suggestions =
            input_focused && !self.suggestions_dismissed && !suggestions.is_empty();
        let theme = cx.theme();
        let (popover_background, border_color, highlight_color) =
            (theme.popover, theme.border, theme.accent);

        let dropdown = v_flex()
            .absolute()
            .top_full()
            .left_0()
            .mt_1()
            .overflow_hidden()
            .gap_1()
            .p_1()
            .rounded_md()
            .border_1()
            .border_color(border_color)
            .bg(popover_background)
            .shadow_md()
            .children(suggestions.iter().cloned().enumerate().map(|(index, tag)| {
                let is_highlighted = self.highlighted_suggestion == Some(index);
                div()
                    .id(SharedString::from(format!("tag-suggestion-{index}")))
                    .cursor(PointingHand)
                    .p_1()
                    .rounded_sm()
                    .when(is_highlighted, |element| element.bg(highlight_color))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener({
                            let tag = tag.clone();
                            move |this, _event, window, cx| {
                                this.pick_suggestion(tag.clone(), window, cx);
                            }
                        }),
                    )
                    .child(tag_chip(tag, cx.theme()))
            }));

        let tags_input = div()
            .relative()
            .w(px(160.0))
            .child(
                InputBase::new("tags-input")
                    .child(Input::new(&self.entities.tags_input_state))
                    .text_sm(),
            )
            .when(show_suggestions, |element| {
                element.child(deferred(dropdown).with_priority(1))
            })
            .into_any_element();

        let mut tags_input = Some(tags_input);
        let mut elements: Vec<AnyElement> = Vec::new();
        for (index, tag) in tags.iter().enumerate() {
            if self.editing_tag == Some(index) {
                elements.extend(tags_input.take());
                continue;
            }
            elements.push(
                div()
                    .id(SharedString::from(format!("tag-{index}")))
                    .cursor(PointingHand)
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.begin_edit_tag(index, window, cx);
                    }))
                    .child(tag_chip(tag.clone(), cx.theme()))
                    .into_any_element(),
            );
        }
        elements.extend(tags_input);

        div()
            .flex()
            .flex_row()
            .flex_wrap()
            .items_center()
            .gap_1()
            .px_2()
            .pb_1()
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                this.handle_tags_key_down(event, window, cx);
            }))
            .children(elements)
            .into_any_element()
    }
}

impl Render for ListView {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl gpui::IntoElement {
        let tags_editor = self.render_tags_editor(window, cx);
        let theme = cx.theme();
        let border_color = theme.border;
        let muted_color = theme.muted_foreground;
        let focus = self.focus_handle.clone();
        let show_list = self.entities.show_list_state.read(cx);
        div()
            .track_focus(&self.focus_handle)
            .key_context("LIST")
            .on_action(cx.listener(move |this, _action: &NewItem, _window, cx| {
                let selected_id_state = this.entities.selected_id_state.clone();
                let repository = cx.global::<ItemRepository>().0.clone();
                cx.spawn(async move |_this, cx| {
                    match Self::add_empty_item(cx, repository, selected_id_state).await {
                        Ok(()) => {
                            cx.update_global::<AppStatus, ()>(|state, _cx| {
                                state.issues.remove(&AppOperation::CreateNewItem);
                            });
                        }
                        Err(err) => {
                            cx.update_global::<AppStatus, ()>(|state, _cx| {
                                let new_issue = Issue::new(err.to_string());
                                state.issues.insert(AppOperation::CreateNewItem, new_issue);
                            });
                            eprintln!("failed to add item: {err}");
                        }
                    }
                })
                .detach();
            }))
            .on_action(cx.listener(move |this, _action: &DeleteItem, _window, cx| {
                let selected_id_state = this.entities.selected_id_state.clone();
                let delete_item_repository = cx.global::<DeleteItemRepository>().0.clone();
                cx.spawn(async move |_this, cx| {
                    if let Err(err) =
                        Self::delete_item(cx, delete_item_repository, selected_id_state.clone())
                            .await
                    {
                        cx.update_global::<AppStatus, ()>(|state, cx| {
                            let new_issue = Issue::new(err.to_string());
                            let selected_id =
                                selected_id_state.read(cx).selected_id.unwrap_or_default();
                            state
                                .issues
                                .insert(AppOperation::DeleteItem(selected_id), new_issue);
                        });
                        cx.refresh();
                    } else {
                        // remove issue related to this note deletion if any.
                        cx.update_global::<AppStatus, ()>(|state, cx| {
                            let selected_id =
                                selected_id_state.read(cx).selected_id.unwrap_or_default();
                            state.issues.remove(&AppOperation::DeleteItem(selected_id));
                        });

                        // select the next available id.
                        cx.update(|cx| {
                            selected_id_state.update(cx, move |selected_id, cx| {
                                let store = ItemStore::global(cx);
                                *selected_id = SelectedIdState {
                                    selected_id: store.items().iter().next().map(|(id, _)| *id),
                                };
                                cx.notify();
                            });
                        });
                    }
                })
                .detach();
            }))
            .w_full()
            .h_full()
            .flex_row()
            .flex()
            .gap_1()
            .child(
                div()
                    .w_1_5()
                    .border(px(1.0))
                    .border_color(border_color)
                    .rounded_lg()
                    .flex_none()
                    .flex()
                    .flex_col()
                    .when(!*show_list, |el| el.hidden())
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .flex_row()
                            .justify_center()
                            .items_center()
                            //  .border_b(px(1.0))
                            // .border_color(border_color)
                            .p_2()
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .items_center()
                                    .justify_center()
                                    .gap(px(5.0))
                                    .w_full()
                                    .child(
                                        div()
                                            .flex()
                                            .flex_row()
                                            .items_center()
                                            .justify_between()
                                            .gap_1()
                                            .w_full()
                                            .child(div())
                                            .child(Label::new(SharedString::from("All Items")))
                                            .child(
                                                Button::new("new-item")
                                                    .icon(
                                                        Icon::new(IconName::Pencil)
                                                            .small()
                                                            .text_color(muted_color),
                                                    )
                                                    .cursor(PointingHand)
                                                    .tooltip("New Item")
                                                    .on_click(move |_event, window, cx| {
                                                        focus.focus(window, cx);
                                                        window.dispatch_action(
                                                            Box::new(actions::NewItem),
                                                            cx,
                                                        );
                                                    })
                                                    .ghost(),
                                            ),
                                    )
                                    .child(
                                        gpui_component::input::Input::new(
                                            &self.entities.input_state,
                                        )
                                        .prefix(
                                            Icon::new(IconName::Search)
                                                .small()
                                                .text_color(muted_color),
                                        ),
                                    ),
                            ),
                    )
                    .child(
                        List::new(&self.entities.list_state)
                            .flex()
                            .flex_col()
                            .gap_5(),
                    )
                    .context_menu(|menu, _window, _cx| {
                        menu.menu("Delete Item", Box::new(DeleteItem))
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .min_h_0()
                    .overflow_hidden()
                    .child(
                        InputBase::new("title-input")
                            .child(Input::new(&self.entities.title_input_state))
                            .p_2()
                            .font_weight(FontWeight::BOLD)
                            .text_xl()
                            .text_color(cx.theme().primary)
                            .border_color(cx.theme().border),
                    )
                    .child(tags_editor)
                    .child(div().flex_1().min_h_0().child(self.entities.editor.clone())),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::actions::{NewItem, SelectItem};
    use crate::repository::{ItemRepository, TagsRepository, TitleRepository};
    use crate::store::ItemStore;
    use crate::testutils::setup;
    use async_trait::async_trait;
    use chrono::Utc;
    use gpui::Focusable;
    use gpui::{SharedString, TestAppContext};
    use oden_core::entities::item;
    use oden_core::errors::UpdateItemError;
    use oden_core::repository::{ItemRepositoryTrait, TagsRepositoryTrait, TitleRepositoryTrait};
    use sea_orm::DbErr;
    use serde_json::json;
    use tokio::sync::watch;
    use uuid::Uuid;

    struct MockItemRepository;

    #[async_trait]
    impl ItemRepositoryTrait for MockItemRepository {
        async fn find_all(&self) -> Result<Vec<item::Model>, DbErr> {
            Ok(vec![])
        }

        async fn create_item(&self) -> Result<item::Model, DbErr> {
            let now = Utc::now();
            Ok(item::Model {
                id: Uuid::from_u128(1),
                name: "Untitled".to_string(),
                content: "# Untitled".to_string(),
                kind: item::ItemKind::Note,
                tags: json!([]),
                language: None,
                created_at: now,
                modified_at: now,
            })
        }
    }

    #[async_trait]
    impl TitleRepositoryTrait for MockItemRepository {
        async fn update_title(&self, _id: Uuid, _title: String) -> Result<(), UpdateItemError> {
            Ok(())
        }
    }

    #[async_trait]
    impl TagsRepositoryTrait for MockItemRepository {
        async fn update_tags(&self, _id: Uuid, _tags: Vec<String>) -> Result<(), UpdateItemError> {
            Ok(())
        }
    }

    #[gpui::test]
    fn test_tags_input_commit_edit_and_remove(cx: &mut TestAppContext) {
        let (window, _app_mode_state, _selected_id_state) = setup(cx);
        cx.update(|cx| {
            cx.set_global(TagsRepository(Arc::new(MockItemRepository)));
        });
        let target_id = cx.update(|cx| {
            ItemStore::get(cx)
                .items
                .keys()
                .next()
                .copied()
                .expect("item store should contain one item")
        });
        cx.update(|cx| {
            if let Some(item) = ItemStore::get_mut(cx).items.get_mut(&target_id) {
                item.tags.clear();
            }
        });
        window
            .update(cx, |root, window, cx| {
                root.focus.focus(window, cx);
                window.dispatch_action(
                    Box::new(SelectItem {
                        selected_id: target_id,
                    }),
                    cx,
                );
            })
            .unwrap();
        cx.run_until_parked();

        let type_tag = |cx: &mut TestAppContext, text: &str| {
            window
                .update(cx, |root, window, cx| {
                    let tags_input_state =
                        root.list_view.read(cx).entities.tags_input_state.clone();
                    let focus_handle = tags_input_state.read(cx).focus_handle(cx);
                    focus_handle.focus(window, cx);
                    tags_input_state.update(cx, |state, cx| {
                        state.replace_all(text, window, cx);
                    });
                })
                .unwrap();
            cx.dispatch_keystroke(
                *window,
                gpui::Keystroke::parse("enter").expect("valid keystroke"),
            );
            cx.run_until_parked();
        };
        let stored_tags = |cx: &mut TestAppContext| -> Vec<String> {
            cx.update(|cx| {
                ItemStore::get(cx)
                    .items
                    .get(&target_id)
                    .map(|item| item.tags.iter().map(|tag| tag.to_string()).collect())
                    .unwrap_or_default()
            })
        };

        type_tag(cx, "alpha");
        type_tag(cx, "beta");
        assert_eq!(stored_tags(cx), vec!["alpha", "beta"]);

        window
            .update(cx, |root, window, cx| {
                root.list_view.update(cx, |view, cx| {
                    view.begin_edit_tag(0, window, cx);
                });
            })
            .unwrap();
        type_tag(cx, "gamma");
        assert_eq!(stored_tags(cx), vec!["gamma", "beta"]);

        window
            .update(cx, |root, window, cx| {
                root.list_view.update(cx, |view, cx| {
                    view.begin_edit_tag(1, window, cx);
                });
            })
            .unwrap();
        type_tag(cx, "");
        assert_eq!(stored_tags(cx), vec!["gamma"]);

        window
            .update(cx, |root, window, cx| {
                root.list_view.update(cx, |view, cx| {
                    view.suggestions_dismissed = false;
                    view.focus_tags_input(window, cx);
                });
            })
            .unwrap();
        cx.dispatch_keystroke(
            *window,
            gpui::Keystroke::parse("escape").expect("valid keystroke"),
        );
        cx.run_until_parked();
        let dismissed = window
            .update(cx, |root, _window, cx| {
                root.list_view.read(cx).suggestions_dismissed
            })
            .unwrap();
        assert!(dismissed);
    }

    #[gpui::test]
    fn test_list_items_navigation(cx: &mut TestAppContext) {
        let (window, _app_mode_state, selected_id_state) = setup(cx);
        cx.update(|cx| {
            let repository = Arc::new(MockItemRepository);
            cx.set_global(ItemRepository(repository.clone()));
        });
        let uuid = Uuid::new_v4();
        window
            .update(cx, |root, window, cx| {
                root.focus.focus(window, cx);
                window.dispatch_action(Box::new(SelectItem { selected_id: uuid }), cx);
            })
            .unwrap();
        cx.update(|cx| {
            assert_eq!(
                selected_id_state.read(cx).selected_id.unwrap(),
                uuid,
                "id selection failed"
            )
        })
    }

    #[gpui::test]
    fn test_new_item_creation(cx: &mut TestAppContext) {
        let (window, _app_mode_state, selected_id_state) = setup(cx);
        cx.update(|cx| {
            let repository = Arc::new(MockItemRepository);
            cx.set_global(ItemRepository(repository));
        });
        window
            .update(cx, |root, window, cx| {
                let focus_handle = { root.list_view.read(cx).focus_handle.clone() };
                focus_handle.focus(window, cx);
                window.dispatch_action(Box::new(NewItem), cx);
            })
            .unwrap();
        // wait for async tasks to finish.
        cx.run_until_parked();

        cx.update(|cx| {
            let id = selected_id_state
                .read(cx)
                .selected_id
                .expect("expected selected_id to be set after NewItem");
            assert_eq!(id, Uuid::from_u128(1));
        })
    }

    #[gpui::test]
    fn test_selected_id_subscription(cx: &mut TestAppContext) {
        let (window, _app_mode_state, selected_id_state) = setup(cx);
        cx.update(|cx| {
            let repository = Arc::new(MockItemRepository);
            cx.set_global(ItemRepository(repository));
        });
        let target_id = cx.update(|cx| {
            ItemStore::get(cx)
                .items()
                .keys()
                .next()
                .copied()
                .expect("item store should contain at least one item")
        });
        window
            .update(cx, |_root, _window, cx| {
                selected_id_state.update(cx, |state, cx| {
                    state.selected_id = Some(target_id);
                    cx.notify();
                })
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |root, _window, cx| {
                let list_state = root.list_view.read(cx).entities.list_state.read(cx);
                let index_path = list_state
                    .selected_index()
                    .expect("index path should be present");
                let selected_id = list_state.delegate().items[index_path.row].id;
                // assert that the selected_id in the list (that actually shows up as selected in
                // the UI) is the id that we updated the selected_id_state with.
                assert_eq!(selected_id, target_id);
            })
            .unwrap();
    }
    #[gpui::test]
    fn test_title_update_on_selected_id_change(cx: &mut TestAppContext) {
        let (window, _app_mode_state, _selected_id_state) = setup(cx);
        cx.update(|cx| {
            let repository = Arc::new(MockItemRepository);

            cx.set_global(TitleRepository(repository.clone()));

            let target_id = ItemStore::get(cx)
                .items
                .keys()
                .next()
                .copied()
                .expect("item store should contain one item");
            window
                .update(cx, |root, window, cx| {
                    root.focus.focus(window, cx);
                    window.dispatch_action(
                        Box::new(SelectItem {
                            selected_id: target_id,
                        }),
                        cx,
                    );
                })
                .unwrap();
        });
        cx.run_until_parked();
        window
            .update(cx, move |root, _window, cx| {
                let actual_value = root
                    .list_view
                    .read(cx)
                    .entities
                    .title_input_state
                    .read(cx)
                    .value();
                let item = ItemStore::get(cx)
                    .items
                    .values()
                    .next()
                    .cloned()
                    .expect("item store should contain one item");
                assert_eq!(item.name, actual_value);
            })
            .unwrap();
    }

    #[gpui::test]
    fn test_title_input_updates_selected_item_store(cx: &mut TestAppContext) {
        let (window, _app_mode_state, _selected_id_state) = setup(cx);
        cx.update(|cx| {
            let repository = Arc::new(MockItemRepository);
            cx.set_global(TitleRepository(repository));
        });
        let target_id = cx.update(|cx| {
            ItemStore::get(cx)
                .items
                .keys()
                .next()
                .copied()
                .expect("item store should contain one item")
        });
        let (title_tx, _title_rx) = watch::channel(SharedString::from(""));
        cx.update(|cx| {
            ItemStore::get_mut(cx)
                .title_input_tx
                .insert(target_id, title_tx);
        });
        window
            .update(cx, |root, window, cx| {
                root.focus.focus(window, cx);
                window.dispatch_action(
                    Box::new(SelectItem {
                        selected_id: target_id,
                    }),
                    cx,
                );
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |root, window, cx| {
                let title_input_state = root.list_view.read(cx).entities.title_input_state.clone();
                title_input_state.update(cx, |state, cx| {
                    state.replace_all("Updated title", window, cx);
                });
            })
            .unwrap();
        cx.run_until_parked();
        cx.update(|cx| {
            assert_eq!(
                ItemStore::get(cx)
                    .items
                    .get(&target_id)
                    .expect("selected item should remain in the item store")
                    .name,
                "Updated title"
            );
        });
    }
}
