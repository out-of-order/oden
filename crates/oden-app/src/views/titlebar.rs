use gpui::{
    AnyElement, Context, CursorStyle, Entity, FocusHandle, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, Window, div,
};
use gpui_component::{
    ActiveTheme, Icon, TitleBar,
    button::{Button, ButtonVariants},
    h_flex,
    label::Label,
    spinner::Spinner,
};

use crate::{
    actions::{IssueLogs, ToggleList},
    appstatus::AppStatus,
    icons::IconName,
    persistence::{
        PersistencePerNote,
        PersistenceStatus::{self, Failed, Idle, Saving},
    },
};

pub(crate) struct Titlebar {
    _status_entity_sub: Subscription,
    _persistence_sub: Subscription,
    persistence_status: PersistenceStatus,
    show_list: Entity<bool>,
    focus: FocusHandle,
}

impl Titlebar {
    pub(crate) fn new(
        cx: &mut Context<Self>,
        window: &mut Window,
        show_list: Entity<bool>,
        focus: FocusHandle,
    ) -> Self {
        let _status_entity_sub =
            cx.observe_global_in::<AppStatus>(window, |_status_entity, _window, cx| {
                cx.notify();
            });
        let _persistence_sub =
            cx.observe_global_in::<PersistencePerNote>(window, |_persistence, _window, cx| {
                cx.notify();
            });
        Self {
            _status_entity_sub,
            _persistence_sub,
            show_list,
            focus,
            persistence_status: PersistenceStatus::Idle,
        }
    }

    fn compute_persistence_status(&mut self, cx: &gpui::prelude::Context<Titlebar>) {
        let persistence_per_note = cx.global::<PersistencePerNote>();
        self.persistence_status = persistence_per_note
            .0
            .values()
            .copied()
            .reduce(|a, b| a.merge(&b))
            .unwrap_or(PersistenceStatus::Idle);
    }

    fn render_persistence(&mut self, cx: &gpui::prelude::Context<Titlebar>) -> AnyElement {
        let red = cx.theme().red_light;
        let muted = cx.theme().muted_foreground;
        self.compute_persistence_status(cx);
        match self.persistence_status {
            Idle => Label::new("synced")
                .text_sm()
                .text_color(muted)
                .into_any_element(),
            Saving => h_flex()
                .child(Spinner::new().color(muted))
                .child(Label::new("syncing").text_color(muted))
                .text_color(muted)
                .gap_1()
                .items_center()
                .into_any_element(),
            Failed => h_flex()
                .child(Icon::new(IconName::Close).text_color(red))
                .child(
                    div()
                        .child(Label::new("out of sync").text_color(muted))
                        .text_color(muted),
                )
                .gap_1()
                .items_center()
                .into_any_element(),
        }
    }
}

impl Render for Titlebar {
    fn render(
        &mut self,
        _window: &mut gpui::Window,
        cx: &mut gpui::prelude::Context<Self>,
    ) -> impl gpui::prelude::IntoElement {
        let muted = cx.theme().muted_foreground;
        let green = cx.theme().green_light;
        let red = cx.theme().red_light;
        let status_entity = cx.global::<AppStatus>();
        let total_issues_found = status_entity.open_issue_count();
        let issue_label = if total_issues_found == 1 {
            "issue"
        } else {
            "issues"
        };
        let status_message = format!("{} {}", total_issues_found, issue_label);
        let status_message = SharedString::from(status_message);
        let (icon_name, color) = if total_issues_found == 0 {
            (IconName::Check, green)
        } else {
            (IconName::Close, red)
        };
        let show_list_icon = if *self.show_list.read(cx) {
            IconName::ShowList
        } else {
            IconName::HideList
        };

        let focus = self.focus.clone();
        TitleBar::new()
            // render the status of the app.
            .pr_4()
            .bg(cx.theme().background)
            .border_0()
            .child(
                h_flex()
                    .gap_2()
                    .child(div().child(SharedString::from("oden")).text_sm())
                    .child(
                        Button::new("issues")
                            .cursor(CursorStyle::PointingHand)
                            .tooltip("check issue logs")
                            .on_click({
                                let focus = focus.clone();
                                move |_event, window, cx| {
                                    window.focus(&focus, cx);
                                    window.dispatch_action(Box::new(IssueLogs), cx);
                                }
                            })
                            .h_3_4()
                            .ghost()
                            .flex()
                            .flex_row()
                            .gap_2()
                            .items_center()
                            .child(Icon::new(icon_name).text_color(color))
                            .child(Label::new(status_message).text_color(muted)),
                    )
                    .child(
                        Button::new("toggle_list")
                            .h_3_4()
                            .ghost()
                            .tooltip_with_action("toggle item list", &ToggleList, Some("ROOT"))
                            .on_click(move |_event, window, cx| {
                                window.focus(&focus, cx);
                                window.dispatch_action(Box::new(ToggleList), cx);
                            })
                            .cursor(CursorStyle::PointingHand)
                            .child(Icon::new(show_list_icon).text_sm().text_color(muted)),
                    ),
            )
            .child(self.render_persistence(cx))
    }
}

#[cfg(test)]
mod tests {
    use crate::appstatus::Field;
    use chrono::Utc;
    use std::assert_matches;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use gpui::{BorrowAppContext, TestAppContext};
    use oden_core::repository::{ItemRepositoryTrait, TitleRepositoryTrait};
    use oden_core::{entities::item, errors::UpdateItemError};
    use sea_orm::DbErr;
    use uuid::Uuid;

    use crate::appstatus::AppStatus;
    use crate::persistence::{PersistencePerNote, PersistenceStatus};
    use crate::repository::{ItemRepository, TitleRepository};
    use crate::state::AppMode;
    use crate::{
        actions::{IssueLogs, NewItem},
        testutils::setup,
    };

