use std::collections::HashMap;

use gpui::Point;
use uuid::Uuid;

use crate::simulation::Graph;

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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use gpui::Point;

    use crate::{
        fixtures::{construct_graph, fully_connected},
        forces::{Force, LinkForce},
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
                },
            ),
            (
                target_id,
                Node {
                    position: target_node,
                    velocity: Point::default(),
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
}
