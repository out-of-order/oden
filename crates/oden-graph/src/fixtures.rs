use crate::simulation::{Graph, Node};
use gpui::Point;
use std::collections::HashMap;
use uuid::Uuid;

fn make_nodes(size: i32) -> (HashMap<Uuid, Node>, Vec<Uuid>) {
    let mut nodes = HashMap::new();
    let mut ids = vec![];
    for _ in 0..size {
        let id = Uuid::new_v4();
        ids.push(id);
        nodes.insert(
            id,
            Node {
                position: Point::default(),
                velocity: Point::default(),
            },
        );
    }
    (nodes, ids)
}

pub fn chain(ids: &[Uuid]) -> HashMap<Uuid, Vec<Uuid>> {
    let mut adj: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for i in 0..ids.len().saturating_sub(1) {
        adj.entry(ids[i]).or_default().push(ids[i + 1]);
    }
    adj
}

pub fn ring(ids: &[Uuid]) -> HashMap<Uuid, Vec<Uuid>> {
    let mut adj = chain(ids);
    let n = ids.len();
    if n > 1 {
        adj.entry(ids[n - 1]).or_default().push(ids[0]);
    }
    adj
}

pub fn fully_connected(ids: &[Uuid]) -> HashMap<Uuid, Vec<Uuid>> {
    let mut adj: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            adj.entry(ids[i]).or_default().push(ids[j]);
        }
    }
    adj
}

pub fn star(ids: &[Uuid]) -> HashMap<Uuid, Vec<Uuid>> {
    let mut adj: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for spoke in &ids[1..] {
        adj.entry(ids[0]).or_default().push(*spoke);
    }
    adj
}

pub fn construct_graph(size: i32, shape: fn(&[Uuid]) -> HashMap<Uuid, Vec<Uuid>>) -> Graph {
    let (nodes, ids) = make_nodes(size);
    Graph {
        nodes,
        adjacency_list: shape(&ids),
    }
}