    use async_trait::async_trait;

    pub struct FailingItemRepository {}

    #[async_trait]
    impl ItemRepositoryTrait for FailingItemRepository {
        async fn find_all(&self) -> Result<Vec<item::Model>, DbErr> {
            Ok(vec![])
        }

        async fn create_item(&self) -> Result<item::Model, DbErr> {
            Err(DbErr::Custom(
                "an error occurred when inserting an item".into(),
            ))
        }
    }

    #[async_trait]
    impl TitleRepositoryTrait for FailingItemRepository {
        async fn update_title(&self, _id: Uuid, _title: String) -> Result<(), UpdateItemError> {
            Err(UpdateItemError::NotFound)
        }
    }

    #[gpui::test]
    fn test_titlebar_status_change_on_issues(cx: &mut TestAppContext) {
        let (window, _app_mode_state, _selected_id_state) = setup(cx);
        cx.update(|cx| {
            let failing_repository = Arc::new(FailingItemRepository {});
            cx.set_global::<TitleRepository>(TitleRepository(failing_repository.clone()));
            cx.set_global::<ItemRepository>(ItemRepository(failing_repository.clone()));
        });
        window
            .update(cx, |root, window, cx| {
                let focus_handle = { root.list_view.read(cx).focus_handle.clone() };
                focus_handle.focus(window, cx);
                window.dispatch_action(Box::new(NewItem), cx);
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |_root, _window, cx| {
                let status_entity = cx.global::<AppStatus>();
                let issue = status_entity
                    .issues
                    .values()
                    .next()
                    .expect("one issue should have been created");
                assert_eq!(
                    issue.message,
                    "Custom Error: an error occurred when inserting an item"
                );
            })
            .unwrap();
    }

    #[gpui::test]
    fn test_issue_logs_show_open_issues(cx: &mut TestAppContext) {
        let (window, app_mode_state, _selected_id_state) = setup(cx);
        cx.update(|cx| {
            cx.set_global::<ItemRepository>(ItemRepository(Arc::new(FailingItemRepository {})));
        });
        window
            .update(cx, |root, window, cx| {
                let focus_handle = { root.list_view.read(cx).focus_handle.clone() };
                focus_handle.focus(window, cx);
                window.dispatch_action(Box::new(NewItem), cx);
            })
            .unwrap();
        cx.run_until_parked();
        window
            .update(cx, |root, window, cx| {
                root.focus.focus(window, cx);
                window.dispatch_action(Box::new(IssueLogs), cx);
            })
            .unwrap();
        cx.run_until_parked();

        cx.update(|cx| assert_eq!(*app_mode_state.read(cx), AppMode::IssueLogs));
        window
            .update(cx, |root, _window, cx| {
                let log = root.issue_logs_view.read(cx).input_state.read(cx).value();
                assert!(log.starts_with('['), "log should be timestamped: {log}");
                assert!(log.contains(
                    "create item: Custom Error: an error occurred when inserting an item"
                ));
            })
            .unwrap();
    }

    struct FlakyItemRepository {
        remaining_failures: AtomicUsize,
    }

    #[async_trait]
    impl ItemRepositoryTrait for FlakyItemRepository {
        async fn find_all(&self) -> Result<Vec<item::Model>, DbErr> {
            Ok(vec![])
        }

        async fn create_item(&self) -> Result<item::Model, DbErr> {
            if self
                .remaining_failures
                .try_update(Ordering::SeqCst, Ordering::SeqCst, |remaining| {
                    remaining.checked_sub(1)
                })
                .is_ok()
            {
                return Err(DbErr::Custom("database is locked".into()));
            }
            let now = Utc::now();
            Ok(item::Model {
                id: Uuid::from_u128(7),
                name: "".into(),
                content: "".into(),
                kind: item::ItemKind::Note,
                tags: serde_json::json!([]),
                language: None,
                created_at: now,
                modified_at: now,
            })
        }
    }

    #[gpui::test]
    fn test_issue_is_cleared_once_operation_succeeds(cx: &mut TestAppContext) {
        let (window, _app_mode_state, _selected_id_state) = setup(cx);
        cx.update(|cx| {
            cx.set_global::<ItemRepository>(ItemRepository(Arc::new(FlakyItemRepository {
                remaining_failures: AtomicUsize::new(1),
            })));
        });
        for expected_open_issues in [1, 0] {
            window
                .update(cx, |root, window, cx| {
                    let focus_handle = { root.list_view.read(cx).focus_handle.clone() };
                    focus_handle.focus(window, cx);
                    window.dispatch_action(Box::new(NewItem), cx);
                })
                .unwrap();
            cx.run_until_parked();
            cx.update(|cx| {
                assert_eq!(
                    cx.global::<AppStatus>().open_issue_count(),
                    expected_open_issues
                );
            });
        }
        window
            .update(cx, |root, _window, cx| {
                let log = root.issue_logs_view.read(cx).input_state.read(cx).value();
                assert_eq!(log.as_ref(), "No unresolved issues");
            })
            .unwrap();
    }

    #[gpui::test]
    fn test_titlebar_persistence_status_failed(cx: &mut TestAppContext) {
        let (window, _app_mode_state, _selected_id_state) = setup(cx);
        let item_id = Uuid::new_v4();
        cx.update(|cx| {
            cx.update_global::<PersistencePerNote, ()>(|persistence_per_note, _cx| {
                persistence_per_note
                    .0
                    .insert((item_id, Field::Content), PersistenceStatus::Failed);
            });
        });
        cx.run_until_parked();
        window
            .update(cx, |root, _window, cx| {
                assert_matches!(
                    root.titlebar.read(cx).persistence_status,
                    PersistenceStatus::Failed
                );
            })
            .unwrap();
    }
}
