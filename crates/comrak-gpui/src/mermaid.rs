use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;
use std::sync::{Arc, atomic::AtomicU64, atomic::Ordering};
use std::time::Duration;

use anyhow::{Context as _, Result, anyhow};
use gpui::prelude::FluentBuilder;
use gpui::{
    AnyElement, App, AppContext, Bounds, Context, Entity, InteractiveElement, IntoElement,
    MouseButton, ParentElement, ParsedSvg, Pixels, Point, Render, RenderImage, ScrollDelta,
    SharedString, Styled, SvgRenderer, Task, WeakEntity, Window, canvas, div, img, point, px, size,
};
use gpui_component::button::{Button, ButtonVariants};
use gpui_component::dialog::DialogContent;
use gpui_component::{ActiveTheme, Icon, IconName, Sizable, Theme, WindowExt};

const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 4.0;
const ZOOM_STEP: f32 = 0.1;
const PIXELS_PER_ZOOM_TICK: f32 = 20.0;
const INLINE_VIEWPORT_HEIGHT: f32 = 360.0;
const DIALOG_WIDTH_RATIO: f32 = 0.9;
const DIALOG_CONTENT_HEIGHT_RATIO: f32 = 0.75;
const RERASTER_DEBOUNCE: Duration = Duration::from_millis(150);
const RENDER_DEBOUNCE: Duration = Duration::from_millis(300);
const MAX_RASTER_PIXELS: f32 = 32_000_000.0;
const SMOOTH_SCALE: f32 = gpui::SMOOTH_SVG_SCALE_FACTOR;
const FONT_FAMILY: &str =
    "Inter, ui-sans-serif, system-ui, -apple-system, \"Segoe UI\", \"DejaVu Sans\", sans-serif";

pub(crate) fn is_mermaid_info(info: &str) -> bool {
    info.split_whitespace()
        .next()
        .is_some_and(|language| language.eq_ignore_ascii_case("mermaid"))
}

/// Diagram entities that outlive a single call to `render_document`, one per
/// mermaid block position in the document, so zoom, pan and the rendered image
/// survive re-renders and edits of the source.
#[derive(Default)]
pub struct MermaidDiagrams {
    entities: Vec<Entity<MermaidDiagram>>,
}

impl MermaidDiagrams {
    pub(crate) fn sync(&mut self, sources: &[SharedString], cx: &mut App) -> MermaidBlocks {
        self.entities.truncate(sources.len());
        for (index, source) in sources.iter().enumerate() {
            match self.entities.get(index) {
                Some(entity) => {
                    entity.update(cx, |diagram, cx| diagram.set_source(source.clone(), cx));
                }
                None => {
                    let entity = cx.new(|cx| MermaidDiagram::new(source.clone(), cx));
                    self.entities.push(entity);
                }
            }
        }
        MermaidBlocks {
            entities: self.entities.clone(),
            next: Cell::new(0),
        }
    }
}

/// The diagrams of one render pass, handed out in document order.
pub(crate) struct MermaidBlocks {
    entities: Vec<Entity<MermaidDiagram>>,
    next: Cell<usize>,
}

impl MermaidBlocks {
    pub(crate) fn take_next(&self) -> Option<Entity<MermaidDiagram>> {
        let index = self.next.get();
        self.next.set(index + 1);
        self.entities.get(index).cloned()
    }
}

#[derive(Clone, PartialEq)]
struct ThemeKey {
    dark: bool,
    background: gpui::Hsla,
    foreground: gpui::Hsla,
}

enum RenderState {
    Rendering,
    Ready,
    Failed(SharedString),
}

pub(crate) struct MermaidDiagram {
    source: SharedString,
    theme_key: ThemeKey,
    state: RenderState,
    viewport: Entity<DiagramViewport>,
    parsed_svg: Option<Arc<ParsedSvg>>,
    natural_size: Option<gpui::Size<Pixels>>,
    maximized_viewport: Option<WeakEntity<DiagramViewport>>,
    render_task: Option<Task<Result<()>>>,
}

