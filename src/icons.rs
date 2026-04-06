use gpui_component::IconNamed;

#[allow(dead_code)]
pub enum IconName {
    Settings,
    Search,
    Graph,
    List,
}

impl IconNamed for IconName {
    fn path(self) -> gpui::SharedString {
        match self {
            IconName::Settings => "icons/settings.svg",
            IconName::Search => "icons/search.svg",
            IconName::Graph => "icons/graph.svg",
            IconName::List => "icons/list.svg",
        }
        .into()
    }
}
