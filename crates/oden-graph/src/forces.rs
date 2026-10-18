use std::collections::HashMap;

use gpui::Point;
use uuid::Uuid;

use crate::{quadtree::QuadTree, rng::Lcg, simulation::Graph};

pub trait Force {
    // modifies the node.velocity by applying said force to the node.
    // force contribution is multiplied by alpha.
    fn apply_force(&mut self, alpha: f32, nodes: &mut Graph);
}

// ----- Link Force ------
pub struct LinkForce {
    pub x_rest: f32,
    pub spring_constant: f32,
    strengths: HashMap<Uuid, f32>,
}

impl LinkForce {
    fn init_strengths(&mut self, nodes: &mut Graph) {
        let mut degree_map: HashMap<Uuid, f32> = HashMap::new();
        for (source, targets) in &nodes.adjacency_list {
            *degree_map.entry(*source).or_insert(0.0) += targets.len() as f32;
            for target in targets {
                *degree_map.entry(*target).or_insert(0.0) += 1.0;
            }
        }
        for (id, degree) in degree_map {
            self.strengths.insert(id, 1.0 / degree);
        }
    }
    pub fn new() -> Self {
        Self {
            x_rest: 120.,
            spring_constant: 1.,
            strengths: HashMap::new(),
        }
    }
}

impl Default for LinkForce {
    fn default() -> Self {
        Self::new()
    }
}

impl Force for LinkForce {
    fn apply_force(&mut self, alpha: f32, nodes: &mut Graph) {
        self.init_strengths(nodes);
        for source in nodes.adjacency_list.keys() {
            for target in nodes.adjacency_list.get(source).unwrap() {
                let (source_node, target_node) = (
                    nodes.nodes.get(source).unwrap(),
                    nodes.nodes.get(target).unwrap(),
                );
                let x = target_node.position.x + target_node.velocity.x
                    - source_node.position.x
                    - source_node.velocity.x;
                let y = target_node.position.y + target_node.velocity.y
                    - source_node.position.y
                    - source_node.velocity.y;
                let l = (x * x + y * y).sqrt();
                if l == 0.0 {
                    continue;
                }
                let unit_vector = Point { x, y } / l;
                // Hooke's law - k (x - x_rest) u where u is the unit vector
                let force_vector =
                    unit_vector * (-(l - self.x_rest) * self.spring_constant * alpha);

                // calculate bias
                let strength_source = self.strengths.get(source).unwrap();
                let strength_target = self.strengths.get(target).unwrap();
                let bias = strength_source / (strength_source + strength_target);

                // apply the force on target.
                {
                    let target = nodes.nodes.get_mut(target).unwrap();
                    target.velocity += force_vector * bias;
                }
                {
                    let source = nodes.nodes.get_mut(source).unwrap();
                    source.velocity -= force_vector * (1.0 - bias);
                }
            }
        }
    }
}

// ----- Many-Body Force ------
// A Barnes-Hut approximated n-body force (repulsion by default, since
// `strength` defaults negative), equivalent to d3-force's `forceManyBody`.
pub struct ManyBodyForce {
    pub strength: f32,
    pub theta: f32,
    pub distance_min: f32,
    pub distance_max: f32,
    strengths: HashMap<Uuid, f32>,
    rng: Lcg,
}

impl ManyBodyForce {
    pub fn new() -> Self {
        Self {
            strength: -30.,
            theta: 0.9,
            distance_min: 1.,
            distance_max: f32::INFINITY,
            strengths: HashMap::new(),
            rng: Lcg::new(),
        }
    }

    fn init_strengths(&mut self, nodes: &Graph) {
        self.strengths = nodes.nodes.keys().map(|id| (*id, self.strength)).collect();
    }
}

impl Default for ManyBodyForce {
    fn default() -> Self {
        Self::new()
    }
}

