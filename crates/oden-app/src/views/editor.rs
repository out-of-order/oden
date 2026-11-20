use std::cell::Cell;
use std::rc::Rc;

use crate::appstatus::Field;
use crate::completionprovider::LinkCompletionProvider;
use crate::inputvaluewatcher::InputPersistence;
use crate::models::Item;
use crate::repository::{ContentRepository, LinkRepository};
use crate::state::SelectedIdState;
use crate::store::ItemStore;
use comrak_gpui::{MermaidDiagrams, parse_links, render_document};
use gpui::{
    App, AppContext, Context, Entity, InteractiveElement, ParentElement, Pixels, Render,
    ScrollHandle, SharedString, StatefulInteractiveElement, Styled, Subscription, Window, canvas,
    div, point, px,
};
use gpui_component::ActiveTheme;
use gpui_component::input::{Editor, EditorState, InputEvent};
use gpui_component::scroll::ScrollableElement;
use uuid::Uuid;

pub struct EditorView {
    input_state: Entity<EditorState>,
    selected_id_state: Entity<SelectedIdState>,
    mermaid_diagrams: MermaidDiagrams,
    preview_scroll_handle: ScrollHandle,
    scroll_sync: Rc<ScrollSync>,
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
                    input_state.set_scroll_offset(point(px(0.), px(0.)), cx);
                });
                this.preview_scroll_handle.set_offset(point(px(0.), px(0.)));
                this.scroll_sync.reset();
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
            mermaid_diagrams: MermaidDiagrams::default(),
            preview_scroll_handle: ScrollHandle::new(),
            scroll_sync: Rc::new(ScrollSync::default()),
            _input_state_sub,
            _selected_id_state_sub,
        }
    }

    fn get_item_for_selected_id(cx: &mut Context<Self>, selected_id: Uuid) -> Option<Item> {
        ItemStore::get(cx).items().get(&selected_id).cloned()
    }
}

// The offsets (as positive distances) both panes had when the sync last
// ran. Whatever we write into the follower is stored here too, so the
// follower's change is not mistaken for the user scrolling it.
#[derive(Default)]
struct ScrollSync {
    editor_offset: Cell<Pixels>,
    preview_offset: Cell<Pixels>,
    editor_max: Cell<Pixels>,
    preview_max: Cell<Pixels>,
}

impl ScrollSync {
    fn reset(&self) {
        self.editor_offset.set(px(0.));
        self.preview_offset.set(px(0.));
        self.editor_max.set(px(0.));
        self.preview_max.set(px(0.));
    }
}

const SCROLL_TOLERANCE: Pixels = px(0.5);

fn moved(current: Pixels, last_seen: Pixels) -> bool {
    (current - last_seen).abs() > SCROLL_TOLERANCE
}

// Both panes are kept at the same fraction of their own scroll range, so
// their ends always line up. Called while painting, right after the editor,
// because the editor only knows its offsets once it has laid itself out.
// When both panes moved in the same frame the editor wins.
fn sync_scroll(
    input_state: &Entity<EditorState>,
    preview_scroll_handle: &ScrollHandle,
    scroll_sync: &ScrollSync,
    window: &mut Window,
    cx: &mut App,
) {
    let editor = input_state.read(cx);
    let editor_offset = -editor.scroll_offset().y;
    let editor_max = editor.max_scroll_offset().y;
    let preview_offset = -preview_scroll_handle.offset().y;
    let preview_max = preview_scroll_handle.max_offset().y;
    let editor_moved = moved(editor_offset, scroll_sync.editor_offset.get());
    let preview_moved = moved(preview_offset, scroll_sync.preview_offset.get());
    // A pane growing or shrinking (mermaid diagrams finishing, text edits)
    // changes what an offset means, so the preview is re-derived from the editor.
    let layout_changed = moved(editor_max, scroll_sync.editor_max.get())
        || moved(preview_max, scroll_sync.preview_max.get());
    if !editor_moved && !preview_moved && !layout_changed {
        return;
    }
    scroll_sync.editor_max.set(editor_max);
    scroll_sync.preview_max.set(preview_max);
    if editor_max <= px(0.) || preview_max <= px(0.) {
        scroll_sync.editor_offset.set(editor_offset);
        scroll_sync.preview_offset.set(preview_offset);
        return;
    }
    if editor_moved || layout_changed {
        let fraction = (editor_offset / editor_max).clamp(0., 1.);
        let target = preview_max * fraction;
        let current = preview_scroll_handle.offset();
        preview_scroll_handle.set_offset(point(current.x, -target));
        scroll_sync.editor_offset.set(editor_offset);
        scroll_sync.preview_offset.set(target);
    } else {
        let fraction = (preview_offset / preview_max).clamp(0., 1.);
        let target = editor_max * fraction;
        input_state.update(cx, |input_state, cx| {
            let current = input_state.scroll_offset();
            input_state.set_scroll_offset(point(current.x, -target), cx);
        });
        scroll_sync.preview_offset.set(preview_offset);
        scroll_sync.editor_offset.set(target);
    }
    window.refresh();
}

