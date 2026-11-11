use std::{cell::Cell, ops::Range, rc::Rc};

use gpui::{
    App, AvailableSpace, Bounds, Corners, CursorStyle, DispatchPhase, Element, ElementId,
    GlobalElementId, Hitbox, HitboxBehavior, Hsla, InspectorElementId, IntoElement, LayoutId,
    MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, ShapedLine, Size,
    TextAlign, TextRun, WhiteSpace, Window, WindowTextSystem, fill, point, px, size,
};
use unicode_segmentation::UnicodeSegmentation;

use crate::LinkSpan;

const CODE_FONT_SCALE: f32 = 0.875;
const CHIP_PADDING: f32 = 2.;
const CHIP_MARGIN: f32 = 2.;

type LinkClick = Rc<dyn Fn(&str, &mut App, &mut Window)>;

pub(crate) struct CodeChipText {
    text: String,
    runs: Vec<TextRun>,
    links: Rc<Vec<LinkSpan>>,
    code_spans: Vec<Range<usize>>,
    background: Hsla,
    border_color: Hsla,
    radius: Pixels,
    on_link_click: LinkClick,
}

#[derive(Clone)]
struct Fragment {
    range: Range<usize>,
    line: ShapedLine,
    runs: Vec<TextRun>,
    width: Pixels,
    code: Option<usize>,
    origin: Point<Pixels>,
}

struct Token {
    fragments: Vec<Fragment>,
    hard_break: bool,
}

pub(crate) struct LayoutState {
    tokens: Vec<Token>,
    line_height: Pixels,
    baseline: Pixels,
    wrap: bool,
}

pub(crate) struct PaintState {
    layout: Rc<InlineLayout>,
    hitbox: Hitbox,
}

#[derive(Default)]
struct LinkState {
    mouse_down: Rc<Cell<Option<usize>>>,
    hovered: Rc<Cell<Option<usize>>>,
}

#[derive(Default)]
struct InlineLayout {
    fragments: Vec<Fragment>,
    chips: Vec<Bounds<Pixels>>,
    size: Size<Pixels>,
}

impl CodeChipText {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        text: String,
        runs: Vec<TextRun>,
        links: Vec<LinkSpan>,
        code_spans: Vec<Range<usize>>,
        background: Hsla,
        border_color: Hsla,
        radius: Pixels,
        on_link_click: LinkClick,
    ) -> Self {
        Self {
            text,
            runs,
            links: Rc::new(links),
            code_spans,
            background,
            radius,
            border_color,
            on_link_click,
        }
    }

    fn shape(&self, window: &Window) -> LayoutState {
        let style = window.text_style();
        let font_size = style.font_size.to_pixels(window.rem_size());
        let line_height = window.pixel_snap(
            style
                .line_height
                .to_pixels(font_size.into(), window.rem_size()),
        );
        let body = window
            .text_system()
            .shape_line(" ".into(), font_size, &[style.to_run(1)], None);
        LayoutState {
            tokens: self.shape_tokens(window.text_system(), font_size),
            line_height,
            baseline: (line_height + body.ascent - body.descent) / 2.,
            wrap: style.white_space == WhiteSpace::Normal,
        }
    }

    fn shape_tokens(&self, text_system: &WindowTextSystem, font_size: Pixels) -> Vec<Token> {
        let mut tokens = Vec::new();
        let mut start = 0;
        for (end, _) in unicode_linebreak::linebreaks(&self.text) {
            let hard_break = self.text[start..end].ends_with('\n');
            let text_end = end - usize::from(hard_break);
            let mut fragments = Vec::new();
            let mut offset = start;
            while offset < text_end {
                let code = self
                    .code_spans
                    .iter()
                    .position(|span| span.contains(&offset));
                let fragment_end = if let Some(code) = code {
                    self.code_spans[code].end.min(text_end)
                } else {
                    self.code_spans
                        .iter()
                        .find(|span| span.start > offset)
                        .map_or(text_end, |span| span.start.min(text_end))
                };
                let range = offset..fragment_end;
                let runs = slice_runs(&self.runs, &range);
                let line = text_system.shape_line(
                    self.text[range.clone()].to_string().into(),
                    font_size * if code.is_some() { CODE_FONT_SCALE } else { 1. },
                    &runs,
                    None,
                );
                fragments.push(Fragment {
                    width: line.width(),
                    line,
                    runs,
                    range,
                    code,
                    origin: Point::default(),
                });
                offset = fragment_end;
            }
            tokens.push(Token {
                fragments,
                hard_break,
            });
            start = end;
        }
        tokens
    }
}

fn slice_runs(runs: &[TextRun], range: &Range<usize>) -> Vec<TextRun> {
    let mut offset = 0;
    runs.iter()
        .filter_map(|run| {
            let start = offset.max(range.start);
            offset += run.len;
            let end = offset.min(range.end);
            (start < end).then(|| {
                let mut run = run.clone();
                run.len = end - start;
                run
            })
        })
        .collect()
}

