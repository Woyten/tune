use bevy::prelude::*;
use tune::note::NoteLetter;
use tune::pitch::Pitch;
use tune::pitch::Pitched;
use tune::pitch::Ratio;
use tune::scala::Scl;

use crate::toggle::Toggle;
use crate::tuning_layout::Inclination;
use crate::tuning_layout::OnScreenKeyboards;
use crate::tuning_layout::Tilt;

#[derive(Resource)]
pub struct ViewSettings {
    pub on_screen_keyboard: Toggle<OnScreenKeyboards>,
    pub tilt: Toggle<Tilt>,
    pub inclination: Toggle<Inclination>,
    pub viewport_left: Pitch,
    pub viewport_right: Pitch,
    pub reference_scl: Scl,
    pub odd_limit: u16,
}

impl ViewSettings {
    pub fn new(odd_limit: u16) -> Self {
        let on_screen_keyboards = vec![
            OnScreenKeyboards::None,
            OnScreenKeyboards::Isomorphic,
            OnScreenKeyboards::Scale,
            OnScreenKeyboards::Reference,
            OnScreenKeyboards::IsomorphicAndReference,
            OnScreenKeyboards::ScaleAndReference,
        ];

        let tilts = vec![Tilt::None, Tilt::Automatic, Tilt::Lumatone];

        let inclinations = vec![Inclination::None, Inclination::Lumatone];

        Self {
            on_screen_keyboard: Toggle::with_initial_index(on_screen_keyboards, 1),
            tilt: Toggle::with_initial_index(tilts, 1),
            inclination: Toggle::with_initial_index(inclinations, 1),
            viewport_left: NoteLetter::Fsh.in_octave(2).pitch(),
            viewport_right: NoteLetter::Ash.in_octave(5).pitch(),
            reference_scl: Scl::builder().push_cents(100.0).build().unwrap(),
            odd_limit,
        }
    }

    pub fn pitch_range(&self) -> Ratio {
        Ratio::between_pitches(self.viewport_left, self.viewport_right)
    }

    pub fn hor_world_coord(&self, pitch: Pitch) -> f64 {
        Ratio::between_pitches(self.viewport_left, pitch)
            .num_equal_steps_of_size(self.pitch_range())
            - 0.5
    }
}
