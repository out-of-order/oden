use std::iter;

use gpui::Point;
use uuid::Uuid;

use crate::{
    quadtree::QuadTreeKind::{Cell, Leaf},
    simulation::Graph,
};

const LEAF_CAPACITY: usize = 2;
const MAX_DEPTH: usize = 32;

#[derive(Debug, Default, PartialEq, Copy, Clone)]
struct Rect {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

#[derive(Debug)]
struct QuadTree<'a> {
    bounds: Rect,
    depth: usize,
    kind: QuadTreeKind<'a>,
    graph: &'a Graph,
}

#[derive(Debug)]
enum QuadTreeKind<'a> {
    Leaf(Vec<Uuid>),
    Cell([Box<QuadTree<'a>>; 4]),
}

impl<'a> QuadTree<'a> {
    fn new(graph: &'a Graph) -> Self {
        let mut quad_tree = QuadTree {
            bounds: Rect::default(),
            depth: 0,
            kind: Leaf(Vec::new()),
            graph,
        };
        quad_tree.set_boundaries();
        quad_tree.set_nodes();
        quad_tree
    }

    fn set_nodes(&mut self) {
        for node in self.graph.nodes.keys() {
            self.push_node(*node);
        }
    }

    fn push_node(&mut self, node: Uuid) {
        let bounds = self.bounds;
        let depth = self.depth;
        let graph = self.graph;

        match &mut self.kind {
            Leaf(nodes) => {
                if nodes.len() < LEAF_CAPACITY || depth >= MAX_DEPTH {
                    nodes.push(node);
                } else {
                    let nodes_to_redistribute = std::mem::take(nodes);
                    let mid_point = (
                        bounds.min_x + (bounds.max_x - bounds.min_x) / 2.0,
                        bounds.min_y + (bounds.max_y - bounds.min_y) / 2.0,
                    );
                    let north_west_bounds = Rect {
                        min_x: bounds.min_x,
                        min_y: bounds.min_y,
                        max_x: mid_point.0,
                        max_y: mid_point.1,
                    };
                    let north_east_bounds = Rect {
                        min_x: mid_point.0,
                        min_y: bounds.min_y,
                        max_x: bounds.max_x,
                        max_y: mid_point.1,
                    };
                    let south_west_bounds = Rect {
                        min_x: bounds.min_x,
                        min_y: mid_point.1,
                        max_x: mid_point.0,
                        max_y: bounds.max_y,
                    };
                    let south_east_bounds = Rect {
                        min_x: mid_point.0,
                        min_y: mid_point.1,
                        max_x: bounds.max_x,
                        max_y: bounds.max_y,
                    };
                    let mut north_west = QuadTree {
                        kind: Leaf(Vec::new()),
                        bounds: north_west_bounds,
                        depth: depth + 1,
                        graph,
                    };
                    let mut north_east = QuadTree {
                        kind: Leaf(Vec::new()),
                        bounds: north_east_bounds,
                        depth: depth + 1,
                        graph,
                    };
                    let mut south_west = QuadTree {
                        kind: Leaf(Vec::new()),
                        bounds: south_west_bounds,
                        depth: depth + 1,
                        graph,
                    };
                    let mut south_east = QuadTree {
                        kind: Leaf(Vec::new()),
                        bounds: south_east_bounds,
                        depth: depth + 1,
                        graph,
                    };

                    // redistribute the nodes into the quadrants.
                    for node_id in nodes_to_redistribute.into_iter().chain(iter::once(node)) {
                        let node_position = graph.nodes.get(&node_id).unwrap().position;
                        if north_east
                            .bounds
                            .contains_point(node_position.x, node_position.y)
                        {
                            north_east.push_node(node_id);
                        } else if north_west
                            .bounds
                            .contains_point(node_position.x, node_position.y)
                        {
                            north_west.push_node(node_id);
                        } else if south_west
                            .bounds
                            .contains_point(node_position.x, node_position.y)
                        {
                            south_west.push_node(node_id);
                        } else if south_east
                            .bounds
                            .contains_point(node_position.x, node_position.y)
                        {
                            south_east.push_node(node_id);
                        }
                    }
                    *self = QuadTree {
                        kind: Cell([
                            Box::new(north_east),
                            Box::new(north_west),
                            Box::new(south_east),
                            Box::new(south_west),
                        ]),
                        bounds,
                        depth,
                        graph,
                    }
                }
            }
            Cell(quadrants) => {
                let Point { x, y } = graph.nodes.get(&node).unwrap().position;
                if quadrants[0].bounds.contains_point(x, y) {
                    quadrants[0].push_node(node);
                } else if quadrants[1].bounds.contains_point(x, y) {
                    quadrants[1].push_node(node);
                } else if quadrants[2].bounds.contains_point(x, y) {
                    quadrants[2].push_node(node);
                } else if quadrants[3].bounds.contains_point(x, y) {
                    quadrants[3].push_node(node);
                }
            }
        }
    }

    fn set_boundaries(&mut self) {
        let Some(first) = self.graph.nodes.values().next() else {
            return;
        };
        let (mut min_x, mut min_y, mut max_x, mut max_y) = (
            first.position.x,
            first.position.y,
            first.position.x,
            first.position.y,
        );
        for node in self.graph.nodes.values() {
            min_x = min_x.min(node.position.x);
            min_y = min_y.min(node.position.y);
            max_x = max_x.max(node.position.x);
            max_y = max_y.max(node.position.y);
        }
        self.bounds = Rect {
            min_x,
            min_y,
            max_x: max_x.next_up(),
            max_y: max_y.next_up(),
        }
    }
}

impl Rect {
    fn contains_point(&self, x: f32, y: f32) -> bool {
        x >= self.min_x && x < self.max_x && y >= self.min_y && y < self.max_y
    }
}

#[cfg(test)]
mod tests {
    use std::{assert_matches, panic};

    use gpui::Point;

    use crate::{
        fixtures::graph_with_positions,
        quadtree::{
            QuadTree,
            QuadTreeKind::{self, Leaf},
            Rect,
        },
    };

    #[test]
    fn computes_boundaries_correctly() {
        let graph = graph_with_positions(&[Point { x: 0., y: 10. }, Point { x: 10., y: 0. }]);
        let tree = QuadTree::new(&graph);
        let expected_bounds = Rect {
            min_x: 0.,
            min_y: 0.,
            max_x: f32::next_up(10.),
            max_y: f32::next_up(10.),
        };
        assert_eq!(tree.bounds, expected_bounds)
    }

    #[test]
    fn root_is_leaf_when_capacity_is_not_exceeded() {
        let graph = graph_with_positions(&[Point { x: 0., y: 10. }, Point { x: 10., y: 0. }]);
        let tree = QuadTree::new(&graph);
        assert_matches!(tree.kind, QuadTreeKind::Leaf(_));
    }

    #[test]
    fn root_is_cell_and_nodes_are_distributed_when_capacity_is_exceeded() {
        let graph = graph_with_positions(&[
            Point { x: 0., y: 10. },
            Point { x: 3., y: 4. },
            Point { x: 6., y: 7. },
        ]);
        let tree = QuadTree::new(&graph);
        let QuadTreeKind::Cell(quadrants) = tree.kind else {
            panic!("kind should be cell")
        };

        let assert_leaf_contains = |quadrant: &QuadTree, expected_position: Point<f32>| {
            let QuadTreeKind::Leaf(nodes) = &quadrant.kind else {
                panic!("quadrant should be a leaf");
            };
            assert_eq!(nodes.len(), 1);
            assert_eq!(
                graph.nodes.get(&nodes[0]).unwrap().position,
                expected_position
            );
        };

        assert_leaf_contains(&quadrants[0], Point { x: 6., y: 7. });
        assert_leaf_contains(&quadrants[1], Point { x: 3., y: 4. });
        assert_matches!(&quadrants[2].kind, Leaf(nodes) if nodes.is_empty());
        assert_leaf_contains(&quadrants[3], Point { x: 0., y: 10. });
    }
}