#[derive(Default)]
struct Row {
    x: Pixels,
    y: Pixels,
    code: Option<usize>,
    chip_start: Pixels,
}

impl Row {
    fn width_after(&self, fragments: &[Fragment]) -> Pixels {
        let mut x = self.x;
        let mut code = self.code;
        for fragment in fragments {
            if code != fragment.code {
                x += chip_edge(code) + chip_edge(fragment.code);
                code = fragment.code;
            }
            x += fragment.width;
        }
        x + chip_edge(code)
    }

    fn close_chip(&mut self, layout: &mut InlineLayout, line_height: Pixels) {
        if self.code.take().is_some() {
            self.x += px(CHIP_PADDING);
            layout.chips.push(Bounds::new(
                point(self.chip_start, self.y),
                size(self.x - self.chip_start, line_height - px(1.)),
            ));
            self.x += px(CHIP_MARGIN);
        }
    }

    fn finish(&mut self, layout: &mut InlineLayout, line_height: Pixels) {
        self.close_chip(layout, line_height);
        layout.size.width = layout.size.width.max(self.x).ceil();
        self.x = px(0.);
        self.y += line_height;
    }

    fn push(&mut self, mut fragment: Fragment, layout: &mut InlineLayout, line_height: Pixels) {
        if self.code != fragment.code {
            self.close_chip(layout, line_height);
            if fragment.code.is_some() {
                self.x += px(CHIP_MARGIN);
                self.chip_start = self.x;
                self.x += px(CHIP_PADDING);
            }
            self.code = fragment.code;
        }
        fragment.origin = point(self.x, self.y);
        self.x += fragment.width;
        layout.fragments.push(fragment);
    }
}

fn chip_edge(code: Option<usize>) -> Pixels {
    px(if code.is_some() {
        CHIP_PADDING + CHIP_MARGIN
    } else {
        0.
    })
}

fn fitting_prefix(text: &str, available: Pixels, width_at: impl Fn(usize) -> Pixels) -> usize {
    let ends: Vec<_> = text
        .grapheme_indices(true)
        .map(|(start, grapheme)| start + grapheme.len())
        .collect();
    let count = ends.partition_point(|end| width_at(*end) <= available);
    count.checked_sub(1).map_or(0, |index| ends[index])
}

impl LayoutState {
    fn layout(&self, wrap_width: Option<Pixels>, text_system: &WindowTextSystem) -> InlineLayout {
        let mut layout = InlineLayout::default();
        let mut row = Row::default();
        for token in &self.tokens {
            if let Some(width) = wrap_width {
                if row.x > px(0.) && row.width_after(&token.fragments) > width {
                    row.finish(&mut layout, self.line_height);
                }
                for fragment in &token.fragments {
                    let mut fragment = fragment.clone();
                    loop {
                        if row.width_after(std::slice::from_ref(&fragment)) <= width {
                            row.push(fragment, &mut layout, self.line_height);
                            break;
                        }
                        let available = width
                            - row.width_after(&[Fragment {
                                width: px(0.),
                                ..fragment.clone()
                            }]);
                        let mut end = fitting_prefix(&fragment.line.text, available, |end| {
                            text_system
                                .shape_line(
                                    fragment.line.text[..end].to_string().into(),
                                    fragment.line.font_size,
                                    &slice_runs(&fragment.runs, &(0..end)),
                                    None,
                                )
                                .width()
                        });
                        if end == 0 && row.x > px(0.) {
                            row.finish(&mut layout, self.line_height);
                            continue;
                        }
                        if end == 0 {
                            end = fragment.line.text.graphemes(true).next().unwrap().len();
                        }
                        if end == fragment.range.len() {
                            row.push(fragment, &mut layout, self.line_height);
                            break;
                        }
                        // Reshape at emergency breaks so a ligature cannot lose
                        // glyphs when its bytes move onto the continuation row.
                        let prefix_runs = slice_runs(&fragment.runs, &(0..end));
                        let suffix_runs = slice_runs(&fragment.runs, &(end..fragment.range.len()));
                        let prefix = text_system.shape_line(
                            fragment.line.text[..end].to_string().into(),
                            fragment.line.font_size,
                            &prefix_runs,
                            None,
                        );
                        let suffix = text_system.shape_line(
                            fragment.line.text[end..].to_string().into(),
                            fragment.line.font_size,
                            &suffix_runs,
                            None,
                        );
                        let split = fragment.range.start + end;
                        row.push(
                            Fragment {
                                range: fragment.range.start..split,
                                width: prefix.width(),
                                line: prefix,
                                runs: prefix_runs,
                                ..fragment.clone()
                            },
                            &mut layout,
                            self.line_height,
                        );
                        row.finish(&mut layout, self.line_height);
                        fragment.range.start = split;
                        fragment.width = suffix.width();
                        fragment.line = suffix;
                        fragment.runs = suffix_runs;
                    }
                }
            } else {
                for fragment in &token.fragments {
                    row.push(fragment.clone(), &mut layout, self.line_height);
                }
            }
            if token.hard_break {
                row.finish(&mut layout, self.line_height);
            }
        }
        row.finish(&mut layout, self.line_height);
        layout.size.height = row.y;
        layout
    }
}

