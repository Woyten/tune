use std::{ops::Range, sync::Arc};

use bevy::prelude::Resource;
use tune::{
    note::NoteLetter,
    pitch::{Pitch, Pitched},
    scala::Scl,
};

use crate::{
    piano::{PianoEngine, PianoEngineState},
    KeyColor,
};

#[derive(Resource)]
pub struct Model {
    pub engine: Arc<PianoEngine>,
    pub key_colors: Vec<KeyColor>,
    pub reference_scl: Scl,
    pub odd_limit: u16,
}

#[derive(Resource)]
pub struct PianoEngineResource(pub PianoEngineState);

#[derive(Resource)]
pub struct Viewport {
    pub pitch_range: Range<Pitch>,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            pitch_range: NoteLetter::Fsh.in_octave(2).pitch()..NoteLetter::Ash.in_octave(5).pitch(),
        }
    }
}
