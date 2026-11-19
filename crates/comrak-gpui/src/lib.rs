#![recursion_limit = "256"]

use std::cell::RefCell;
use std::collections::HashMap;
use std::ops::Range;
use std::rc::Rc;

use comrak::arena_tree::Node;
use comrak::nodes::NodeValue::{
    self, BlockQuote, Code, CodeBlock, Document, Emph, Heading, Item, LineBreak, Link, List,
    Paragraph, Strong, TaskItem, Text,
};
use comrak::nodes::{Ast, ListType};
use comrak::{Arena, Options, parse_document};
use gpui::{
    AnyElement, App, ElementId, Font, FontFeatures, FontStyle, FontWeight, InteractiveText,
    IntoElement, ParentElement, SharedString, Styled, StyledText, TextRun, Window, div, px, rems,
};
use gpui_component::checkbox::Checkbox;
use gpui_component::highlighter::SyntaxHighlighter;
use gpui_component::{ActiveTheme, Rope, Theme};

mod inline_text;
mod mermaid;

use inline_text::CodeChipText;
pub use mermaid::MermaidDiagrams;
use mermaid::{MermaidBlocks, is_mermaid_info};

thread_local! {
    static CODE_HIGHLIGHTERS: RefCell<HashMap<String, SyntaxHighlighter>> = RefCell::new(HashMap::new());
}

#[derive(Clone, Default)]
struct TextStyle {
    weight: FontWeight,
    style: FontStyle,
    color: Option<gpui::Hsla>,
    background_color: Option<gpui::Hsla>,
    underline: Option<gpui::UnderlineStyle>,
    strikethrough: Option<gpui::StrikethroughStyle>,
}

#[derive(Copy, Clone)]
// a struct to store any context that needs to be
// passed between parent and child nodes.
// Used for BlockQuote, to change the text color
// of a paragraph inside a blockquote, and to hand out
// the mermaid diagram entities in document order.
struct RenderingContext<'a> {
    in_blockquote: bool,
    list_depth: usize,
    mermaid_blocks: &'a MermaidBlocks,
}

/// parses a markdown document using comrak and returns a
/// renderable gpui element
pub fn render_document(
    document: &str,
    mermaid_diagrams: &mut MermaidDiagrams,
    cx: &mut App,
    on_link_click: impl Fn(&str, &mut App, &mut Window) + 'static,
) -> AnyElement {
    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.tasklist = true;
    let root = parse_document(&arena, document, &options);
    let on_link_click = Rc::new(on_link_click);
    // descendants() is pre-order, the same order render_node visits code blocks in.
    let mermaid_sources: Vec<SharedString> = root
        .descendants()
        .filter_map(|node| match &node.data().value {
            CodeBlock(code_block) if is_mermaid_info(&code_block.info) => Some(SharedString::from(
                code_block.literal.trim_end().to_string(),
            )),
            _ => None,
        })
        .collect();
    let mermaid_blocks = mermaid_diagrams.sync(&mermaid_sources, cx);
    let render_cx = RenderingContext {
        in_blockquote: false,
        list_depth: 0,
        mermaid_blocks: &mermaid_blocks,
    };
    render_node(root, cx, render_cx, &on_link_click)
}

fn render_block_for_children<'a, F>(
    node: &'a Node<'a, RefCell<Ast>>,
    cx: &App,
    render_context: RenderingContext<'_>,
    on_link_click: &Rc<F>,
) -> Vec<AnyElement>
where
    F: Fn(&str, &mut App, &mut Window) + 'static,
{
    let children = node.children().collect::<Vec<_>>();
    node.children()
        .enumerate()
        .map(|(idx, child)| {
            let is_last = idx == children.len() - 1;
            let gap = if is_last { 0. } else { block_for_node(child) };
            div()
                .pb(rems(gap))
                .child(render_node(child, cx, render_context, on_link_click))
                .into_any_element()
        })
        .collect()
}