impl InlineLayout {
    fn link_at(
        &self,
        position: Point<Pixels>,
        bounds: Bounds<Pixels>,
        links: &[LinkSpan],
        line_height: Pixels,
    ) -> Option<usize> {
        let position = position - bounds.origin;
        self.fragments.iter().find_map(|fragment| {
            let glyph_bounds = Bounds::new(fragment.origin, size(fragment.width, line_height));
            if !glyph_bounds.contains(&position) {
                return None;
            }
            let index =
                fragment.range.start + fragment.line.index_for_x(position.x - fragment.origin.x)?;
            links.iter().position(|link| link.range.contains(&index))
        })
    }
}

impl Element for CodeChipText {
    type RequestLayoutState = Rc<LayoutState>;
    type PrepaintState = PaintState;

    fn id(&self) -> Option<ElementId> {
        Some("md-paragraph".into())
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        window: &mut Window,
        _: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let state = Rc::new(self.shape(window));
        let measure = state.clone();
        let id = window.request_measured_layout(
            Default::default(),
            move |known, available, window, _| {
                let width = if measure.wrap {
                    known.width.or(match available.width {
                        AvailableSpace::Definite(width) => Some(width),
                        _ => None,
                    })
                } else {
                    None
                };
                measure.layout(width, window.text_system()).size
            },
        );
        (id, state)
    }

    fn prepaint(
        &mut self,
        _: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        window: &mut Window,
        _: &mut App,
    ) -> Self::PrepaintState {
        PaintState {
            layout: Rc::new(state.layout(
                state.wrap.then_some(bounds.size.width),
                window.text_system(),
            )),
            hitbox: window.insert_hitbox(bounds, HitboxBehavior::Normal),
        }
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut Self::RequestLayoutState,
        paint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        for chip in &paint.layout.chips {
            let chip = Bounds::new(bounds.origin + chip.origin, chip.size);
            window.paint_quad(
                fill(chip, self.background)
                    .corner_radii(Corners::all(self.radius))
                    .border_widths(gpui::Edges::all(px(1.)))
                    .border_color(self.border_color),
            );
        }
        for fragment in &paint.layout.fragments {
            let baseline = (state.line_height + fragment.line.ascent - fragment.line.descent) / 2.;
            let origin = bounds.origin + fragment.origin + point(px(0.), state.baseline - baseline);
            fragment
                .line
                .paint_background(origin, state.line_height, TextAlign::Left, None, window, cx)
                .ok();
            fragment
                .line
                .paint(origin, state.line_height, TextAlign::Left, None, window, cx)
                .ok();
        }
        if self.links.is_empty() {
            return;
        }
        window.with_element_state::<LinkState, _>(id.unwrap(), |previous, window| {
            let link_state = previous.unwrap_or_default();
            let layout = paint.layout.clone();
            let links = self.links.clone();
            let line_height = state.line_height;
            let hitbox = paint.hitbox.clone();
            let link_at =
                Rc::new(move |position| layout.link_at(position, bounds, &links, line_height));
            if link_at(window.mouse_position()).is_some() {
                window.set_cursor_style(CursorStyle::PointingHand, &hitbox);
            }
            window.on_mouse_event({
                let link_at = link_at.clone();
                let hitbox = hitbox.clone();
                let mouse_down = link_state.mouse_down.clone();
                move |event: &MouseDownEvent, phase, window, _| {
                    if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                        mouse_down.set(
                            hitbox
                                .is_hovered(window)
                                .then(|| link_at(event.position))
                                .flatten(),
                        );
                    }
                }
            });
            window.on_mouse_event({
                let link_at = link_at.clone();
                let hitbox = hitbox.clone();
                let mouse_down = link_state.mouse_down.clone();
                let links = self.links.clone();
                let on_link_click = self.on_link_click.clone();
                move |event: &MouseUpEvent, phase, window, cx| {
                    if phase == DispatchPhase::Bubble && event.button == MouseButton::Left {
                        let down = mouse_down.take();
                        if let Some(index) = down
                            && hitbox.is_hovered(window)
                            && link_at(event.position) == Some(index)
                        {
                            on_link_click(&links[index].href, cx, window);
                        }
                    }
                }
            });
            window.on_mouse_event({
                let hovered = link_state.hovered.clone();
                move |event: &MouseMoveEvent, phase, window, _| {
                    if phase == DispatchPhase::Bubble {
                        let index = hitbox
                            .is_hovered(window)
                            .then(|| link_at(event.position))
                            .flatten();
                        if hovered.replace(index) != index {
                            window.refresh();
                        }
                    }
                }
            });
            ((), link_state)
        });
    }
}

impl IntoElement for CodeChipText {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