impl MermaidDiagram {
    fn new(source: SharedString, cx: &mut Context<Self>) -> Self {
        let weak_diagram = cx.weak_entity();
        let on_maximize: MaximizeHandler = Rc::new(move |window, cx| {
            if let Some(diagram) = weak_diagram.upgrade() {
                diagram.update(cx, |diagram, cx| diagram.open_maximized(window, cx));
            }
        });
        let mut diagram = Self {
            source: SharedString::default(),
            theme_key: theme_key(cx.theme()),
            state: RenderState::Rendering,
            viewport: cx.new(|_| {
                DiagramViewport::new(
                    ViewportHeight::Fixed(px(INLINE_VIEWPORT_HEIGHT)),
                    Some(on_maximize),
                )
            }),
            parsed_svg: None,
            natural_size: None,
            maximized_viewport: None,
            render_task: None,
        };
        diagram.set_source(source, cx);
        diagram
    }

    fn set_source(&mut self, source: SharedString, cx: &mut Context<Self>) {
        if self.source == source {
            return;
        }
        let debounce = if self.source.is_empty() {
            Duration::ZERO
        } else {
            RENDER_DEBOUNCE
        };
        self.source = source;
        self.start_render(debounce, cx);
    }

    fn start_render(&mut self, debounce: Duration, cx: &mut Context<Self>) {
        let source = self.source.clone();
        let config = mermaid_config(cx.theme());
        let svg_renderer = cx.svg_renderer();
        self.theme_key = theme_key(cx.theme());
        if !self.viewport.read(cx).has_content() {
            self.state = RenderState::Rendering;
        }
        self.render_task = Some(cx.spawn(async move |this, cx| {
            if !debounce.is_zero() {
                cx.background_executor().timer(debounce).await;
            }
            let rendered = cx
                .background_spawn(async move { rasterize_source(&source, config, &svg_renderer) })
                .await;
            this.update(cx, |this, cx| {
                match rendered {
                    Ok((parsed_svg, image)) => this.finish_render(parsed_svg, image, cx),
                    Err(error) => {
                        this.state = RenderState::Failed(format!("{error:#}").into());
                    }
                }
                cx.notify();
            })
        }));
    }

    fn finish_render(
        &mut self,
        parsed_svg: Arc<ParsedSvg>,
        image: Arc<RenderImage>,
        cx: &mut Context<Self>,
    ) {
        let device_size = image.size(0);
        let natural_size = size(
            px(device_size.width.0 as f32 / SMOOTH_SCALE),
            px(device_size.height.0 as f32 / SMOOTH_SCALE),
        );
        self.state = RenderState::Ready;
        self.parsed_svg = Some(parsed_svg.clone());
        self.natural_size = Some(natural_size);
        if let Some(maximized) = self
            .maximized_viewport
            .as_ref()
            .and_then(WeakEntity::upgrade)
        {
            maximized.update(cx, |viewport, cx| {
                viewport.set_svg(parsed_svg.clone(), natural_size, cx)
            });
        }
        self.viewport.update(cx, |viewport, cx| {
            viewport.set_content(parsed_svg, natural_size, image, cx)
        });
    }

    fn open_maximized(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(parsed_svg), Some(natural_size)) = (self.parsed_svg.clone(), self.natural_size)
        else {
            return;
        };
        let viewport = cx.new(|_| DiagramViewport::new(ViewportHeight::Fill, None));
        viewport.update(cx, |viewport, cx| {
            viewport.set_svg(parsed_svg, natural_size, cx)
        });
        self.maximized_viewport = Some(viewport.downgrade());

