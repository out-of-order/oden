use std::rc::Rc;

use crate::appstatus::Field;
use crate::completionprovider::LinkCompletionProvider;
use crate::inputvaluewatcher::InputPersistence;
use crate::models::Item;
use crate::repository::{ContentRepository, LinkRepository};
use crate::state::SelectedIdState;
use crate::store::ItemStore;
use comrak_gpui::{parse_links, render_document};
use gpui::{
    AppContext, Context, Entity, ParentElement, Render, SharedString, Styled, Subscription, Window,
    div, px,
};
use gpui_component::ActiveTheme;
use gpui_component::input::{Editor, EditorState, InputEvent};
use gpui_component::scroll::ScrollableElement;
use uuid::Uuid;

pub struct EditorView {
    input_state: Entity<EditorState>,
    selected_id_state: Entity<SelectedIdState>,
    _selected_id_state_sub: Subscription,
    _input_state_sub: Subscription,
}

impl EditorView {
    pub fn new(
        cx: &mut Context<Self>,
        window: &mut Window,
        selected_id_state: Entity<SelectedIdState>,
    ) -> Self {
        let input_state = cx.new(|cx| {
            EditorState::new(window, cx)
                .language("markdown")
                .searchable(true)
                // this is essential so the layout does not break
                // if the text is too long.
                .soft_wrap(true)
                .line_number(true)
                .vim()
        });
        input_state.update(cx, |input_state, _cx| {
            input_state.lsp_mut().completion_provider = Some(Rc::new(LinkCompletionProvider {}))
        });
        let _selected_id_state_sub = cx.observe_in(
            &selected_id_state,
            window,
            move |this, selected_id_state, window, cx| {
                let selected_id_maybe = selected_id_state.read(cx).selected_id;
                let content = selected_id_maybe
                    .and_then(|selected_id| Self::get_item_for_selected_id(cx, selected_id))
                    .map(|item| item.content)
                    .unwrap_or_else(|| "".into());
                this.input_state.update(cx, |input_state, cx| {
                    input_state.set_value(content, window, cx);
                });
            },
        );
        let _input_state_sub = cx.subscribe_in(
            &input_state,
            window,
            move |view, input_state, event: &InputEvent, _window, cx| {
                if let InputEvent::Change = event {
                    let Some(selected_id) = view.selected_id_state.read(cx).selected_id else {
                        return;
                    };
                    let new_content = input_state.read(cx).value();
                    let store = ItemStore::get_mut(cx);
                    if let Some(item) = store.items.get_mut(&selected_id) {
                        item.content = new_content.clone();
                    }
                    let targets: Vec<Uuid> = parse_links(new_content.as_str())
                        .iter()
                        .filter_map(|link| {
                            link.strip_prefix("oden://item/")
                                .and_then(|id| Uuid::parse_str(id).ok())
                        })
                        .collect();
                    store.sync_links(selected_id, targets.clone());
                    let needs_new_receiver = match store.item_content_tx.get(&selected_id) {
                        Some(tx) => tx.send(new_content.clone()).is_err(),
                        None => true,
                    };
                    if needs_new_receiver {
                        let content_repository = cx.global::<ContentRepository>().0.clone();
                        let link_repository = cx.global::<LinkRepository>().0.clone();
                        InputPersistence::spawn(
                            cx,
                            selected_id,
                            new_content,
                            Field::Content,
                            move |content: SharedString| {
                                let content_repository = content_repository.clone();
                                let link_repository = link_repository.clone();

                                Box::pin(async move {
                                    content_repository
                                        .update_content(selected_id, content.to_string())
                                        .await?;
                                    let targets: Vec<Uuid> = parse_links(content.as_str())
                                        .iter()
                                        .filter_map(|link| {
                                            link.strip_prefix("oden://item/")
                                                .and_then(|id| Uuid::parse_str(id).ok())
                                        })
                                        .collect();
                                    link_repository.sync_links(selected_id, targets).await?;
                                    Ok(())
                                })
                            },
                        );
                    }
                }
            },
        );
        EditorView {
            input_state,
            selected_id_state,
            _input_state_sub,
            _selected_id_state_sub,
        }
    }

