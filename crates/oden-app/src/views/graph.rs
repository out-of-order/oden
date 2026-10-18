use crate::store::ItemStore;
use gpui::{
    Context, Font, FontFeatures, FontStyle, FontWeight, InteractiveElement, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, PinchEvent, ScrollWheelEvent, ShapedLine,
    SharedString, Subscription, TextAlign, TextRun, TextSystem,
};
use gpui::{ParentElement, Path, PathBuilder, Pixels, Point, canvas};
use gpui::{Render, Styled, div, px};
use gpui_component::ActiveTheme;
use oden_graph::forces::{CollideForce, LinkForce, ManyBodyForce};
use oden_graph::simulation;
use oden_graph::simulation::{Graph, Simulation};
use std::collections::HashMap;

pub struct GraphView {
    simulation: Simulation,
    pan: Point<f32>,
    // Set once `render` has centered `pan` on the viewport for the first
    // time (the viewport size isn't known until then).
    pan_initialized: bool,
    // Current zoom scale factor.
    zoom: f32,
    is_panning: bool,
    last_mouse_pos: Point<Pixels>,
    _store_sub: Subscription,
}

impl GraphView {
    pub fn new(cx: &mut Context<Self>) -> Self {
        let simulation = Self::create_simulation_from_store(cx);
        let _store_sub = cx.observe_global::<ItemStore>(|this, cx| {
            this.simulation = Self::create_simulation_from_store(cx);
        });
        Self {
            simulation,
            pan: Point::default(),
            pan_initialized: false,
            zoom: 1.0,
            is_panning: false,
            last_mouse_pos: Point::default(),
            _store_sub,
        }
    }

    pub fn restart(&mut self, cx: &mut Context<Self>) {
        self.simulation = Self::create_simulation_from_store(cx);
        cx.notify();
    }

    fn create_simulation_from_store(cx: &mut Context<GraphView>) -> Simulation {
        let items = cx.global::<ItemStore>().items.clone();
        let font = Font {
            family: cx.theme().font_family.clone(),
            features: FontFeatures::default(),
            fallbacks: None,
            weight: FontWeight::default(),
            style: FontStyle::default(),
        };
        let text_system = cx.text_system();
        let mut nodes = HashMap::new();
        let mut adjacency_list = HashMap::new();
        for (id, item) in items {
            let radius = Self::label_radius(text_system.as_ref(), &font, &item.name);
            nodes.insert(
                id,
                simulation::Node {
                    position: Point::default(),
                    velocity: Point::default(),
                    label: item.name.clone(),
                    radius,
                },
            );
            adjacency_list.insert(id, Vec::new());
        }
        for (source_id, target_ids) in cx.global::<ItemStore>().links.clone() {
            if let Some(old_targets) = adjacency_list.get_mut(&source_id) {
                *old_targets = target_ids;
            }
        }
        let graph: Graph = Graph {
            nodes,
            adjacency_list,
        };
        Simulation::new(graph)
            .with_force(Box::new(LinkForce::default()))
            .with_force(Box::new(ManyBodyForce::default()))
            .with_force(Box::new(CollideForce::default()))
    }

    fn label_radius(text_system: &TextSystem, font: &Font, label: &SharedString) -> f32 {
        let font_id = text_system.resolve_font(font);
        let width: f32 = label
            .chars()
            .filter_map(|ch| text_system.advance(font_id, px(LABEL_FONT_SIZE), ch).ok())
            .map(|size| f32::from(size.width))
            .sum();
        width / 2.0 + LABEL_PADDING + VERTEX_RADIUS
    }

    fn to_pixels(point: Point<f32>, pan: Point<f32>, zoom: f32) -> Point<Pixels> {
        Point {
            x: px(point.x * zoom + pan.x),
            y: px(point.y * zoom + pan.y),
        }
    }

    fn draw_vertex(
        point: Point<f32>,
        vertex_radius: f32,
        pan: Point<f32>,
        zoom: f32,
    ) -> Path<Pixels> {
        let point = Self::to_pixels(point, pan, zoom);
        let vertex_radius = vertex_radius * zoom;
        let radius_vector = Point {
            x: px(vertex_radius),
            y: px(0.),
        };
        let radii = Point {
            x: px(vertex_radius),
            y: px(vertex_radius),
        };
        let mut builder = PathBuilder::fill();
        builder.move_to(point);
        // draw the first half of the circle
        builder.line_to(point + radius_vector);
        builder.arc_to(radii, px(0.), false, false, point - radius_vector);
        // draw the second half
        builder.arc_to(radii, px(0.), false, false, point + radius_vector);
        builder.build().unwrap()
    }

    pub fn draw_edge(
        source: Point<f32>,
        target: Point<f32>,
        pan: Point<f32>,
        zoom: f32,
    ) -> Path<Pixels> {
        let source = Self::to_pixels(source, pan, zoom);
        let target = Self::to_pixels(target, pan, zoom);
        let mut builder = PathBuilder::stroke(px(1.5 * zoom));
        builder.move_to(source);
        builder.line_to(target);
        builder.build().unwrap()
    }