        let window_size = window.viewport_size();
        let dialog_width = window_size.width * DIALOG_WIDTH_RATIO;
        let content_height = window_size.height * DIALOG_CONTENT_HEIGHT_RATIO;
        let released_viewport = viewport.clone();
        window.open_dialog(cx, move |dialog, _, _| {
            let viewport = viewport.clone();
            let released_viewport = released_viewport.clone();
            dialog
                .title("Diagram")
                .width(dialog_width)
                .max_w(dialog_width)
                .content(move |content: DialogContent, _, _| {
                    content.child(div().w_full().h(content_height).child(viewport.clone()))
                })
                .on_close(move |_, _, cx| {
                    released_viewport.update(cx, |viewport, cx| viewport.release_image(cx));
                })
        });
    }

    fn render_source(&self, status: Option<SharedString>, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                div()
                    .font_family(theme.mono_font_family.clone())
                    .text_sm()
                    .text_color(theme.secondary_foreground)
                    .child(self.source.clone()),
            )
            .children(status.map(|status| {
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(status)
            }))
            .into_any_element()
    }
}

impl Render for MermaidDiagram {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let current_theme_key = theme_key(cx.theme());
        if current_theme_key != self.theme_key {
            self.start_render(Duration::ZERO, cx);
        }

        let theme = cx.theme();
        let container = div()
            .relative()
            .w_full()
            .mt_2()
            .mb_2()
            .p_2()
            .rounded_lg()
            .border(px(0.5))
            .border_color(theme.colors.border);

        if let RenderState::Failed(error) = &self.state {
            let status = format!("Could not render diagram: {error}");
            return container.child(self.render_source(Some(status.into()), cx));
        }
        if !self.viewport.read(cx).has_content() {
            return container.child(self.render_source(Some("Rendering diagram...".into()), cx));
        }
        container.child(self.viewport.clone())
    }
}

type MaximizeHandler = Rc<dyn Fn(&mut Window, &mut App)>;

enum ViewportHeight {
    Fixed(Pixels),
    /// Takes the height of the parent, which must have a definite one: the fit
    /// zoom and the offset clamping need to know the viewport size.
    Fill,
}

/// The zoomable and pannable view of a rendered diagram. It owns everything
/// that depends on where the diagram is shown (zoom, offset, its own raster),
/// while the parsed SVG it draws comes from the `MermaidDiagram` that owns it.
struct DiagramViewport {
    height: ViewportHeight,
    on_maximize: Option<MaximizeHandler>,
    parsed_svg: Option<Arc<ParsedSvg>>,
    natural_size: Option<gpui::Size<Pixels>>,
    image: Option<Arc<RenderImage>>,
    image_scale: f32,
    zoom: f32,
    offset: Point<Pixels>,
    needs_fit: bool,
    drag_origin: Option<Point<Pixels>>,
    bounds: Rc<RefCell<Bounds<Pixels>>>,
    raster_task: Option<Task<Result<()>>>,
}

impl DiagramViewport {
    fn new(height: ViewportHeight, on_maximize: Option<MaximizeHandler>) -> Self {
        Self {
            height,
            on_maximize,
            parsed_svg: None,
            natural_size: None,
            image: None,
            image_scale: 1.0,
            zoom: 1.0,
            offset: Point::default(),
            needs_fit: true,
            drag_origin: None,
            bounds: Rc::new(RefCell::new(Bounds::default())),
            raster_task: None,
        }
    }

    fn has_content(&self) -> bool {
        self.image.is_some() && self.natural_size.is_some()
    }

    fn set_content(
        &mut self,
        parsed_svg: Arc<ParsedSvg>,
        natural_size: gpui::Size<Pixels>,
        image: Arc<RenderImage>,
        cx: &mut Context<Self>,
    ) {
        let first_content = self.natural_size.is_none();
        self.natural_size = Some(natural_size);
        if let Some(previous) = self.image.replace(image) {
            cx.drop_image(previous, None);
        }
        self.parsed_svg = Some(parsed_svg);
        self.image_scale = 1.0;
        if first_content {
            self.needs_fit = true;
        } else {
            self.clamp_offset();
            self.schedule_reraster(cx);
        }
        cx.notify();
    }

