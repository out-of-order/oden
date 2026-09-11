use std::sync::Arc;
use std::time::Duration;

use gpui::SharedString;
use oden_core::errors::UpdateItemError;
use oden_core::repository::{ContentRepositoryTrait, TitleRepositoryTrait};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::watch::Receiver;
use uuid::Uuid;

use crate::persistence::PersistenceStatus;

pub struct InputValueWatcher;

impl InputValueWatcher {
    pub fn spawn_content_watcher(
        mut rx: Receiver<SharedString>,
        error_tx: UnboundedSender<UpdateItemError>,
        persistence_state_tx: UnboundedSender<PersistenceStatus>,
        id: Uuid,
        repository: Arc<dyn ContentRepositoryTrait + Send + Sync>,
    ) {
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
                let content = rx.borrow_and_update().clone();
                if let Err(e) = repository.update_content(id, content.to_string()).await {
                    if persistence_state_tx
                        .send(PersistenceStatus::Failed)
                        .is_err()
                        || error_tx.send(e).is_err()
                    {
                        return;
                    }
                } else {
                    if persistence_state_tx.send(PersistenceStatus::Idle).is_err() {
                        return;
                    };
                };
            }
        });
    }

    pub fn spawn_title_watcher(
        mut rx: Receiver<SharedString>,
        id: Uuid,
        repository: Arc<dyn TitleRepositoryTrait + Send + Sync>,
    ) {
        tokio::spawn(async move {
            loop {
                if rx.changed().await.is_err() {
                    return;
                }
                Self::debounce(&mut rx).await;
                let title = rx.borrow_and_update().clone();
                if repository
                    .update_title(id, title.to_string())
                    .await
                    .is_err()
                {};
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
