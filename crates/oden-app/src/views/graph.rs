use gpui::{ParentElement, Path, PathBuilder, Pixels, Point, Size, canvas};
use gpui::{Render, Styled, div, px};
use gpui_component::ActiveTheme;
use oden_graph::simulation::{Graph, Node, Simulation};
use std::collections::HashMap;
use uuid::Uuid;

pub struct GraphView {
    simulation: Simulation,
}

#[cfg(debug_assertions)]
pub fn construct_graph(size: i32) -> Graph {
    let mut nodes: HashMap<uuid::Uuid, Node> = HashMap::new();
    for _ in 0..size {
        nodes.insert(
            Uuid::new_v4(),
            Node {
                position: Point::default(),
                velocity: Point::default(),
            },
        );
    }
    Graph {
        nodes,
        adjacency_list: HashMap::new(),
    }
}

impl GraphView {
    pub fn new() -> Self {
        let graph: Graph = construct_graph(5);
        let simulation = Simulation::new(graph);
        Self { simulation }
    }

    fn to_pixels(point: Point<f32>, view_port: Size<Pixels>) -> Point<Pixels> {
        // TODO: handle edge cases
        Point {
            x: px(point.x) + view_port.width / 2.,
            y: px(point.y) + view_port.height / 2.,
        }
    }

    fn draw_vertice(
        point: Point<f32>,
        vertice_radius: f32,
        view_port: Size<Pixels>,
    ) -> Path<Pixels> {
        let point = Self::to_pixels(point, view_port);
        let radius_vector = Point {
            x: px(vertice_radius),
            y: px(0.),
        };
        let radii = Point {
            x: px(vertice_radius),
            y: px(vertice_radius),
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
}

impl Render for GraphView {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::prelude::Context<Self>,
    ) -> impl gpui::prelude::IntoElement {
        let color = cx.theme().red;
        if !self.simulation.should_stop {
            self.simulation.tick();
            window.request_animation_frame();
        }
        let positions = self.simulation.positions();
        div()
            .w_full()
            .h_full()
            .border(px(1.0))
            .border_color(color)
            .child(
                canvas(
                    move |_bounds, window, _cx| {
                        let paths: Vec<Path<Pixels>> = positions
                            .iter()
                            .map(|position| {
                                Self::draw_vertice(*position, 10.0, window.viewport_size())
                            })
                            .collect();
                        paths
                    },
                    move |_bounds, paths, window, _cx| {
                        for path in paths {
                            window.paint_path(path, color);
                        }
                    },
                )
                .size_full()
                .border(px(1.0))
                .border_color(color),
            )
    }
}