    /// Updates the diagram this viewport draws without providing a raster: the
    /// viewport makes its own, and keeps showing the previous one meanwhile.
    fn set_svg(
        &mut self,
        parsed_svg: Arc<ParsedSvg>,
        natural_size: gpui::Size<Pixels>,
        cx: &mut Context<Self>,
    ) {
        self.natural_size = Some(natural_size);
        self.parsed_svg = Some(parsed_svg);
        self.clamp_offset();
        self.schedule_reraster(cx);
        cx.notify();
    }

    fn release_image(&mut self, cx: &mut Context<Self>) {
        self.raster_task = None;
        if let Some(image) = self.image.take() {
            cx.drop_image(image, None);
        }
    }

    fn schedule_reraster(&mut self, cx: &mut Context<Self>) {
        let (Some(parsed_svg), Some(natural_size)) = (self.parsed_svg.clone(), self.natural_size)
        else {
            return;
        };
        let target_scale = self.raster_scale(natural_size);
        let svg_renderer = cx.svg_renderer();
        // Nothing is shown yet, so there is no blurry image to avoid flashing.
        let debounce = if self.image.is_some() {
            RERASTER_DEBOUNCE
        } else {
            Duration::ZERO
        };
        self.raster_task = Some(cx.spawn(async move |this, cx| {
            if !debounce.is_zero() {
                cx.background_executor().timer(debounce).await;
            }
            let rendered = cx
                .background_spawn(async move {
                    svg_renderer
                        .render_parsed(&parsed_svg, target_scale)
                        .map_err(|error| anyhow!("{error}"))
                })
                .await;
            let image = rendered?;
            this.update(cx, |this, cx| {
                if let Some(previous) = this.image.replace(image) {
                    cx.drop_image(previous, None);
                }
                this.image_scale = target_scale;
                cx.notify();
            })
        }));
    }

    fn raster_scale(&self, natural_size: gpui::Size<Pixels>) -> f32 {
        let natural_pixels = f32::from(natural_size.width) * f32::from(natural_size.height);
        let cap = (MAX_RASTER_PIXELS / (natural_pixels * SMOOTH_SCALE * SMOOTH_SCALE))
            .sqrt()
            .max(MIN_ZOOM);
        self.zoom.min(cap)
    }

    fn viewport_size(&self) -> gpui::Size<Pixels> {
        self.bounds.borrow().size
    }

    fn fit_zoom(&self) -> Option<f32> {
        let natural_size = self.natural_size?;
        let viewport = self.viewport_size();
        if viewport.width <= px(0.) || viewport.height <= px(0.) {
            return None;
        }
        let fit_width = f32::from(viewport.width) / f32::from(natural_size.width);
        let fit_height = f32::from(viewport.height) / f32::from(natural_size.height);
        Some(fit_width.min(fit_height).clamp(MIN_ZOOM, 1.0))
    }

    fn clamp_offset(&mut self) {
        let Some(natural_size) = self.natural_size else {
            return;
        };
        let viewport = self.viewport_size();
        self.offset = point(
            clamp_axis(
                self.offset.x,
                natural_size.width * self.zoom,
                viewport.width,
            ),
            clamp_axis(
                self.offset.y,
                natural_size.height * self.zoom,
                viewport.height,
            ),
        );
    }

    fn reset_view(&mut self, cx: &mut Context<Self>) {
        self.zoom = self.fit_zoom().unwrap_or(1.0);
        self.offset = Point::default();
        self.clamp_offset();
        self.schedule_reraster(cx);
        cx.notify();
    }

    fn set_zoom_at(&mut self, new_zoom: f32, anchor: Point<Pixels>, cx: &mut Context<Self>) {
        let new_zoom = new_zoom.clamp(MIN_ZOOM, MAX_ZOOM);
        if (new_zoom - self.zoom).abs() < f32::EPSILON {
            return;
        }
        // Keep the diagram point under the anchor fixed while scaling.
        let ratio = new_zoom / self.zoom;
        self.offset = point(
            anchor.x - (anchor.x - self.offset.x) * ratio,
            anchor.y - (anchor.y - self.offset.y) * ratio,
        );
        self.zoom = new_zoom;
        self.clamp_offset();
        self.schedule_reraster(cx);
        cx.notify();
    }

