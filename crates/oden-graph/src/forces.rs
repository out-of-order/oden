use std::collections::HashMap;

use gpui::Point;
use uuid::Uuid;

use crate::simulation::Graph;

pub trait Force {
    // modifies the node.velocity by applying said force to the node.
    // force contribution is multiplied by alpha.
    fn apply_force(&mut self, alpha: f32, nodes: &mut Graph);
}

pub struct LinkForce {
    pub x_rest: f32,
    pub hooks_constant: f32,
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
            hooks_constant: 1.,
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
                // hooks law - k (x - x_rest) u where u is the unit vector
                let force_vector = unit_vector * (-(l - self.x_rest) * self.hooks_constant * alpha);

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
