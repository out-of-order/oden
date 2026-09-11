use crate::appstatus::{AppOperation, Field, Issue};
use crate::store::ItemStore;
use gpui::{BorrowAppContext, Context, Result, SharedString};
use oden_core::errors::UpdateItemError;
use std::pin::Pin;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch::{self, Receiver};
use uuid::Uuid;

use crate::appstatus::AppStatus;
use crate::persistence::{PersistencePerNote, PersistenceStatus};

struct InputValueWatcher;

pub struct InputPersistence;

impl InputPersistence {
    /// Assumes persistence channel needs creation.
    /// Creates a tokio watch channel which applies debounce logic
    /// for the input before persisting to the database.
    /// Spawns a channel to receive errors and another to receive
    /// persistence state throughout.
    ///
    /// Args:
    /// - `item_id`: selected item id
    /// - `value`: updated value
    /// - `updated_field`: which field of the item is being updated
    /// - `save`: a closure which updates the field in the database
    pub fn spawn<T: 'static, F>(
        cx: &mut Context<T>,
        item_id: Uuid,
        value: SharedString,
        updated_field: Field,
        save: F,
    ) where
        F: Send
            + 'static
            + Fn(
                SharedString,
            )
                -> Pin<Box<dyn Future<Output = Result<(), UpdateItemError>> + Send + 'static>>,
    {
        let (tx, rx) = watch::channel(value.clone());
        cx.update_global::<ItemStore, ()>(|store, _cx| {
            match updated_field {
                Field::Title => store.title_input_tx.insert(item_id, tx),
                Field::Content => store.item_content_tx.insert(item_id, tx),
            };
        });
        let (error_tx, mut error_rx) = tokio::sync::mpsc::unbounded_channel::<UpdateItemError>();
        cx.spawn(async move |_this, cx| {
            while let Some(error_value) = error_rx.recv().await {
                cx.update(|cx| {
                    cx.update_global::<AppStatus, ()>(|app_status, _cx| {
                        app_status.issues.insert(
                            AppOperation::UpdateItem(updated_field),
                            Issue::new(error_value.to_string()),
                        );
                    })
                });
            }
        })
        .detach();
        let (persistence_tx, mut persistence_rx) =
            tokio::sync::mpsc::unbounded_channel::<PersistenceStatus>();
        cx.spawn(async move |_this, cx| {
            while let Some(persistence_value) = persistence_rx.recv().await {
                cx.update(|cx| {
                    cx.update_global::<PersistencePerNote, ()>(|persistence_per_note, _cx| {
                        persistence_per_note.0.insert(item_id, persistence_value);
                    })
                });
            }
        })
        .detach();
        InputValueWatcher::spawn(rx, error_tx, persistence_tx, save);
    }
}

impl InputValueWatcher {
    /// Applies a debounce logic to an input,
    /// Publishes eventual error to an error channel
    /// Published persistence state to a persistence channel.
    fn spawn<F>(
        mut rx: Receiver<SharedString>,
        error_tx: UnboundedSender<UpdateItemError>,
        persistence_state_tx: UnboundedSender<PersistenceStatus>,
        save: F,
    ) where
        F: Send
            + 'static
            + Fn(
                SharedString,
            )
                -> Pin<Box<dyn Future<Output = Result<(), UpdateItemError>> + Send + 'static>>,
    {
        tokio::spawn(async move {
            loop {
                if rx.changed().await.is_err() {
                    return;
                }
                Self::debounce(&mut rx).await;
                if persistence_state_tx
                    .send(PersistenceStatus::Saving)
                    .is_err()
                {
                    return;
                };
                let value = rx.borrow_and_update().clone();
                match save(value).await {
                    Ok(()) => {
                        if persistence_state_tx.send(PersistenceStatus::Idle).is_err() {
                            return;
                        };
                    }
                    Err(e) => {
                        if persistence_state_tx
                            .send(PersistenceStatus::Failed)
                            .is_err()
                            || error_tx.send(e).is_err()
                        {
                            return;
                        }
                    }
                }
            }
        });
    }

    async fn debounce(rx: &mut Receiver<SharedString>) {
        loop {
            tokio::select! {
                _ = tokio::time::sleep(Duration::from_millis(1000)) => break,
                changed = rx.changed() => {
                   if changed.is_err() {
                        return;
                   } else {
                        continue;
                   }
                }
            }
        }
    }
}
