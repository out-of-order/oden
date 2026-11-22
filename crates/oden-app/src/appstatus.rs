use std::collections::HashMap;

use chrono::{DateTime, Utc};
use gpui::{App, Global};
use uuid::Uuid;

pub(crate) struct AppStatus {
    pub(crate) issues: HashMap<AppOperation, Issue>,
}

impl Global for AppStatus {}

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
    Tags,
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

impl std::fmt::Display for AppOperation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AppOperation::CreateNewItem => write!(formatter, "create item"),
            AppOperation::UpdateItem(field) => {
                write!(formatter, "update {}", field.to_string().to_lowercase())
            }
            AppOperation::DeleteItem(item_id) => write!(formatter, "delete item {item_id}"),
        }
    }
}

impl std::fmt::Display for Field {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Field::Title => write!(formatter, "Title"),
            Field::Content => write!(formatter, "Content"),
            Field::Tags => write!(formatter, "Tags"),
        }
    }
}

impl AppStatus {
    pub(crate) fn init(cx: &mut App) {
        cx.set_global(Self {
            issues: HashMap::new(),
        })
    }

    pub(crate) fn open_issue_count(&self) -> usize {
        self.issues
            .values()
            .filter(|issue| issue.issue_status == IssueStatus::Open)
            .count()
    }

    /// One line per open issue, oldest first.
    pub(crate) fn format_log(&self) -> String {
        let mut open_issues: Vec<(&AppOperation, &Issue)> = self
            .issues
            .iter()
            .filter(|(_, issue)| issue.issue_status == IssueStatus::Open)
            .collect();
        open_issues.sort_by_key(|(_, issue)| issue.created_at);
        open_issues
            .into_iter()
            .map(|(operation, issue)| {
                format!(
                    "[{}] {}: {}",
                    issue.created_at.format("%Y-%m-%d %H:%M:%S UTC"),
                    operation,
                    issue.message
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_log_lists_open_issues_oldest_first() {
        let mut status = AppStatus {
            issues: HashMap::new(),
        };
        let mut older = Issue::new("disk full".to_string());
        older.created_at = DateTime::from_timestamp(1_000, 0).expect("valid timestamp");
        let mut newer = Issue::new("locked".to_string());
        newer.created_at = DateTime::from_timestamp(2_000, 0).expect("valid timestamp");
        let mut dismissed = Issue::new("ignored".to_string());
        dismissed.issue_status = IssueStatus::Dismissed;
        status
            .issues
            .insert(AppOperation::UpdateItem(Field::Title), newer);
        status.issues.insert(AppOperation::CreateNewItem, older);
        status
            .issues
            .insert(AppOperation::UpdateItem(Field::Tags), dismissed);

        assert_eq!(
            status.format_log(),
            "[1970-01-01 00:16:40 UTC] create item: disk full\n[1970-01-01 00:33:20 UTC] update title: locked"
        );
    }
}