impl Render for EditorView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl gpui::IntoElement {
        let content = self.input_state.read(cx).value();
        let selected_id_state = self.selected_id_state.clone();
        let rendered_document = render_document(
            content.as_ref(),
            &mut self.mermaid_diagrams,
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
        );
        let input_state = self.input_state.clone();
        let preview_scroll_handle = self.preview_scroll_handle.clone();
        let scroll_sync = self.scroll_sync.clone();
        let theme = cx.theme();
        div()
            .flex()
            .flex_row()
            .size_full()
            .p_1()
            .gap_1()
            .overflow_hidden()
            .child(
                div()
                    .flex_1()
                    .w_0()
                    .min_w_0()
                    .overflow_hidden()
                    .rounded_lg()
                    .border(px(1.))
                    .border_color(theme.border)
                    .relative()
                    .child(
                        Editor::new(&self.input_state)
                            .h_full()
                            .border(px(0.))
                            .font_family("JetBrainsMono Nerd Font"),
                    )
                    .child(
                        canvas(
                            |_, _, _| {},
                            move |_, _, window, cx| {
                                sync_scroll(
                                    &input_state,
                                    &preview_scroll_handle,
                                    &scroll_sync,
                                    window,
                                    cx,
                                )
                            },
                        )
                        .absolute()
                        .size_full(),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .w_0()
                    .rounded_lg()
                    .border(px(1.))
                    .border_color(theme.border)
                    .min_w_0()
                    .overflow_hidden()
                    .child(
                        div()
                            .relative()
                            .size_full()
                            .child(
                                div()
                                    .id("markdown-preview")
                                    .size_full()
                                    .flex()
                                    .flex_col()
                                    .overflow_y_scroll()
                                    .track_scroll(&self.preview_scroll_handle)
                                    .child(
                                        // size_full on the document would otherwise cap
                                        // its height at the viewport and nothing would overflow.
                                        div()
                                            .flex_none()
                                            .h_auto()
                                            .min_h_full()
                                            .child(rendered_document),
                                    ),
                            )
                            .vertical_scrollbar(&self.preview_scroll_handle),
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

    fn select_item_with_content(
        cx: &mut gpui::TestAppContext,
        content: String,
    ) -> gpui::WindowHandle<crate::root::AppRoot> {
        let (window, _app_mode_state, _selected_id_state) = setup(cx);
        cx.update(|cx| cx.set_global(ItemRepository(Arc::new(MockItemRepository))));
        let selected_id = cx.update(|cx| {
            *ItemStore::get(cx)
                .items()
                .keys()
                .next()
                .expect("store should have at least one item in this test")
        });
        window
            .update(cx, |root, window, cx| {
                root.focus.focus(window, cx);
                window.dispatch_action(Box::new(SelectItem { selected_id }), cx);
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |root, window, cx| {
                root.list_view.read(cx).editor().update(cx, |editor, cx| {
                    editor.input_state.update(cx, |input_state, cx| {
                        input_state.set_value(content, window, cx);
                    });
                });
            })
            .unwrap();
        cx.run_until_parked();
        draw(cx, &window);
        window
    }

    fn draw(cx: &mut gpui::TestAppContext, window: &gpui::WindowHandle<crate::root::AppRoot>) {
        gpui::AppContext::update_window(cx, (*window).into(), |_root, window, cx| {
            window.draw(cx).clear(cx);
        })
        .unwrap();
    }

    #[gpui::test]
    fn test_preview_follows_editor_scroll_fraction(cx: &mut gpui::TestAppContext) {
        let content = (0..200)
            .map(|index| format!("paragraph {index}\n"))
            .collect::<Vec<_>>()
            .join("\n");
        let window = select_item_with_content(cx, content);
        window
            .update(cx, |root, _window, cx| {
                root.list_view.read(cx).editor().update(cx, |editor, cx| {
                    editor.input_state.update(cx, |input_state, cx| {
                        let halfway = input_state.max_scroll_offset().y * 0.5;
                        input_state.set_scroll_offset(gpui::point(gpui::px(0.), -halfway), cx);
                    });
                });
            })
            .unwrap();
        for _ in 0..3 {
            draw(cx, &window);
            cx.run_until_parked();
        }
        let (editor_fraction, preview_fraction) = window
            .update(cx, |root, _window, cx| {
                let editor = root.list_view.read(cx).editor().read(cx);
                let input_state = editor.input_state.read(cx);
                (
                    -input_state.scroll_offset().y / input_state.max_scroll_offset().y,
                    -editor.preview_scroll_handle.offset().y
                        / editor.preview_scroll_handle.max_offset().y,
                )
            })
            .unwrap();
        assert!(editor_fraction > 0.1, "editor should have scrolled");
        assert!((editor_fraction - preview_fraction).abs() < 0.01);

        window
            .update(cx, |root, _window, cx| {
                root.list_view.read(cx).editor().update(cx, |editor, cx| {
                    editor.input_state.update(cx, |input_state, cx| {
                        let end = input_state.max_scroll_offset().y;
                        input_state.set_scroll_offset(gpui::point(gpui::px(0.), -end), cx);
                    });
                });
            })
            .unwrap();
        for _ in 0..3 {
            draw(cx, &window);
            cx.run_until_parked();
        }
        let (preview_offset, preview_max) = window
            .update(cx, |root, _window, cx| {
                let handle = &root
                    .list_view
                    .read(cx)
                    .editor()
                    .read(cx)
                    .preview_scroll_handle;
                (-handle.offset().y, handle.max_offset().y)
            })
            .unwrap();
        assert!(
            (preview_max - preview_offset).abs() < gpui::px(1.),
            "both panes should reach their end together"
        );
    }

    #[gpui::test]
    fn test_editor_follows_preview_scroll_fraction(cx: &mut gpui::TestAppContext) {
        let content = (0..200)
            .map(|index| format!("paragraph {index}\n"))
            .collect::<Vec<_>>()
            .join("\n");
        let window = select_item_with_content(cx, content);
        window
            .update(cx, |root, _window, cx| {
                let editor = root.list_view.read(cx).editor().read(cx);
                let handle = &editor.preview_scroll_handle;
                let halfway = handle.max_offset().y * 0.5;
                handle.set_offset(gpui::point(gpui::px(0.), -halfway));
            })
            .unwrap();
        for _ in 0..5 {
            draw(cx, &window);
            cx.run_until_parked();
        }
        let (editor_fraction, preview_fraction) = window
            .update(cx, |root, _window, cx| {
                let editor = root.list_view.read(cx).editor().read(cx);
                let input_state = editor.input_state.read(cx);
                (
                    -input_state.scroll_offset().y / input_state.max_scroll_offset().y,
                    -editor.preview_scroll_handle.offset().y
                        / editor.preview_scroll_handle.max_offset().y,
                )
            })
            .unwrap();
        assert!(
            (preview_fraction - 0.5).abs() < 0.01,
            "preview must not be pushed back by the echo"
        );
        assert!((editor_fraction - preview_fraction).abs() < 0.01);
    }

    #[gpui::test]
    fn test_selecting_another_item_resets_both_panes(cx: &mut gpui::TestAppContext) {
        let content = (0..200)
            .map(|index| format!("paragraph {index}\n"))
            .collect::<Vec<_>>()
            .join("\n");
        let window = select_item_with_content(cx, content);
        window
            .update(cx, |root, _window, cx| {
                root.list_view.read(cx).editor().update(cx, |editor, cx| {
                    editor.input_state.update(cx, |input_state, cx| {
                        let halfway = input_state.max_scroll_offset().y * 0.5;
                        input_state.set_scroll_offset(gpui::point(gpui::px(0.), -halfway), cx);
                    });
                });
            })
            .unwrap();
        for _ in 0..3 {
            draw(cx, &window);
            cx.run_until_parked();
        }
        let current_id = window
            .update(cx, |root, _window, cx| {
                root.list_view
                    .read(cx)
                    .editor()
                    .read(cx)
                    .selected_id_state
                    .read(cx)
                    .selected_id
            })
            .unwrap();
        let other_id = cx.update(|cx| {
            *ItemStore::get(cx)
                .items()
                .keys()
                .find(|id| Some(**id) != current_id)
                .expect("store should have at least two items in this test")
        });
        window
            .update(cx, |_root, window, cx| {
                window.dispatch_action(
                    Box::new(SelectItem {
                        selected_id: other_id,
                    }),
                    cx,
                );
            })
            .unwrap();
        for _ in 0..3 {
            draw(cx, &window);
            cx.run_until_parked();
        }
        let (editor_offset, preview_offset) = window
            .update(cx, |root, _window, cx| {
                let editor = root.list_view.read(cx).editor().read(cx);
                (
                    editor.input_state.read(cx).scroll_offset().y,
                    editor.preview_scroll_handle.offset().y,
                )
            })
            .unwrap();
        assert_eq!(editor_offset, gpui::px(0.));
        assert_eq!(preview_offset, gpui::px(0.));
    }
}