    fn get_item_for_selected_id(cx: &mut Context<Self>, selected_id: Uuid) -> Option<Item> {
        ItemStore::get(cx).items().get(&selected_id).cloned()
    }
}

impl Render for EditorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let content = self.input_state.read(cx).value();
        let theme = cx.theme();
        let selected_id_state = self.selected_id_state.clone();
        div()
            .flex()
            .flex_row()
            .size_full()
            .overflow_hidden()
            .child(
                div()
                    // 50% split when we set flex_1 with two children.
                    .flex_1()
                    // min_w_0 so width does not interfere with the split.
                    .w_0()
                    .min_w_0()
                    .overflow_hidden()
                    .border_r(px(1.))
                    .border_color(theme.border)
                    .child(
                        Editor::new(&self.input_state)
                            .h_full()
                            .border(px(0.))
                            .font_family("JetBrainsMono Nerd Font"),
                    ),
            )
            .child(
                div().flex_1().w_0().min_w_0().overflow_hidden().child(
                    div()
                        .size_full()
                        .overflow_y_scrollbar()
                        .child(render_document(
                            content.as_ref(),
                            cx,
                            move |href, cx, _window| {
                                if let Some(id) = href.strip_prefix("oden://item/") {
                                    if let Ok(selected_id) = Uuid::parse_str(id) {
                                        selected_id_state.update(cx, |state, cx| {
                                            if state.selected_id != Some(selected_id) {
                                                state.selected_id = Some(selected_id);
                                                cx.notify();
                                            }
                                        });
                                    }
                                } else {
                                    cx.open_url(href);
                                }
                            },
                        )),
                ),
            )
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::actions::SelectItem;
    use crate::repository::ItemRepository;
    use crate::store::ItemStore;
    use crate::testutils::setup;
    use async_trait::async_trait;
    use oden_core::entities::item;
    use oden_core::errors::UpdateItemError;
    use oden_core::repository::{ItemRepositoryTrait, TitleRepositoryTrait};
    use sea_orm::DbErr;
    use uuid::Uuid;

    struct MockItemRepository;

    #[async_trait]
    impl ItemRepositoryTrait for MockItemRepository {
        async fn find_all(&self) -> Result<Vec<item::Model>, DbErr> {
            Ok(vec![])
        }

        async fn create_item(&self) -> Result<item::Model, DbErr> {
            Err(DbErr::Custom("not used in this test".into()))
        }
    }

    #[async_trait]
    impl TitleRepositoryTrait for MockItemRepository {
        async fn update_title(&self, _id: Uuid, _title: String) -> Result<(), UpdateItemError> {
            Ok(())
        }
    }

    #[gpui::test]
    fn test_editor_updates_on_select(cx: &mut gpui::TestAppContext) {
        let (window, _app_mode_state, _selected_id_state) = setup(cx);
        cx.update(|cx| {
            let repository = Arc::new(MockItemRepository);
            cx.set_global(ItemRepository(repository));
        });
        let selected_id = cx.update(|cx| {
            ItemStore::get(cx)
                .items()
                .keys()
                .next()
                .copied()
                .expect("store should have at least one item in this test")
        });
        window
            .update(cx, |root, window, cx| {
                root.focus.focus(window, cx);
                window.dispatch_action(Box::new(SelectItem { selected_id }), cx);
            })
            .unwrap();
        let editor_text = window
            .update(cx, |root, _window, cx| {
                root.list_view
                    .read(cx)
                    .editor()
                    .read(cx)
                    .input_state
                    .read(cx)
                    .value()
            })
            .unwrap();
        let expected_content = cx.update(|cx| {
            ItemStore::get(cx)
                .items()
                .get(&selected_id)
                .unwrap()
                .content
                .clone()
        });
        assert!(editor_text == expected_content)
    }
}
