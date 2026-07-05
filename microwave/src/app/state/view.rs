use bevy::color::palettes::css;
use bevy::prelude::*;
use clap::Parser;
use tune::note::NoteLetter;
use tune::pitch::Pitch;
use tune::pitch::Pitched;
use tune::pitch::Ratio;
use tune::scala::KbmRoot;
use tune::scala::Scl;

use crate::profile::ColorPalette;
use crate::toggle::Toggle;
use crate::tuning_layout::CustomKeyboardOptions;
use crate::tuning_layout::TuningLayout;

#[derive(Resource)]
pub struct ViewState {
    pub on_screen_keyboard: Toggle<OnScreenKeyboards>,
    pub tilt: Toggle<Tilt>,
    pub inclination: Toggle<Inclination>,
    pub viewport_left: Pitch,
    pub viewport_right: Pitch,
    pub reference_tuning_layout: TuningLayout,
    pub odd_limit: u16,
}

#[derive(Debug)]
pub enum OnScreenKeyboards {
    None,
    Isomorphic,
    Scale,
    Reference,
    IsomorphicAndReference,
    ScaleAndReference,
}

#[derive(Debug)]
pub enum Tilt {
    None,
    Automatic,
    Lumatone,
}

#[derive(Debug)]
pub enum Inclination {
    Lumatone,
    None,
}

impl Inclination {
    pub fn degrees(&self) -> f32 {
        match self {
            Inclination::Lumatone => 15.0,
            Inclination::None => 0.0,
        }
    }
}

impl ViewState {
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

        let reference_tuning_layout = TuningLayout::new(
            Scl::builder().push_cents(100.0).build().unwrap(),
            KbmRoot::from(NoteLetter::D.in_octave(4)).to_kbm(),
            CustomKeyboardOptions::parse_from([""; 0]),
            &{
                let black = css::WHITE * 0.2;
                let white = css::WHITE;

                ColorPalette {
                    natural_color: white,
                    sharp_colors: vec![black],
                    flat_colors: vec![black],
                    enharmonic_colors: vec![black],
                }
            },
        );

        Self {
            on_screen_keyboard: Toggle::with_initial_index(on_screen_keyboards, 1),
            tilt: Toggle::with_initial_index(tilts, 1),
            inclination: Toggle::with_initial_index(inclinations, 1),
            viewport_left: NoteLetter::Fsh.in_octave(2).pitch(),
            viewport_right: NoteLetter::Ash.in_octave(5).pitch(),
            reference_tuning_layout,
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