impl Force for ManyBodyForce {
    fn apply_force(&mut self, alpha: f32, nodes: &mut Graph) {
        if nodes.nodes.len() < 2 {
            return;
        }
        self.init_strengths(nodes);

        let mut tree = QuadTree::new(nodes);
        tree.accumulate_charge(&self.strengths);

        let theta2 = self.theta * self.theta;
        let distance_min2 = self.distance_min * self.distance_min;
        let distance_max2 = self.distance_max * self.distance_max;

        let mut deltas: HashMap<Uuid, Point<f32>> = HashMap::with_capacity(nodes.nodes.len());
        for (id, node) in nodes.nodes.iter() {
            let delta = tree.apply_charge(
                *id,
                node.position,
                &self.strengths,
                theta2,
                distance_min2,
                distance_max2,
                &mut self.rng,
            );
            deltas.insert(*id, delta);
        }

        for (id, delta) in deltas {
            nodes.nodes.get_mut(&id).unwrap().velocity += delta * alpha;
        }
    }
}

// ----- Collide Force ------
// Keeps nodes from overlapping by treating each one as a circle of
// `Node::radius` and pushing overlapping pairs apart, equivalent to
// d3-force's `forceCollide`. Note that, like d3's collide force, this is
// *not* scaled by `alpha`: collisions should keep resolving at full
// strength even as the simulation cools down.
pub struct CollideForce {
    pub strength: f32,
    pub iterations: usize,
    rng: Lcg,
}

impl CollideForce {
    pub fn new() -> Self {
        Self {
            strength: 1.,
            iterations: 1,
            rng: Lcg::new(),
        }
    }
}

impl Default for CollideForce {
    fn default() -> Self {
        Self::new()
    }
}

