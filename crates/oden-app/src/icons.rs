#![allow(dead_code)]

use gpui_component::IconNamed;

pub enum IconName {
    Settings,
    Search,
    Graph,
    List,
    Minimize,
    Restore,
    Close,
}

impl IconNamed for IconName {
    fn path(self) -> gpui::SharedString {
        match self {
            IconName::Settings => "icons/settings.svg",
            IconName::Search => "icons/search.svg",
            IconName::Graph => "icons/graph.svg",
            IconName::List => "icons/list.svg",
            IconName::Minimize => "icons/window-minimize.svg",
            IconName::Restore => "icons/window-restore.svg",
            IconName::Close => "icons/window-close.svg",
        }
        .into()
    }
}
