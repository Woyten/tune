use bevy::color::palettes::css;
use bevy::prelude::*;
use bevy::window::WindowResolution;
use clap::Parser;
use tune::note::NoteLetter;
use tune::pitch::Pitch;
use tune::pitch::Pitched;
use tune::pitch::Ratio;
use tune::scala::KbmRoot;
use tune::scala::Scl;
use tune::tuning::Scale;

use crate::profile::ColorPalette;
use crate::toggle::Toggle;
use crate::tunable;
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
    pub resolution: WindowResolution,
}

#[derive(Debug)]
pub enum OnScreenKeyboards {
    None,
    Isomorphic,
    Linear,
    Reference,
    IsomorphicAndReference,
    LinearAndReference,
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
            OnScreenKeyboards::Linear,
            OnScreenKeyboards::Reference,
            OnScreenKeyboards::IsomorphicAndReference,
            OnScreenKeyboards::LinearAndReference,
        ];

        let tilts = vec![Tilt::None, Tilt::Automatic, Tilt::Lumatone];

        let inclinations = vec![Inclination::None, Inclination::Lumatone];

        let reference_tuning_layout = TuningLayout::new(
            Scl::builder().push_cents(100.0).build().unwrap(),
            KbmRoot::from(NoteLetter::D.in_octave(4)).to_kbm(),
            CustomKeyboardOptions::parse_from([""; 0]),
            &{
                ColorPalette {
                    natural_color: css::WHITE,
                    sharp_colors: vec![css::BLACK],
                    flat_colors: vec![css::BLACK],
                    enharmonic_colors: vec![css::BLACK],
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
            resolution: WindowResolution::default(),
        }
    }

    pub fn pitch_range(&self) -> Ratio {
        Ratio::between_pitches(self.viewport_left, self.viewport_right)
    }

    pub fn world_coord_of_pitch(&self, pitch: Pitch) -> f32 {
        (Ratio::between_pitches(self.viewport_left, pitch)
            .num_equal_steps_of_size(self.pitch_range())
            - 0.5) as f32
    }

    pub fn world_coords_of_tuning<'a>(
        &'a self,
        tuning: &'a impl Scale,
    ) -> impl Iterator<Item = (i32, f32)> + 'a {
        tunable::range(tuning, self.viewport_left, self.viewport_right).map(move |key_degree| {
            (
                key_degree,
                self.world_coord_of_pitch(tuning.sorted_pitch_of(key_degree)),
            )
        })
    }

    pub fn width(&self) -> f32 {
        self.resolution.width()
    }

    pub fn height(&self) -> f32 {
        self.resolution.height()
    }

    pub fn left(&self) -> f32 {
        -self.width() / 2.0
    }

    pub fn right(&self) -> f32 {
        self.width() / 2.0
    }

    pub fn bottom(&self) -> f32 {
        -self.height() / 2.0
    }

    pub fn top(&self) -> f32 {
        self.height() / 2.0
    }

    pub fn line_height(&self, num_lines: usize) -> f32 {
        const FONT_SIZE: f32 = 20.0;

        FONT_SIZE.min(self.height() / num_lines.max(1) as f32)
    }

    pub fn font_size(&self, num_lines: usize) -> FontSize {
        const LINE_SPACING: f32 = 1.2;

        FontSize::Px(self.line_height(num_lines) / LINE_SPACING)
    }
}
