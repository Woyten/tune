use std::{ops::Range, sync::Arc};

use bevy::prelude::Resource;
use tune::{
    key::{Keyboard, PianoKey},
    note::NoteLetter,
    pitch::{Pitch, Pitched},
    scala::Scl,
};

use crate::{keyboard::KeyboardLayout, piano::PianoEngine, KeyColor};

#[derive(Resource)]
pub struct Model {
    pub engine: Arc<PianoEngine>,
    pub key_colors: Vec<KeyColor>,
    pub reference_scl: Scl,
    pub keyboard: Keyboard,
    pub layout: KeyboardLayout,
    pub odd_limit: u16,
}

pub enum Event {
    Pressed(SourceId, Location, u8),
    Moved(SourceId, Location),
    Released(SourceId, u8),
}

#[derive(Copy, Clone, Debug, Eq, Hash, PartialEq)]
pub enum SourceId {
    Mouse,
    Touchpad(u64),
    Keyboard(i8, i8),
    Midi(PianoKey),
}

pub enum Location {
    Pitch(Pitch),
    Degree(i32),
}

impl Model {
    pub fn new(
        engine: Arc<PianoEngine>,
        key_colors: Vec<KeyColor>,
        keyboard: Keyboard,
        layout: KeyboardLayout,
        odd_limit: u16,
    ) -> Self {
        Self {
            engine,
            key_colors,
            reference_scl: Scl::builder().push_cents(100.0).build().unwrap(),
            keyboard,
            layout,
            odd_limit,
        }
    }
}

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
