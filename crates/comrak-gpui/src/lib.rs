use std::cell::RefCell;
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
    AnyElement, App, Bounds, Corners, Element, ElementId, Font, FontFeatures, FontStyle,
    FontWeight, GlobalElementId, InspectorElementId, InteractiveText, IntoElement, LayoutId,
    ParentElement, Pixels, SharedString, Styled, StyledText, TextRun, Window, div, fill, px, rems,
};
use gpui_component::checkbox::Checkbox;
use gpui_component::{ActiveTheme, Theme};

#[derive(Clone, Default)]
struct TextStyle {
    weight: FontWeight,
    style: FontStyle,
    color: Option<gpui::Hsla>,
    background_color: Option<gpui::Hsla>,
    underline: Option<gpui::UnderlineStyle>,
    strikethrough: Option<gpui::StrikethroughStyle>,
}

#[derive(Default, Debug, Copy, Clone)]
// a struct to store any context that needs to be
// passed between parent and child nodes.
// Now only used for BlockQuote, to change the text color
// of a paragraph inside a blockquote.
struct RenderingContext {
    in_blockquote: bool,
}

/// parses a markdown document using comrak and returns a
/// renderable gpui element
pub fn render_document(
    document: &str,
    cx: &App,
    on_link_click: impl Fn(&str, &mut App, &mut Window) + 'static,
) -> AnyElement {
    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.tasklist = true;
    let root = parse_document(&arena, document, &options);
    let on_link_click = Rc::new(on_link_click);
    render_node(root, cx, RenderingContext::default(), &on_link_click)
}

fn render_block_for_children<'a, F>(
    node: &'a Node<'a, RefCell<Ast>>,
    cx: &App,
    render_context: RenderingContext,
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

// recursively traverses the AST tree by using the children() method and maps
// each child into a gpui element
fn render_node<'a, F>(
    node: &'a Node<'a, RefCell<Ast>>,
    cx: &App,
    render_cx: RenderingContext,
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
            let ranges: Vec<Range<usize>> = links.iter().map(|link| link.range.clone()).collect();
            let styled_text = StyledText::new(text).with_runs(runs);
            let text_layout = styled_text.layout().clone();
            let on_link_click = on_link_click.clone();
            let inner = InteractiveText::new(
                ElementId::Name(SharedString::from("md-paragraph")),
                styled_text,
            )
            .on_click(ranges, move |index, window, cx| {
                let link = &links[index];
                on_link_click(&link.href, cx, window);
            });
            CodeChipText {
                inner,
                text_layout,
                code_spans,
                chip_background: theme.secondary,
                chip_corner_radius: theme.radius,
                chip_padding_x: px(3.),
                chip_padding_y: px(1.),
            }
            .into_any_element()
        }
        Text(cow) => {
            let text = cow.as_ref().to_string();
            div().child(text).into_any_element()
        }
        CodeBlock(code_block) => {
            let literal = &code_block.literal;
            let mut literal = literal.clone();
            let _ = literal.pop();
            div()
                .border_r_2()
                .border_color(theme.border)
                .bg(theme.secondary)
                .p_2()
                .mt_2()
                .mb_2()
                .flex()
                .flex_col()
                .justify_center()
                .child(
                    div()
                        // TODO: syntax highlighting for code blocks.
                        .child(SharedString::from(literal.to_string()))
                        .font_family(theme.mono_font_family.clone())
                        .text_sm()
                        .text_color(theme.secondary_foreground),
                )
                .into_any_element()
        }
        BlockQuote => {
            let block_quote_render_cx = RenderingContext {
                in_blockquote: true,
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
        List(_) => div()
            .flex()
            .flex_col()
            .children(
                node.children()
                    .map(|node| render_node(node, cx, render_cx, on_link_click))
                    .collect::<Vec<_>>(),
            )
            .into_any_element(),
        Item(metadata) => {
            let symbol = match metadata.list_type {
                ListType::Bullet => SharedString::from("•"),
                ListType::Ordered => SharedString::from(format!("{}.", metadata.start)),
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

struct LinkSpan {
    range: Range<usize>,
    href: String,
}

struct CodeChipText {
    inner: InteractiveText,
    text_layout: gpui::TextLayout,
    code_spans: Vec<Range<usize>>,
    chip_background: gpui::Hsla,
    chip_corner_radius: Pixels,
    chip_padding_x: Pixels,
    chip_padding_y: Pixels,
}

impl CodeChipText {
    fn paint_chip(&self, span: &Range<usize>, window: &mut Window) {
        let Some(start) = self.text_layout.position_for_index(span.start) else {
            return;
        };
        let Some(end) = self.text_layout.position_for_index(span.end) else {
            return;
        };

        if start.y != end.y {
            return;
        }

        let line_height = self.text_layout.line_height();

        let bounds = Bounds::from_corners(
            gpui::point(start.x - self.chip_padding_x, start.y - self.chip_padding_y),
            gpui::point(
                end.x + self.chip_padding_x,
                start.y + line_height + self.chip_padding_y,
            ),
        );

        window.paint_quad(
            fill(bounds, self.chip_background).corner_radii(Corners::all(self.chip_corner_radius)),
        );
    }
}

impl Element for CodeChipText {
    type RequestLayoutState = <InteractiveText as Element>::RequestLayoutState;
    type PrepaintState = <InteractiveText as Element>::PrepaintState;

    fn id(&self) -> Option<ElementId> {
        self.inner.id()
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        self.inner.source_location()
    }

    fn request_layout(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        self.inner.request_layout(id, inspector_id, window, cx)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        self.inner
            .prepaint(id, inspector_id, bounds, request_layout, window, cx)
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        for span in &self.code_spans {
            self.paint_chip(span, window);
        }

        self.inner.paint(
            id,
            inspector_id,
            bounds,
            request_layout,
            prepaint,
            window,
            cx,
        );
    }
}

impl IntoElement for CodeChipText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