    fn zoom_at(&mut self, screen_pos: Point<Pixels>, factor: f32) {
        let new_zoom = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        if new_zoom == self.zoom {
            return;
        }
        let ratio = new_zoom / self.zoom;
        let screen_pos = Point {
            x: f32::from(screen_pos.x),
            y: f32::from(screen_pos.y),
        };

        self.pan.x = screen_pos.x + (self.pan.x - screen_pos.x) * ratio;
        self.pan.y = screen_pos.y + (self.pan.y - screen_pos.y) * ratio;
        self.zoom = new_zoom;
    }

    fn viewport_center(window: &gpui::Window) -> Point<Pixels> {
        let viewport = window.viewport_size();
        Point {
            x: viewport.width / 2.,
            y: viewport.height / 2.,
        }
    }
}

const LABEL_PADDING: f32 = 4.;
const LABEL_FONT_SIZE: f32 = 11.;
const VERTEX_RADIUS: f32 = 5.;
const MIN_ZOOM: f32 = 0.1;
const MAX_ZOOM: f32 = 4.0;
const ZOOM_WHEEL_SENSITIVITY: f32 = 0.004;

impl Render for GraphView {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::prelude::Context<Self>,
    ) -> impl gpui::prelude::IntoElement {
        let vertices_color = cx.theme().primary;
        let edges_color = cx.theme().secondary;
        let label_color = cx.theme().foreground;
        let label_font_family = cx.theme().font_family.clone();
        if !self.simulation.should_stop {
            self.simulation.tick();
            window.request_animation_frame();
        }
        if !self.pan_initialized {
            let center = Self::viewport_center(window);
            self.pan = Point {
                x: f32::from(center.x),
                y: f32::from(center.y),
            };
            self.pan_initialized = true;
        }
        let nodes_render_data: Vec<(SharedString, Point<f32>)> =
            self.simulation.nodes_render_data();
        let edges = self.simulation.edges();
        let pan = self.pan;
        let zoom = self.zoom;
        div()
            .flex_1()
            .min_w(px(360.0))
            .h_full()
            .overflow_hidden()
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, window, cx| {
                let pixel_delta = event.delta.pixel_delta(window.line_height());
                let factor = (1.0 - f32::from(pixel_delta.y) * ZOOM_WHEEL_SENSITIVITY).max(0.01);
                this.zoom_at(event.position, factor);
                cx.notify();
            }))
            .on_pinch(cx.listener(|this, event: &PinchEvent, _window, cx| {
                this.zoom_at(event.position, 1.0 + event.delta);
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                    this.is_panning = true;
                    this.last_mouse_pos = event.position;
                    cx.notify();
                }),
            )
            .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _window, cx| {
                if this.is_panning && event.dragging() {
                    let delta = event.position - this.last_mouse_pos;
                    this.pan.x += f32::from(delta.x);
                    this.pan.y += f32::from(delta.y);
                    this.last_mouse_pos = event.position;
                    cx.notify();
                }
            }))
            .on_mouse_up(
                MouseButton::Left,
                cx.listener(|this, _event: &MouseUpEvent, _window, _cx| {
                    this.is_panning = false;
                }),
            )
            .on_mouse_up_out(
                MouseButton::Left,
                cx.listener(|this, _event: &MouseUpEvent, _window, _cx| {
                    this.is_panning = false;
                }),
            )
            .child(
                canvas(
                    move |_bounds, window, _cx| {
                        let edges: Vec<Path<Pixels>> = edges
                            .iter()
                            .map(|(source, target)| Self::draw_edge(*source, *target, pan, zoom))
                            .collect();
                        let labels: Vec<(ShapedLine, Point<f32>)> = nodes_render_data
                            .iter()
                            .map(|node| {
                                let run = TextRun {
                                    len: node.0.len(),
                                    font: Font {
                                        family: label_font_family.clone(),
                                        features: FontFeatures::default(),
                                        fallbacks: None,
                                        weight: FontWeight::default(),
                                        style: FontStyle::default(),
                                    },
                                    color: label_color,
                                    background_color: None,
                                    underline: None,
                                    strikethrough: None,
                                };
                                let shaped_line = window.text_system().shape_line(
                                    node.0.clone(),
                                    px(LABEL_FONT_SIZE * zoom),
                                    &[run],
                                    None,
                                );
                                (shaped_line, node.1)
                            })
                            .collect();
                        let vertices: Vec<Path<Pixels>> = nodes_render_data
                            .iter()
                            .map(|node| node.1)
                            .map(|position| Self::draw_vertex(position, VERTEX_RADIUS, pan, zoom))
                            .collect();

                        (vertices, edges, labels)
                    },
                    move |_bounds, (vertices, edges, labels), window, cx| {
                        for path in edges {
                            window.paint_path(path, edges_color);
                        }
                        for path in vertices {
                            window.paint_path(path, vertices_color);
                        }
                        for (shaped_line, position) in labels {
                            let shaped_line_width = f32::from(shaped_line.width());
                            let screen_position = Self::to_pixels(position, pan, zoom);
                            let label_position = Point {
                                x: screen_position.x - px(shaped_line_width / 2.0),
                                y: screen_position.y + px(LABEL_PADDING * zoom),
                            };
                            let _ = shaped_line.paint(
                                label_position,
                                px(16. * zoom),
                                TextAlign::Left,
                                None,
                                window,
                                cx,
                            );
                        }
                    },
                )
                .size_full(),
            )
    }
}
