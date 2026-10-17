use crate::store::ItemStore;
use gpui::{
    Context, Font, FontFeatures, FontStyle, FontWeight, ShapedLine, SharedString, Subscription,
    TextAlign, TextRun,
};
use gpui::{ParentElement, Path, PathBuilder, Pixels, Point, Size, canvas};
use gpui::{Render, Styled, div, px};
use gpui_component::ActiveTheme;
use oden_graph::forces::LinkForce;
use oden_graph::simulation;
use oden_graph::simulation::{Graph, Simulation};
use std::collections::HashMap;

pub struct GraphView {
    simulation: Simulation,
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
            _store_sub,
        }
    }

    fn create_simulation_from_store(cx: &mut Context<GraphView>) -> Simulation {
        let items = cx.global::<ItemStore>().items.clone();
        let mut nodes = HashMap::new();
        let mut adjacency_list = HashMap::new();
        for (id, item) in items {
            nodes.insert(
                id,
                simulation::Node {
                    position: Point::default(),
                    velocity: Point::default(),
                    label: item.name.clone(),
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
        Simulation::new(graph).with_force(Box::new(LinkForce::default()))
    }

    fn to_pixels(point: Point<f32>, view_port: Size<Pixels>) -> Point<Pixels> {
        // TODO: handle edge cases
        // for now scaling is 1 to 1 from floats to pixels.
        Point {
            x: px(point.x) + view_port.width / 2.,
            y: px(point.y) + view_port.height / 2.,
        }
    }

    fn draw_vertex(point: Point<f32>, vertex_radius: f32, view_port: Size<Pixels>) -> Path<Pixels> {
        let point = Self::to_pixels(point, view_port);
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
        view_port: Size<Pixels>,
    ) -> Path<Pixels> {
        let source = Self::to_pixels(source, view_port);
        let target = Self::to_pixels(target, view_port);
        let mut builder = PathBuilder::stroke(px(1.5));
        builder.move_to(source);
        builder.line_to(target);
        builder.build().unwrap()
    }
}

const LABEL_PADDING: f32 = 4.;

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
        let nodes_render_data: Vec<(SharedString, Point<f32>)> =
            self.simulation.nodes_render_data();
        let edges = self.simulation.edges();
        div().flex_none().min_w(px(360.0)).w_full().h_full().child(
            canvas(
                move |_bounds, window, _cx| {
                    let edges: Vec<Path<Pixels>> = edges
                        .iter()
                        .map(|(source, target)| {
                            Self::draw_edge(*source, *target, window.viewport_size())
                        })
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
                                px(11.),
                                &[run],
                                None,
                            );
                            let shaped_line_width = f32::from(shaped_line.width());
                            (
                                shaped_line,
                                Point {
                                    x: node.1.x - shaped_line_width / 2.0,
                                    y: node.1.y + LABEL_PADDING,
                                },
                            )
                        })
                        .collect();
                    let vertices: Vec<Path<Pixels>> = nodes_render_data
                        .iter()
                        .map(|node| node.1)
                        .map(|position| Self::draw_vertex(position, 5.0, window.viewport_size()))
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
                        let _ = shaped_line.paint(
                            Self::to_pixels(position, window.viewport_size()),
                            px(16.),
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