    fn zoom_around_center(&mut self, direction: f32, cx: &mut Context<Self>) {
        let viewport = self.viewport_size();
        let center = point(viewport.width / 2., viewport.height / 2.);
        self.set_zoom_at(self.zoom + direction * ZOOM_STEP, center, cx);
    }

    fn local_position(&self, position: Point<Pixels>) -> Point<Pixels> {
        position - self.bounds.borrow().origin
    }

    fn handle_scroll(&mut self, event: &gpui::ScrollWheelEvent, cx: &mut Context<Self>) -> bool {
        if event.modifiers.control || event.modifiers.platform {
            let ticks = match event.delta {
                ScrollDelta::Lines(lines) => lines.y,
                ScrollDelta::Pixels(pixels) => f32::from(pixels.y) / PIXELS_PER_ZOOM_TICK,
            }
            .clamp(-1.0, 1.0);
            if ticks != 0.0 {
                let anchor = self.local_position(event.position);
                self.set_zoom_at(self.zoom + ticks * ZOOM_STEP, anchor, cx);
            }
            return true;
        }
        // Plain vertical scrolling must keep scrolling the note preview; only
        // horizontal motion pans the diagram.
        let delta = event.delta.pixel_delta(px(20.));
        let horizontal = event.modifiers.shift || delta.x.abs() > delta.y.abs();
        if !horizontal {
            return false;
        }
        let horizontal_delta = if event.modifiers.shift && delta.x == px(0.) {
            delta.y
        } else {
            delta.x
        };
        self.offset.x += horizontal_delta;
        self.clamp_offset();
        cx.notify();
        true
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> AnyElement {
        let percent = format!("{:.0}%", self.zoom * 100.0);
        div()
            .absolute()
            .top_1()
            .right_1()
            .flex()
            .items_center()
            .gap_1()
            .child(
                div()
                    .px_1()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(percent),
            )
            .when_some(self.on_maximize.clone(), |toolbar, on_maximize| {
                toolbar.child(
                    Button::new("mermaid-maximize")
                        .icon(Icon::new(IconName::Maximize))
                        .ghost()
                        .xsmall()
                        .on_click(move |_, window, cx| on_maximize(window, cx)),
                )
            })
            .child(
                Button::new("mermaid-fit")
                    .label("Fit")
                    .ghost()
                    .xsmall()
                    .on_click(cx.listener(|this, _, _, cx| this.reset_view(cx))),
            )
            .into_any_element()
    }
}

impl Render for DiagramViewport {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(natural_size) = self.natural_size else {
            return div();
        };

        if self.needs_fit {
            match self.fit_zoom() {
                Some(fit_zoom) => {
                    self.needs_fit = false;
                    self.zoom = fit_zoom;
                    self.offset = Point::default();
                    self.clamp_offset();
                    self.schedule_reraster(cx);
                }
                None => window.request_animation_frame(),
            }
        }

        let bounds_cell = self.bounds.clone();
        let diagram = cx.entity();
        let display_size = size(
            natural_size.width * self.zoom,
            natural_size.height * self.zoom,
        );
        let dragging = self.drag_origin.is_some();