// applies semantic gaps between children based on their node type
fn block_for_node(node: &Node<'_, RefCell<Ast>>) -> f32 {
    match &node.data().value {
        NodeValue::Paragraph => 0.8, // rem
        NodeValue::Heading(_) => 0.4,
        NodeValue::CodeBlock(_) => 1.0,
        NodeValue::List(_) => 1.0,
        NodeValue::BlockQuote => 1.0,
        _ => 0.6,
    }
}

// list_depth is 1-based: the outermost list has depth 1.
fn bullet_color(list_depth: usize, theme: &Theme) -> gpui::Hsla {
    let palette = [
        theme.blue,
        theme.green,
        theme.yellow,
        theme.red,
        theme.magenta,
        theme.cyan,
    ];
    palette[list_depth.saturating_sub(1) % palette.len()]
}

fn render_bullet(list_depth: usize, theme: &Theme) -> impl IntoElement {
    let color = bullet_color(list_depth, theme);
    // even depths are rings, so levels stay distinguishable without relying on color alone.
    let circle = if list_depth.is_multiple_of(2) {
        div()
            .size(px(6.))
            .rounded_full()
            .border_1()
            .border_color(color)
    } else {
        div().size(px(6.)).rounded_full().bg(color)
    };
    // the container is as tall as a text line, so the circle centers on the first line
    // instead of sticking to the top of the row. flex_shrink_0 keeps it round when the text wraps.
    div()
        .flex()
        .items_center()
        .flex_shrink_0()
        .h(rems(1.5))
        .child(circle)
}

