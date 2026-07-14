use crate::simulation::Graph;

pub trait Force {
    // modifies the node.velocity by applying said force to the node.
    // force contribution is multiplied by alpha.
    fn apply_force(&self, alpha: f32, nodes: &mut Graph);
}