        let viewport_element = div()
            .id("mermaid-viewport")
            .relative()
            .w_full()
            .map(|viewport| match self.height {
                ViewportHeight::Fixed(height) => viewport.h(height),
                ViewportHeight::Fill => viewport.h_full(),
            })
            .overflow_hidden()
            .map_cursor(dragging)
            .child(
                canvas(
                    move |bounds, _, _| *bounds_cell.borrow_mut() = bounds,
                    move |bounds, _, window, _| {
                        // The note preview's scroll container claims wheel events
                        // during the capture phase and stops them, so a bubble
                        // phase listener on the viewport would never run.
                        window.on_mouse_event(
                            move |event: &gpui::ScrollWheelEvent, phase, _window, cx| {
                                if phase != gpui::DispatchPhase::Capture
                                    || !bounds.contains(&event.position)
                                {
                                    return;
                                }
                                let handled =
                                    diagram.update(cx, |this, cx| this.handle_scroll(event, cx));
                                if handled {
                                    cx.stop_propagation();
                                }
                            },
                        );
                    },
                )
                .absolute()
                .size_full(),
            )
            .children(self.image.clone().map(|image| {
                img(gpui::ImageSource::Render(image))
                    .absolute()
                    .left(self.offset.x)
                    .top(self.offset.y)
                    .w(display_size.width)
                    .h(display_size.height)
            }))
            .on_pinch(cx.listener(|this, event: &gpui::PinchEvent, _, cx| {
                let anchor = this.local_position(event.position);
                this.set_zoom_at(this.zoom * (1.0 + event.delta), anchor, cx);
                cx.stop_propagation();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &gpui::MouseDownEvent, _, cx| {
                    if event.click_count >= 2 {
                        this.reset_view(cx);
                        return;
                    }
                    this.drag_origin = Some(event.position);
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &gpui::MouseMoveEvent, _, cx| {
                let Some(origin) = this.drag_origin else {
                    return;
                };
                if !event.dragging() {
                    this.drag_origin = None;
                    cx.notify();
                    return;
                }
                this.offset += event.position - origin;
                this.drag_origin = Some(event.position);
                this.clamp_offset();
                cx.notify();
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag_origin = None;
                    cx.notify();
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
                    this.drag_origin = None;
                    cx.notify();
                }),
            );

        div()
            .relative()
            .w_full()
            .when(matches!(self.height, ViewportHeight::Fill), |wrapper| {
                wrapper.h_full()
            })
            .child(viewport_element)
            .child(self.render_toolbar(cx))
    }
}

trait CursorExt: Styled + Sized {
    fn map_cursor(self, dragging: bool) -> Self {
        if dragging {
            self.cursor_grabbing()
        } else {
            self.cursor_grab()
        }
    }
}

impl<T: Styled + Sized> CursorExt for T {}

fn clamp_axis(offset: Pixels, content: Pixels, viewport: Pixels) -> Pixels {
    if content <= viewport {
        (viewport - content) / 2.
    } else {
        offset.clamp(viewport - content, px(0.))
    }
}

fn theme_key(theme: &Theme) -> ThemeKey {
    ThemeKey {
        dark: theme.is_dark(),
        background: theme.colors.background,
        foreground: theme.colors.foreground,
    }
}

fn css_color(color: gpui::Hsla) -> String {
    let rgba = gpui::Rgba::from(color);
    let channel = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!(
        "#{:02x}{:02x}{:02x}",
        channel(rgba.r),
        channel(rgba.g),
        channel(rgba.b)
    )
}

fn mermaid_config(theme: &Theme) -> serde_json::Value {
    let colors = &theme.colors;
    let background = css_color(colors.background);
    let text = css_color(colors.foreground);
    let node_fill = css_color(colors.secondary);
    let node_text = css_color(colors.secondary_foreground);
    let line = css_color(colors.muted_foreground);
    let border = css_color(colors.border);
    let accent = css_color(colors.primary);
    let muted = css_color(colors.muted);

    serde_json::json!({
        "theme": "base",
        "look": "classic",
        "darkMode": theme.is_dark(),
        "fontFamily": FONT_FAMILY,
        "htmlLabels": true,
        "flowchart": { "htmlLabels": true, "padding": 16, "wrappingWidth": 200, "minNodeWidth": 0 },
        "state": { "wrappingWidth": 200, "minNodeWidth": 0 },
        "themeVariables": {
            "primaryColor": node_fill,
            "primaryTextColor": node_text,
            "primaryBorderColor": border,
            "lineColor": line,
            "secondaryColor": muted,
            "secondaryTextColor": text,
            "tertiaryColor": background,
            "tertiaryTextColor": text,
            "background": background,
            "mainBkg": node_fill,
            "nodeBorder": border,
            "nodeTextColor": node_text,
            "clusterBkg": muted,
            "clusterBorder": border,
            "titleColor": text,
            "edgeLabelBackground": background,
            "textColor": text,
            "fontFamily": FONT_FAMILY,
            "noteBkgColor": muted,
            "noteBorderColor": border,
            "noteTextColor": text,
            "actorBkg": node_fill,
            "actorBorder": border,
            "actorTextColor": node_text,
            "labelTextColor": text,
            "loopTextColor": text,
            "signalColor": text,
            "signalTextColor": text,
            "activationBkgColor": muted,
            "activationBorderColor": border,
            "classText": text,
            "labelColor": node_text,
            "pieTitleTextColor": text,
            "pieSectionTextColor": text,
            "pieLegendTextColor": text,
            "pieStrokeColor": border,
            "pieOuterStrokeColor": border,
            "quadrantPointFill": accent,
        },
    })
}

fn rasterize_source(
    source: &str,
    config: serde_json::Value,
    svg_renderer: &SvgRenderer,
) -> Result<(Arc<ParsedSvg>, Arc<RenderImage>)> {
    // merman is third-party layout code working on arbitrary user input; a
    // panic there must not take the whole app down.
    let svg = catch_unwind(AssertUnwindSafe(|| render_to_svg(source, config)))
        .map_err(|_| anyhow!("the diagram renderer crashed"))??;
    let parsed_svg = svg_renderer
        .parse_svg(svg.as_bytes())
        .map_err(|error| anyhow!("{error}"))?;
    let image = svg_renderer
        .render_parsed(&parsed_svg, 1.0)
        .map_err(|error| anyhow!("{error}"))?;
    Ok((Arc::new(parsed_svg), image))
}

fn render_to_svg(source: &str, config: serde_json::Value) -> Result<String> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let diagram_id = format!("oden-mermaid-{}", COUNTER.fetch_add(1, Ordering::Relaxed));