// recursively traverses the AST tree by using the children() method and maps
// each child into a gpui element
fn render_node<'a, F>(
    node: &'a Node<'a, RefCell<Ast>>,
    cx: &App,
    render_cx: RenderingContext<'_>,
    on_link_click: &Rc<F>,
) -> AnyElement
where
    F: Fn(&str, &mut App, &mut Window) + 'static,
{
    let theme = cx.theme();
    match &node.data().value {
        Document => div()
            .p_4()
            .gap_12()
            .size_full()
            .children(render_block_for_children(
                node,
                cx,
                render_cx,
                on_link_click,
            ))
            .into_any_element(),
        Heading(node_heading) => match node_heading.level {
            1 => div()
                .text_2xl()
                .border_b(px(1.))
                .border_color(theme.border)
                .text_color(cx.theme().primary)
                .font_weight(FontWeight::BOLD)
                .children(
                    node.children()
                        .map(|node| render_node(node, cx, render_cx, on_link_click))
                        .collect::<Vec<_>>(),
                )
                .into_any_element(),
            2 => div()
                .text_xl()
                .text_color(cx.theme().secondary_foreground)
                .font_weight(FontWeight::BOLD)
                .border_b(px(1.))
                .border_color(theme.border)
                .children(
                    node.children()
                        .map(|node| render_node(node, cx, render_cx, on_link_click))
                        .collect::<Vec<_>>(),
                )
                .into_any_element(),
            3 => div()
                .text_lg()
                .text_color(cx.theme().accent_foreground)
                .border_b(px(1.))
                .border_color(theme.border)
                .font_weight(FontWeight::BOLD)
                .children(
                    node.children()
                        .map(|node| render_node(node, cx, render_cx, on_link_click))
                        .collect::<Vec<_>>(),
                )
                .into_any_element(),
            _ => div()
                .children(
                    node.children()
                        .map(|node| render_node(node, cx, render_cx, on_link_click))
                        .collect::<Vec<_>>(),
                )
                .into_any_element(),
        },
        Paragraph => {
            let mut text = String::new();
            let mut runs = vec![];
            let mut links = vec![];
            let mut code_spans = vec![];
            let text_style = if render_cx.in_blockquote {
                TextStyle {
                    color: Some(theme.muted_foreground),
                    ..Default::default()
                }
            } else {
                TextStyle::default()
            };
            collect_segments(
                node,
                &mut text,
                &text_style,
                &mut runs,
                &mut links,
                &mut code_spans,
                theme,
            );
            if !code_spans.is_empty() {
                return CodeChipText::new(
                    text,
                    runs,
                    links,
                    code_spans,
                    theme.secondary,
                    theme.border,
                    theme.radius,
                    on_link_click.clone(),
                )
                .into_any_element();
            }
            let ranges: Vec<Range<usize>> = links.iter().map(|link| link.range.clone()).collect();
            let styled_text = StyledText::new(text).with_runs(runs);
            let on_link_click = on_link_click.clone();
            InteractiveText::new(
                ElementId::Name(SharedString::from("md-paragraph")),
                styled_text,
            )
            .on_click(ranges, move |index, window, cx| {
                let link = &links[index];
                on_link_click(&link.href, cx, window);
            })
            .into_any_element()
        }
        Text(cow) => {
            let text = cow.as_ref().to_string();
            div().child(text).into_any_element()
        }
        CodeBlock(code_block) if is_mermaid_info(&code_block.info) => {
            match render_cx.mermaid_blocks.take_next() {
                Some(diagram) => diagram.into_any_element(),
                None => div().into_any_element(),
            }
        }
        CodeBlock(code_block) => {
            let literal = &code_block.literal;
            let mut literal = literal.clone();
            let _ = literal.pop();
            let lang = code_block.info.split_whitespace().next().unwrap_or("");
            let styles = CODE_HIGHLIGHTERS.with(|cache| {
                let mut highlighter = cache.borrow_mut();
                let highlighter = highlighter
                    .entry(lang.to_string())
                    .or_insert_with(|| SyntaxHighlighter::new(lang));
                let text: &Rope = &Rope::from_str(&literal[..]);
                highlighter.update(None, text, None);
                highlighter.styles(&(0..literal.len()), cx.theme().highlight_theme.as_ref())
            });

            div()
                .border(px(0.5))
                .border_color(theme.colors.border)
                .rounded_lg()
                .bg(theme.secondary)
                .p_2()
                .mt_2()
                .mb_2()
                .flex()
                .flex_col()
                .justify_center()
                .child(
                    div()
                        .child(StyledText::new(literal.clone()).with_highlights(styles))
                        .font_family(theme.mono_font_family.clone())
                        .text_sm()
                        .text_color(theme.secondary_foreground),
                )
                .into_any_element()
        }
        BlockQuote => {
            let block_quote_render_cx = RenderingContext {
                in_blockquote: true,
                ..render_cx
            };
            div()
                .flex()
                .flex_col()
                .p_2()
                .border_l(px(4.))
                .border_color(theme.border)
                .bg(theme.muted)
                .children(
                    node.children()
                        .map(|node| render_node(node, cx, block_quote_render_cx, on_link_click))
                        .collect::<Vec<_>>(),
                )
                .into_any_element()
        }
        List(_) => {
            let list_render_cx = RenderingContext {
                list_depth: render_cx.list_depth + 1,
                ..render_cx
            };
            div()
                .flex()
                .flex_col()
                .children(
                    node.children()
                        .map(|node| render_node(node, cx, list_render_cx, on_link_click))
                        .collect::<Vec<_>>(),
                )
                .into_any_element()
        }
        Item(metadata) => {
            let symbol = match metadata.list_type {
                ListType::Bullet => render_bullet(render_cx.list_depth, theme).into_any_element(),
                ListType::Ordered => div()
                    .child(SharedString::from(format!("{}.", metadata.start)))
                    .into_any_element(),
            };
            div()
                .flex()
                .flex_row()
                .gap_2()
                .pl(px(metadata.padding as f32))
                .child(symbol)
                .child(
                    // text wrappers should have flex_1, so text wraps to available width.
                    div().flex_1().w_full().min_w_0().children(
                        node.children()
                            .map(|node| render_node(node, cx, render_cx, on_link_click))
                            .collect::<Vec<_>>(),
                    ),
                )
                .into_any_element()
        }
        TaskItem(metadata) => div()
            .flex()
            .flex_row()
            .items_start()
            .gap_2()
            .w_full()
            .child(Checkbox::new("list-item-checkbox").checked(metadata.symbol.is_some()))
            .child(
                div().flex_1().min_w_0().mt(-px(5.)).children(
                    node.children()
                        .map(|node| render_node(node, cx, render_cx, on_link_click))
                        .collect::<Vec<_>>(),
                ),
            )
            .into_any_element(),
        _ => div().into_any_element(),
    }
}

