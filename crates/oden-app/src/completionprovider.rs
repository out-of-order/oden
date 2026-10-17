use gpui::{AppContext, Task};
use gpui_base::input::CompletionProvider;
use gpui_component::RopeExt;
use lsp_types::{
    CompletionContext, CompletionItem, CompletionResponse, Documentation, MarkupContent,
    MarkupKind, TextEdit,
};

use crate::store::ItemStore;

const CONTENT_PREVIEW_CHARS: usize = 200;

pub struct LinkCompletionProvider {}

impl CompletionProvider for LinkCompletionProvider {
    fn completions(
        &self,
        text: &gpui_component::Rope,
        offset: usize,
        trigger: CompletionContext,
        _window: &mut gpui::Window,
        cx: &mut gpui::App,
    ) -> gpui::Task<gpui::Result<CompletionResponse>> {
        let trigger_character = trigger.trigger_character.unwrap_or_default();
        if trigger_character.starts_with("[[") {
            let start_offset = offset.saturating_sub(trigger_character.len());
            let start = text.offset_to_position(start_offset);
            let end = text.offset_to_position(offset);
            let replace_range = lsp_types::Range::new(start, end);
            let store = cx.global::<ItemStore>();
            let search = trigger_character.trim_start_matches("[[");
            let completion_items = store
                .items
                .iter()
                .filter(|(_, item)| item.name.to_lowercase().contains(&search.to_lowercase()))
                .map(|(id, item)| {
                    let preview: String =
                        item.content.chars().take(CONTENT_PREVIEW_CHARS).collect();
                    let tags = item
                        .tags
                        .iter()
                        .map(|tag| tag.as_ref())
                        .collect::<Vec<&str>>()
                        .join(", ");
                    let mut documentation = preview;
                    if !tags.is_empty() {
                        documentation.push_str("\n\n_tags: ");
                        documentation.push_str(&tags);
                        documentation.push('_');
                    }
                    CompletionItem {
                        label: item.name.clone().into(),
                        detail: Some(format!("{:?}", item.kind)),
                        documentation: Some(Documentation::MarkupContent(MarkupContent {
                            kind: MarkupKind::Markdown,
                            value: documentation,
                        })),
                        filter_text: Some(search.to_string()),
                        text_edit: Some(lsp_types::CompletionTextEdit::Edit(TextEdit {
                            range: replace_range,
                            new_text: format!("[{}](oden://item/{})", item.name, id),
                        })),
                        ..Default::default()
                    }
                })
                .collect::<Vec<CompletionItem>>();
            return cx
                .background_spawn(async move { Ok(CompletionResponse::Array(completion_items)) });
        }
        Task::ready(Ok(CompletionResponse::Array(vec![])))
    }

    fn is_completion_trigger(&self, _offset: usize, _new_text: &str, _cx: &mut gpui::App) -> bool {
        true
    }
}