    let request = merman::SvgRequest {
        environment: merman::SvgEnvironment::deterministic(),
        options: merman::svg::SvgRenderOptions {
            diagram_id: Some(diagram_id),
            ..merman::svg::SvgRenderOptions::default()
        },
        pipeline: Some(merman::svg::SvgPipeline::resvg_safe()),
        ..merman::SvgRequest::default()
    };
    let engine = merman::Engine::new().with_site_config(merman::MermaidConfig::from_value(config));
    let output = merman::Renderer::new()
        .with_engine(engine)
        .render(merman::RenderRequest::svg(
            source,
            merman::OperationControl::new(),
            request,
        ));
    let merman::RenderOutput::Svg(Some(svg)) = output.context("merman render failed")? else {
        return Err(anyhow!("merman returned no SVG for the given input"));
    };
    let (svg, _) = svg.into_parts();
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> serde_json::Value {
        serde_json::json!({ "theme": "base", "fontFamily": FONT_FAMILY })
    }

    #[test]
    fn supported_diagram_types_rasterize() {
        let svg_renderer = SvgRenderer::new(Arc::new(()));
        let sources = [
            "flowchart TD\n  A[Start] --> B{Choice}\n  B -->|yes| C[Done]\n  B -->|no| A",
            "sequenceDiagram\n  Alice->>Bob: Hello\n  Bob-->>Alice: Hi",
            "classDiagram\n  class Shelter {\n    -List~Animal~ animals\n  }",
            "stateDiagram-v2\n  [*] --> Idle\n  Idle --> Busy",
            "pie title Pets\n  \"Dogs\" : 3\n  \"Cats\" : 2",
        ];
        for source in sources {
            let (_, image) = rasterize_source(source, default_config(), &svg_renderer)
                .unwrap_or_else(|error| panic!("{source}: {error:#}"));
            let device_size = image.size(0);
            assert!(
                device_size.width.0 > 0 && device_size.height.0 > 0,
                "{source}"
            );
        }
    }

    #[test]
    fn invalid_source_is_an_error_not_a_panic() {
        let svg_renderer = SvgRenderer::new(Arc::new(()));
        let result = rasterize_source("not a diagram at all", default_config(), &svg_renderer);
        assert!(result.is_err());
    }
}
