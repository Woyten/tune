use bevy::prelude::Resource;

use crate::toggle::Toggle;
use crate::tuning_layout::Inclination;
use crate::tuning_layout::OnScreenKeyboards;
use crate::tuning_layout::Tilt;

#[derive(Resource)]
pub struct KeyboardViewSettings {
    pub on_screen_keyboard: Toggle<OnScreenKeyboards>,
    pub tilt: Toggle<Tilt>,
    pub inclination: Toggle<Inclination>,
}

impl KeyboardViewSettings {
    pub fn new() -> Self {
        let on_screen_keyboards = vec![
            OnScreenKeyboards::Isomorphic,
            OnScreenKeyboards::Scale,
            OnScreenKeyboards::Reference,
            OnScreenKeyboards::IsomorphicAndReference,
            OnScreenKeyboards::ScaleAndReference,
            OnScreenKeyboards::None,
        ];

        let tilts = vec![Tilt::Automatic, Tilt::Lumatone, Tilt::None];

        let inclinations = vec![Inclination::Lumatone, Inclination::None];

        Self {
            on_screen_keyboard: on_screen_keyboards.into(),
            tilt: tilts.into(),
            inclination: inclinations.into(),
        }
    }
}
