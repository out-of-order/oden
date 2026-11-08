use gpui::Action;
use gpui::actions;
use uuid::Uuid;

actions!(iconRail, [ListMode, SearchMode, GraphMode, Settings]);
actions!(listMode, [NewItem, DeleteItem]);
actions!(root, [ToggleList]);

#[derive(Clone, PartialEq, serde::Deserialize, schemars::JsonSchema, Action)]
#[action(namespace=listView)]
pub struct SelectItem {
    pub selected_id: Uuid,
}
