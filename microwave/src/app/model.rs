use std::sync::Arc;

use bevy::prelude::Resource;
use tune::{pitch::Pitch, scala::Scl};

use crate::{
    piano::{PianoEngine, PianoEngineState},
    KeyColor,
};

#[derive(Resource)]
pub struct PianoEngineResource(pub Arc<PianoEngine>);

#[derive(Resource)]
pub struct PianoEngineStateResource(pub PianoEngineState);

#[derive(Resource)]
pub struct ViewModel {
    pub viewport_left: Pitch,
    pub viewport_right: Pitch,
    pub on_screen_keyboards: OnScreenKeyboards,
    pub scale_keyboard_colors: Vec<KeyColor>,
    pub reference_scl: Scl,
    pub odd_limit: u16,
}

#[derive(Clone, Copy)]
pub enum OnScreenKeyboards {
    Scale,
    Reference,
    ScaleAndReference,
    None,
}

impl ViewModel {
    pub fn toggle_on_screen_keyboards(&mut self) {
        self.on_screen_keyboards = match self.on_screen_keyboards {
            OnScreenKeyboards::Scale => OnScreenKeyboards::Reference,
            OnScreenKeyboards::Reference => OnScreenKeyboards::ScaleAndReference,
            OnScreenKeyboards::ScaleAndReference => OnScreenKeyboards::None,
            OnScreenKeyboards::None => OnScreenKeyboards::Scale,
        };
    }
}
