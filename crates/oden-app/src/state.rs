#![allow(dead_code)]

use uuid::Uuid;

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum AppMode {
    Search,
    List,
    Graph,
    Settings,
}

pub struct SelectedIdState {
    pub selected_id: Option<Uuid>,
}

impl SelectedIdState {
    pub fn init() -> Self {
        Self { selected_id: None }
    }
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
