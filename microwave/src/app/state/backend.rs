use std::collections::BTreeMap;

use bevy::prelude::*;

#[derive(Default, Resource)]
pub struct BackendState {
    pub backend: String,
    pub program: Option<String>,
    pub bank: Option<String>,
    pub envelope: Option<String>,
    pub recorder_details: BTreeMap<usize, String>,
}
