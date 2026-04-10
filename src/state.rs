#![allow(dead_code)]

use uuid::Uuid;

pub struct AppState {
    pub mode: AppMode,
    pub selected_id: Option<Uuid>,
}

impl AppState {
    pub fn new(mode: AppMode, selected_id: Option<Uuid>) -> Self {
        Self { mode, selected_id }
    }

    pub fn init() -> Self {
        Self {
            mode: AppMode::List,
            selected_id: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum AppMode {
    Search,
    List,
    Graph,
    Settings,
}

impl std::fmt::Display for AppMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            AppMode::Search => "Search",
            AppMode::List => "List",
            AppMode::Graph => "Graph",
            AppMode::Settings => "Settings",
        };
        write!(f, "{}", s)
    }
}
