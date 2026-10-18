use std::collections::HashMap;
use std::iter;

use gpui::Point;
use uuid::Uuid;

use crate::{
    quadtree::QuadTreeKind::{Cell, Leaf},
    rng::Lcg,
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
pub(crate) struct QuadTree<'a> {
    bounds: Rect,
    depth: usize,
    kind: QuadTreeKind<'a>,
    graph: &'a Graph,
    // aggregated by `accumulate_charge`: the summed strength ("mass") of
    // every node beneath this cell, and the strength-weighted centroid of
    // their positions. Used by the many-body Barnes-Hut approximation.
    charge: f32,
    centroid: Point<f32>,
    // aggregated by `accumulate_radius`: the largest collision radius of
    // any node beneath this cell. Used to prune the collision traversal.
    max_radius: f32,
}

#[derive(Debug)]
enum QuadTreeKind<'a> {
    Leaf(Vec<Uuid>),
    Cell([Box<QuadTree<'a>>; 4]),
}

impl<'a> QuadTree<'a> {
    pub(crate) fn new(graph: &'a Graph) -> Self {
        let mut quad_tree = QuadTree {
            bounds: Rect::default(),
            depth: 0,
            kind: Leaf(Vec::new()),
            graph,
            charge: 0.0,
            centroid: Point::default(),
            max_radius: 0.0,
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
                        charge: 0.0,
                        centroid: Point::default(),
                        max_radius: 0.0,
                    };
                    let mut north_east = QuadTree {
                        kind: Leaf(Vec::new()),
                        bounds: north_east_bounds,
                        depth: depth + 1,
                        graph,
                        charge: 0.0,
                        centroid: Point::default(),
                        max_radius: 0.0,
                    };
                    let mut south_west = QuadTree {
                        kind: Leaf(Vec::new()),
                        bounds: south_west_bounds,
                        depth: depth + 1,
                        graph,
                        charge: 0.0,
                        centroid: Point::default(),
                        max_radius: 0.0,
                    };
                    let mut south_east = QuadTree {
                        kind: Leaf(Vec::new()),
                        bounds: south_east_bounds,
                        depth: depth + 1,
                        graph,
                        charge: 0.0,
                        centroid: Point::default(),
                        max_radius: 0.0,
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
                        charge: 0.0,
                        centroid: Point::default(),
                        max_radius: 0.0,
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
        max_x = max_x.next_up();
        max_y = max_y.next_up();
        let size = (max_x - min_x).max(max_y - min_y);
        self.bounds = Rect {
            min_x,
            min_y,
            max_x: min_x + size,
            max_y: min_y + size,
        }
    }

    pub(crate) fn accumulate_charge(&mut self, strengths: &HashMap<Uuid, f32>) {
        match &mut self.kind {
            Leaf(nodes) => {
                let mut charge = 0.0;
                let mut weight = 0.0;
                let mut centroid = Point::default();
                for id in nodes.iter() {
                    let strength = strengths.get(id).copied().unwrap_or(0.0);
                    let position = self.graph.nodes.get(id).unwrap().position;
                    charge += strength;
                    let w = strength.abs();
                    weight += w;
                    centroid += position * w;
                }
                self.charge = charge;
                self.centroid = if weight > 0.0 {
                    centroid / weight
                } else {
                    Point::default()
                };
            }
            Cell(children) => {
                let mut charge = 0.0;
                let mut weight = 0.0;
                let mut centroid = Point::default();
                for child in children.iter_mut() {
                    child.accumulate_charge(strengths);
                    charge += child.charge;
                    let w = child.charge.abs();
                    weight += w;
                    centroid += child.centroid * w;
                }
                self.charge = charge;
                self.centroid = if weight > 0.0 {
                    centroid / weight
                } else {
                    Point::default()
                };
            }
        }
    }

    pub(crate) fn accumulate_radius(&mut self, radii: &HashMap<Uuid, f32>) {
        match &mut self.kind {
            Leaf(nodes) => {
                self.max_radius = nodes
                    .iter()
                    .map(|id| radii.get(id).copied().unwrap_or(0.0))
                    .fold(0.0, f32::max);
            }
            Cell(children) => {
                let mut max_radius: f32 = 0.0;
                for child in children.iter_mut() {
                    child.accumulate_radius(radii);
                    max_radius = max_radius.max(child.max_radius);
                }
                self.max_radius = max_radius;
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_charge(
        &self,
        exclude: Uuid,
        position: Point<f32>,
        strengths: &HashMap<Uuid, f32>,
        theta2: f32,
        distance_min2: f32,
        distance_max2: f32,
        rng: &mut Lcg,
    ) -> Point<f32> {
        if self.charge == 0.0 {
            return Point::default();
        }

        match &self.kind {
            Cell(children) => {
                let dx = self.centroid.x - position.x;
                let dy = self.centroid.y - position.y;
                let width = self.bounds.max_x - self.bounds.min_x;
                let l = dx * dx + dy * dy;

                if width * width / theta2 < l {
                    // Far enough: approximate the whole cell as a single
                    // mass at its centroid, and don't descend further.
                    if l >= distance_max2 {
                        return Point::default();
                    }
                    let mut dx = dx;
                    let mut dy = dy;
                    let mut l = l;
                    jiggle_if_zero(&mut dx, &mut dy, &mut l, rng);
                    if l < distance_min2 {
                        l = (distance_min2 * l).sqrt();
                    }
                    return Point { x: dx, y: dy } * (self.charge / l);
                }

                let mut total = Point::default();
                for child in children.iter() {
                    total += child.apply_charge(
                        exclude,
                        position,
                        strengths,
                        theta2,
                        distance_min2,
                        distance_max2,
                        rng,
                    );
                }
                total
            }
            Leaf(nodes) => {
                let mut total = Point::default();
                for id in nodes.iter() {
                    if *id == exclude {
                        continue;
                    }
                    let other = self.graph.nodes.get(id).unwrap().position;
                    let mut dx = other.x - position.x;
                    let mut dy = other.y - position.y;
                    let mut l = dx * dx + dy * dy;
                    if l >= distance_max2 {
                        continue;
                    }
                    jiggle_if_zero(&mut dx, &mut dy, &mut l, rng);
                    if l < distance_min2 {
                        l = (distance_min2 * l).sqrt();
                    }
                    let strength = strengths.get(id).copied().unwrap_or(0.0);
                    total += Point { x: dx, y: dy } * (strength / l);
                }
                total
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn apply_collisions(
        &self,
        exclude: Uuid,
        position: Point<f32>,
        own_radius: f32,
        radii: &HashMap<Uuid, f32>,
        strength: f32,
        rng: &mut Lcg,
        deltas: &mut HashMap<Uuid, Point<f32>>,
    ) {
        let reach = own_radius + self.max_radius;
        if self.bounds.min_x > position.x + reach
            || self.bounds.max_x < position.x - reach
            || self.bounds.min_y > position.y + reach
            || self.bounds.max_y < position.y - reach
        {
            return;
        }

        match &self.kind {
            Cell(children) => {
                for child in children.iter() {
                    child.apply_collisions(
                        exclude, position, own_radius, radii, strength, rng, deltas,
                    );
                }
            }
            Leaf(nodes) => {
                for id in nodes.iter() {
                    if *id <= exclude {
                        continue;
                    }
                    let other_position = self.graph.nodes.get(id).unwrap().position;
                    let other_radius = radii.get(id).copied().unwrap_or(0.0);
                    let r = own_radius + other_radius;
                    let mut dx = position.x - other_position.x;
                    let mut dy = position.y - other_position.y;
                    let mut l = dx * dx + dy * dy;
                    if l >= r * r {
                        continue;
                    }
                    jiggle_if_zero(&mut dx, &mut dy, &mut l, rng);
                    let distance = l.sqrt();
                    let factor = (r - distance) / distance * strength;
                    dx *= factor;
                    dy *= factor;

                    let own_r2 = own_radius * own_radius;
                    let other_r2 = other_radius * other_radius;
                    let total_r2 = own_r2 + other_r2;
                    let (own_share, other_share) = if total_r2 > 0.0 {
                        (other_r2 / total_r2, own_r2 / total_r2)
                    } else {
                        (0.5, 0.5)
                    };

                    let delta = Point { x: dx, y: dy };
                    *deltas.entry(exclude).or_default() += delta * own_share;
                    *deltas.entry(*id).or_default() -= delta * other_share;
                }
            }
        }
    }
}

fn jiggle_if_zero(dx: &mut f32, dy: &mut f32, l: &mut f32, rng: &mut Lcg) {
    if *dx == 0.0 {
        *dx = rng.jiggle();
        *l += *dx * *dx;
    }
    if *dy == 0.0 {
        *dy = rng.jiggle();
        *l += *dy * *dy;
    }
}

impl Rect {
    fn contains_point(&self, x: f32, y: f32) -> bool {
        x >= self.min_x && x < self.max_x && y >= self.min_y && y < self.max_y
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::{assert_matches, panic};

    use gpui::Point;
    use uuid::Uuid;

    use crate::{
        fixtures::graph_with_positions,
        quadtree::{
            QuadTree,
            QuadTreeKind::{self, Leaf},
            Rect,
        },
        rng::Lcg,
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

    #[test]
    fn accumulates_charge_and_centroid_for_a_leaf() {
        let graph = graph_with_positions(&[Point { x: 0., y: 0. }, Point { x: 10., y: 0. }]);
        let mut tree = QuadTree::new(&graph);
        let strengths: HashMap<Uuid, f32> = graph.nodes.keys().map(|id| (*id, -10.0)).collect();
        tree.accumulate_charge(&strengths);
        assert_eq!(tree.charge, -20.0);
        assert_eq!(tree.centroid, Point { x: 5., y: 0. });
    }

    #[test]
    fn accumulates_charge_and_centroid_across_cells() {
        let graph = graph_with_positions(&[
            Point { x: 0., y: 10. },
            Point { x: 3., y: 4. },
            Point { x: 6., y: 7. },
        ]);
        let mut tree = QuadTree::new(&graph);
        let strengths: HashMap<Uuid, f32> = graph.nodes.keys().map(|id| (*id, -5.0)).collect();
        tree.accumulate_charge(&strengths);
        assert_eq!(tree.charge, -15.0);
        // all three nodes carry equal (negative) strength, so the centroid
        // is their unweighted average position.
        assert_eq!(tree.centroid, Point { x: 3., y: 7. });
    }

    #[test]
    fn accumulates_max_radius_across_cells() {
        let graph = graph_with_positions(&[
            Point { x: 0., y: 10. },
            Point { x: 3., y: 4. },
            Point { x: 6., y: 7. },
        ]);
        let mut tree = QuadTree::new(&graph);
        let mut radii: HashMap<Uuid, f32> = graph.nodes.keys().map(|id| (*id, 1.0)).collect();
        let bumped_id = *graph.nodes.keys().next().unwrap();
        radii.insert(bumped_id, 9.0);
        tree.accumulate_radius(&radii);
        assert_eq!(tree.max_radius, 9.0);
    }

    #[test]
    fn apply_charge_repels_along_the_line_between_two_nodes() {
        let graph = graph_with_positions(&[Point { x: 0., y: 0. }, Point { x: 10., y: 0. }]);
        let mut tree = QuadTree::new(&graph);
        let strengths: HashMap<Uuid, f32> = graph.nodes.keys().map(|id| (*id, -30.0)).collect();
        tree.accumulate_charge(&strengths);
        let mut rng = Lcg::new();

        // find the node sitting at the origin; it should be pushed in the
        // negative-x direction, away from the other node at (10, 0).
        let (source_id, source_position) = graph
            .nodes
            .iter()
            .map(|(id, node)| (*id, node.position))
            .find(|(_, position)| *position == Point { x: 0., y: 0. })
            .unwrap();
        let delta = tree.apply_charge(
            source_id,
            source_position,
            &strengths,
            0.81,
            1.0,
            f32::INFINITY,
            &mut rng,
        );
        // negative strength => repulsion => pushed away from the target,
        // i.e. towards negative x. y is nudged by a (tiny) deterministic
        // jiggle since the two points are exactly colinear.
        assert!(delta.x < 0.0);
        assert!(delta.y.abs() < 1e-4);
    }

    #[test]
    fn apply_collisions_pushes_overlapping_nodes_apart() {
        let graph = graph_with_positions(&[Point { x: 0., y: 0. }, Point { x: 4., y: 0. }]);
        let ids: Vec<Uuid> = graph.nodes.keys().copied().collect();
        let radii: HashMap<Uuid, f32> = graph.nodes.keys().map(|id| (*id, 5.0)).collect();
        let mut tree = QuadTree::new(&graph);
        tree.accumulate_radius(&radii);
        let mut rng = Lcg::new();
        let mut deltas: HashMap<Uuid, Point<f32>> = HashMap::new();
        for id in &ids {
            let position = graph.nodes.get(id).unwrap().position;
            tree.apply_collisions(*id, position, 5.0, &radii, 1.0, &mut rng, &mut deltas);
        }

        // identify nodes by their actual position rather than id ordering,
        // since id-to-position assignment is random.
        let left_id = ids
            .iter()
            .copied()
            .find(|id| graph.nodes.get(id).unwrap().position == Point { x: 0., y: 0. })
            .unwrap();
        let right_id = ids.into_iter().find(|id| *id != left_id).unwrap();

        // the two 5-radius circles are only 4 apart, so they overlap and
        // should be pushed apart symmetrically along the x axis.
        assert!(deltas[&left_id].x < 0.0);
        assert!(deltas[&right_id].x > 0.0);
        assert_eq!(deltas[&left_id].x, -deltas[&right_id].x);
    }
}