/// parses the links in the documents and returns them.
pub fn parse_links(document: &str) -> Vec<String> {
    let mut links = Vec::new();
    let options = Options::default();
    let arena = Arena::new();
    let root = parse_document(&arena, document, &options);
    for child in root.children() {
        if let Paragraph = child.data().value {
            for text_node in child.children() {
                if let Link(node_link) = &text_node.data().value {
                    links.push(node_link.url.clone());
                }
            }
        }
    }
    links
}

// collect text segments, register text runs, and return one StyledText block.
fn collect_segments<'a>(
    node: &'a Node<'a, RefCell<Ast>>,
    text: &mut String,
    style: &TextStyle,
    runs: &mut Vec<TextRun>,
    links: &mut Vec<LinkSpan>,
    code_spans: &mut Vec<Range<usize>>,
    theme: &Theme,
) {
    for child in node.children() {
        match &child.data().value {
            Text(cow) => {
                let s = cow.as_ref();
                text.push_str(s);
                runs.push(TextRun {
                    len: s.len(),
                    font: Font {
                        family: theme.font_family.clone(),
                        features: FontFeatures::default(),
                        fallbacks: None,
                        weight: style.weight,
                        style: style.style,
                    },
                    color: style.color.unwrap_or(theme.foreground),
                    background_color: style.background_color,
                    underline: style.underline,
                    strikethrough: style.strikethrough,
                });
            }
            LineBreak => {
                text.push('\n');
                runs.push(TextRun {
                    len: 1,
                    font: Font {
                        family: theme.font_family.clone(),
                        features: FontFeatures::default(),
                        fallbacks: None,
                        weight: style.weight,
                        style: style.style,
                    },
                    color: style.color.unwrap_or(theme.foreground),
                    background_color: style.background_color,
                    underline: style.underline,
                    strikethrough: style.strikethrough,
                });
            }
            Strong => {
                let mut child_style = style.clone();
                child_style.color = Some(theme.red);
                child_style.weight = FontWeight::BOLD;
                collect_segments(child, text, &child_style, runs, links, code_spans, theme);
            }
            Emph => {
                let mut child_style = style.clone();
                child_style.color = Some(theme.green);
                child_style.style = FontStyle::Italic;
                collect_segments(child, text, &child_style, runs, links, code_spans, theme);
            }
            Link(node_link) => {
                let mut child_style = style.clone();
                child_style.color = Some(theme.link);
                let start = text.len();
                collect_segments(child, text, &child_style, runs, links, code_spans, theme);
                let end = text.len();
                if end > start {
                    links.push(LinkSpan {
                        range: Range { start, end },
                        href: node_link.url.clone(),
                    });
                }
            }
            Code(node_code) => {
                let s = &node_code.literal;
                let start = text.len();
                text.push_str(s);
                let end = text.len();
                runs.push(TextRun {
                    len: s.len(),
                    font: Font {
                        family: theme.mono_font_family.clone(),
                        features: FontFeatures::default(),
                        fallbacks: None,
                        weight: style.weight,
                        style: style.style,
                    },
                    color: theme.primary,
                    background_color: None,
                    underline: style.underline,
                    strikethrough: style.strikethrough,
                });
                code_spans.push(Range { start, end });
            }
            _ => {}
        }
    }
}

#[derive(Clone)]
struct LinkSpan {
    range: Range<usize>,
    href: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bullet_color_changes_per_level_and_wraps() {
        let theme = Theme {
            colors: gpui_component::ThemeColor {
                blue: gpui::hsla(0.6, 1., 0.5, 1.),
                green: gpui::hsla(0.3, 1., 0.5, 1.),
                ..Default::default()
            },
            ..Default::default()
        };
        let palette_len = 6;
        assert_ne!(bullet_color(1, &theme), bullet_color(2, &theme));
        assert_eq!(
            bullet_color(1, &theme),
            bullet_color(1 + palette_len, &theme)
        );
    }
}