impl Force for CollideForce {
    fn apply_force(&mut self, _alpha: f32, nodes: &mut Graph) {
        if nodes.nodes.len() < 2 {
            return;
        }
        let radii: HashMap<Uuid, f32> = nodes
            .nodes
            .iter()
            .map(|(id, node)| (*id, node.radius.max(0.0)))
            .collect();

        for _ in 0..self.iterations.max(1) {
            let mut tree = QuadTree::new(nodes);
            tree.accumulate_radius(&radii);

            let mut deltas: HashMap<Uuid, Point<f32>> = HashMap::new();
            for (id, node) in nodes.nodes.iter() {
                let own_radius = radii.get(id).copied().unwrap_or(0.0);
                tree.apply_collisions(
                    *id,
                    node.position,
                    own_radius,
                    &radii,
                    self.strength,
                    &mut self.rng,
                    &mut deltas,
                );
            }

            for (id, delta) in deltas {
                nodes.nodes.get_mut(&id).unwrap().velocity += delta;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use gpui::{Point, SharedString};

    use crate::{
        fixtures::{construct_graph, fully_connected},
        forces::{CollideForce, Force, LinkForce, ManyBodyForce},
        simulation::{Graph, Node},
    };

    #[test]
    fn test_node_strengths() {
        let mut graph = construct_graph(4, fully_connected);
        let mut force = LinkForce::default();
        force.apply_force(1.0, &mut graph);
        assert!(
            force
                .strengths
                .values()
                .all(|s| (*s - 1. / 3.).abs() < f32::EPSILON)
        );
    }

    #[test]
    fn test_link_force() {
        let (source_id, source_node) = (uuid::Uuid::new_v4(), Point { x: 0., y: 0. });
        let (target_id, target_node) = (uuid::Uuid::new_v4(), Point { x: 200., y: 0. });
        let mut force = LinkForce::new();
        let adjacency_list =
            HashMap::from([(source_id, vec![target_id]), (target_id, vec![source_id])]);
        let nodes = HashMap::from([
            (
                source_id,
                Node {
                    position: source_node,
                    velocity: Point::default(),
                    label: SharedString::default(),
                    radius: 5.0,
                },
            ),
            (
                target_id,
                Node {
                    position: target_node,
                    velocity: Point::default(),
                    label: SharedString::default(),
                    radius: 5.0,
                },
            ),
        ]);
        let mut nodes: Graph = Graph {
            adjacency_list,
            nodes,
        };
        // unit vector is (1, 0). Force is - k (x - xrest) * alpha * bias = 200 - 120 / 2 = 40
        // bias is 1/2, each node has one neighbour.
        force.apply_force(1., &mut nodes);
        let source = nodes.nodes.get(&source_id).unwrap();
        assert_eq!(source.velocity, Point { x: 40., y: 0. });
        let target = nodes.nodes.get(&target_id).unwrap();
        assert_eq!(target.velocity, Point { x: -40., y: 0. });
    }

    #[test]
    fn test_many_body_force_repels_nodes_apart() {
        let (source_id, source_node) = (uuid::Uuid::new_v4(), Point { x: 0., y: 0. });
        let (target_id, target_node) = (uuid::Uuid::new_v4(), Point { x: 10., y: 0. });
        let nodes = HashMap::from([
            (
                source_id,
                Node {
                    position: source_node,
                    velocity: Point::default(),
                    label: SharedString::default(),
                    radius: 5.0,
                },
            ),
            (
                target_id,
                Node {
                    position: target_node,
                    velocity: Point::default(),
                    label: SharedString::default(),
                    radius: 5.0,
                },
            ),
        ]);
        let mut graph = Graph {
            adjacency_list: HashMap::new(),
            nodes,
        };
        let mut force = ManyBodyForce::default();
        force.apply_force(1., &mut graph);
        let source = graph.nodes.get(&source_id).unwrap();
        let target = graph.nodes.get(&target_id).unwrap();
        // default strength is negative (repulsion), so the two nodes should
        // be pushed apart, away from each other.
        assert!(source.velocity.x < 0.0);
        assert!(target.velocity.x > 0.0);
    }

    #[test]
    fn test_collide_force_pushes_overlapping_nodes_apart() {
        let (source_id, source_node) = (uuid::Uuid::new_v4(), Point { x: 0., y: 0. });
        let (target_id, target_node) = (uuid::Uuid::new_v4(), Point { x: 4., y: 0. });
        let nodes = HashMap::from([
            (
                source_id,
                Node {
                    position: source_node,
                    velocity: Point::default(),
                    label: SharedString::default(),
                    radius: 5.0,
                },
            ),
            (
                target_id,
                Node {
                    position: target_node,
                    velocity: Point::default(),
                    label: SharedString::default(),
                    radius: 5.0,
                },
            ),
        ]);
        let mut graph = Graph {
            adjacency_list: HashMap::new(),
            nodes,
        };
        let mut force = CollideForce::default();
        force.apply_force(1., &mut graph);
        let source = graph.nodes.get(&source_id).unwrap();
        let target = graph.nodes.get(&target_id).unwrap();
        // the two radius-5 circles are only 4 apart (overlapping), so they
        // should be pushed apart along the x axis, symmetrically.
        assert!(source.velocity.x < 0.0);
        assert!(target.velocity.x > 0.0);
        assert_eq!(source.velocity.x, -target.velocity.x);
    }

    #[test]
    fn test_collide_force_ignores_non_overlapping_nodes() {
        let (source_id, source_node) = (uuid::Uuid::new_v4(), Point { x: 0., y: 0. });
        let (target_id, target_node) = (uuid::Uuid::new_v4(), Point { x: 100., y: 0. });
        let nodes = HashMap::from([
            (
                source_id,
                Node {
                    position: source_node,
                    velocity: Point::default(),
                    label: SharedString::default(),
                    radius: 5.0,
                },
            ),
            (
                target_id,
                Node {
                    position: target_node,
                    velocity: Point::default(),
                    label: SharedString::default(),
                    radius: 5.0,
                },
            ),
        ]);
        let mut graph = Graph {
            adjacency_list: HashMap::new(),
            nodes,
        };
        let mut force = CollideForce::default();
        force.apply_force(1., &mut graph);
        let source = graph.nodes.get(&source_id).unwrap();
        let target = graph.nodes.get(&target_id).unwrap();
        assert_eq!(source.velocity, Point::default());
        assert_eq!(target.velocity, Point::default());
    }
}
