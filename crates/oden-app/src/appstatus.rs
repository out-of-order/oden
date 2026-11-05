use std::collections::HashMap;

use chrono::{DateTime, Utc};
use gpui::{App, Global};
use uuid::Uuid;

pub(crate) struct AppStatus {
    pub(crate) issues: HashMap<AppOperation, Issue>,
}

impl Global for AppStatus {}

#[allow(dead_code)]
pub(crate) struct Issue {
    pub(crate) message: String,
    pub(crate) issue_status: IssueStatus,
    pub(crate) created_at: DateTime<Utc>,
}

#[derive(Debug, Eq, Hash, PartialEq, PartialOrd)]
#[allow(warnings)]
pub(crate) enum AppOperation {
    CreateNewItem,
    UpdateItem(Field),
    DeleteItem(Uuid),
}

#[derive(Debug, Eq, Hash, PartialEq, PartialOrd, Copy, Clone)]
pub(crate) enum Field {
    Title,
    Content,
}

impl Issue {
    pub fn new(message: String) -> Self {
        let now = Utc::now();
        Self {
            message,
            created_at: now,
            issue_status: IssueStatus::Open,
        }
    }
}

#[derive(Debug, PartialEq, PartialOrd)]
#[allow(dead_code)]
pub(crate) enum IssueStatus {
    Open,
    Dismissed,
}

impl AppStatus {
    pub(crate) fn init(cx: &mut App) {
        cx.set_global(Self {
            issues: HashMap::new(),
        })
    }
}
