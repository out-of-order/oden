use std::collections::HashMap;

use gpui::{App, Global};
use uuid::Uuid;

use crate::{
    appstatus::Field,
    persistence::PersistenceStatus::{Failed, Idle, Saving},
};

#[derive(Clone, Copy, Debug)]
pub enum PersistenceStatus {
    Saving,
    Failed,
    Idle,
}

impl PersistenceStatus {
    pub fn merge(&self, persistence_state: &PersistenceStatus) -> PersistenceStatus {
        match (self, persistence_state) {
            (Failed, _) | (_, Failed) => Failed,
            (Saving, _) | (_, Saving) => Saving,
            _ => Idle,
        }
    }
}

pub struct PersistencePerNote(pub HashMap<(Uuid, Field), PersistenceStatus>);

impl Global for PersistencePerNote {}

impl PersistencePerNote {
    pub fn init(cx: &mut App) {
        cx.set_global::<PersistencePerNote>(PersistencePerNote(HashMap::new()));
    }
}
