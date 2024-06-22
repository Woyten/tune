use std::{
    fmt::{self, Display},
    sync::Arc,
};

use bevy::prelude::*;
use tune::{
    layout::{IsomorphicLayout, Layer},
    pergen::Mos,
    pitch::Ratio,
    scala::Scl,
};

use crate::{app::Toggle, profile::ColorPalette, CustomKeyboardOptions};

#[derive(Resource)]
pub struct VirtualKeyboardResource {
    pub on_screen_keyboard: Toggle<OnScreenKeyboards>,
    pub scale: Toggle<VirtualKeyboardScale>,
    pub layout: Toggle<Option<Arc<VirtualKeyboardLayout>>>,
    pub compression: Toggle<Compression>,
    pub tilt: Toggle<Tilt>,
    pub inclination: Toggle<Inclination>,
    pub avg_step_size: Ratio,
}

#[derive(Clone, Copy)]
pub enum OnScreenKeyboards {
    Isomorphic,
    Scale,
    Reference,
    IsomorphicAndReference,
    ScaleAndReference,
    None,
}

impl Display for OnScreenKeyboards {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Isomorphic => write!(f, "Isomorphic"),
            Self::Scale => write!(f, "Scale"),
            Self::Reference => write!(f, "Reference"),
            Self::IsomorphicAndReference => write!(f, "Isomorphic + Reference"),
            Self::ScaleAndReference => write!(f, "Scale + Reference"),
            Self::None => write!(f, "OFF"),
        }
    }
}

pub struct VirtualKeyboardScale {
    layout: Arc<VirtualKeyboardLayout>,
    colors: Vec<Color>,
}

pub struct VirtualKeyboardLayout {
    scale_name: String,
    mos: Mos,
    orig_mos: Mos,
}

#[derive(Debug)]
pub enum Compression {
    None,
    Compressed,
    Expanded,
}

#[derive(Debug)]
pub enum Tilt {
    Automatic,
    Lumatone,
    None,
}

#[derive(Debug)]
pub enum Inclination {
    Lumatone,
    None,
}

impl VirtualKeyboardResource {
    pub fn new(
        scl: &Scl,
        options: CustomKeyboardOptions,
        palette: &ColorPalette,
    ) -> VirtualKeyboardResource {
        let on_screen_keyboards = vec![
            OnScreenKeyboards::Isomorphic,
            OnScreenKeyboards::Scale,
            OnScreenKeyboards::Reference,
            OnScreenKeyboards::IsomorphicAndReference,
            OnScreenKeyboards::ScaleAndReference,
            OnScreenKeyboards::None,
        ];

        let avg_step_size = if scl.period().is_negligible() {
            Ratio::from_octaves(1)
        } else {
            scl.period()
        }
        .divided_into_equal_steps(scl.num_items());

        let mut scales = Vec::new();
        let mut layouts = vec![None];

        IsomorphicLayout::find_by_step_size(avg_step_size)
            .into_iter()
            .map(|isomorphic_layout| {
                let scale_name = format!(
                    "{} | {}{}",
                    isomorphic_layout.notation(),
                    isomorphic_layout.get_scale_name(),
                    isomorphic_layout
                        .alt_tritave()
                        .then_some(" | b-val")
                        .unwrap_or_default(),
                );

                let mos = isomorphic_layout.mos();

                (
                    VirtualKeyboardLayout {
                        scale_name,
                        mos: mos.coprime(),
                        orig_mos: mos,
                    },
                    generate_colors(&isomorphic_layout, palette),
                )
            })
            .chain({
                let mos = Mos::new(
                    options.num_primary_steps,
                    options.num_secondary_steps,
                    options.primary_step,
                    options.secondary_step,
                )
                .unwrap();

                [(
                    VirtualKeyboardLayout {
                        scale_name: options.layout_name,
                        mos,
                        orig_mos: mos,
                    },
                    options.colors.0,
                )]
            })
            .for_each(|(layout, colors)| {
                let layout = Arc::new(layout);

                scales.push(VirtualKeyboardScale {
                    layout: layout.clone(),
                    colors,
                });
                layouts.push(Some(layout));
            });

        let compressions = vec![
            Compression::None,
            Compression::Compressed,
            Compression::Expanded,
        ];

        let tilts = vec![Tilt::Automatic, Tilt::Lumatone, Tilt::None];

        let inclinations = vec![Inclination::Lumatone, Inclination::None];

        VirtualKeyboardResource {
            on_screen_keyboard: on_screen_keyboards.into(),
            scale: scales.into(),
            layout: layouts.into(),
            compression: compressions.into(),
            tilt: tilts.into(),
            inclination: inclinations.into(),
            avg_step_size,
        }
    }

    pub fn scale_name(&self) -> &str {
        &self.scale.curr_option().layout.scale_name
    }

    pub fn colors(&self) -> &[Color] {
        &self.scale.curr_option().colors
    }

    pub fn scale_step_sizes(&self) -> (u16, u16, i32) {
        let mos = &self.scale.curr_option().layout.orig_mos;
        (mos.primary_step(), mos.secondary_step(), mos.sharpness())
    }

    pub fn layout_name(&self) -> &str {
        self.layout
            .curr_option()
            .as_ref()
            .map(|layout| &*layout.scale_name)
            .unwrap_or("Automatic")
    }

    pub fn layout_step_counts(&self) -> (i32, i32) {
        match self.tilt.curr_option() {
            Tilt::Automatic => {
                let mos = self.curr_layout().mos;
                let num_primary_steps = i32::from(mos.num_primary_steps());
                let num_secondary_steps = i32::from(mos.num_secondary_steps());

                let num_primary_steps = match self.compression.curr_option() {
                    Compression::None => num_primary_steps,
                    Compression::Compressed => num_primary_steps - num_secondary_steps,
                    Compression::Expanded => num_primary_steps + num_secondary_steps,
                };
                (num_primary_steps, num_secondary_steps)
            }
            Tilt::Lumatone => (5, 2),
            Tilt::None => (1, 0),
        }
    }

    pub fn layout_step_sizes(&self) -> (i32, i32, i32) {
        let mos = &self.curr_layout().mos;
        let primary_step = i32::from(mos.primary_step());
        let secondary_step = i32::from(mos.secondary_step());
        let secondary_step = match self.compression.curr_option() {
            Compression::None => secondary_step,
            Compression::Compressed => secondary_step + primary_step,
            Compression::Expanded => secondary_step - primary_step,
        };
        (primary_step, secondary_step, primary_step - secondary_step)
    }

    fn curr_layout(&self) -> &VirtualKeyboardLayout {
        self.layout
            .curr_option()
            .as_ref()
            .unwrap_or_else(|| &self.scale.curr_option().layout)
    }

    pub fn get_key(&self, num_primary_steps: i16, num_secondary_steps: i16) -> i32 {
        let num_primary_steps = match self.compression.curr_option() {
            Compression::None => num_primary_steps,
            Compression::Compressed => num_primary_steps + num_secondary_steps,
            Compression::Expanded => num_primary_steps - num_secondary_steps,
        };

        self.curr_layout()
            .mos
            .get_key(num_primary_steps, num_secondary_steps)
    }

    pub fn inclination(&self) -> f32 {
        match self.inclination.curr_option() {
            Inclination::Lumatone => 15.0,
            Inclination::None => 0.0,
        }
    }
}

fn generate_colors(layout: &IsomorphicLayout, palette: &ColorPalette) -> Vec<Color> {
    let mut colors: Vec<_> = layout
        .get_layers()
        .into_iter()
        .map(|layer| {
            let get_color = |colors: &[Color], index| colors[usize::from(index) % colors.len()];

            match layer {
                Layer::Natural => palette.natural_color,
                Layer::Sharp(index) => get_color(&palette.sharp_colors, index),
                Layer::Flat(index) => get_color(&palette.flat_colors, index),
                Layer::Enharmonic(index) => get_color(&palette.enharmonic_colors, index),
            }
        })
        .collect();

    if layout.mos().sharpness() == 0 {
        for i in 0..layout.mos().num_cycles() {
            colors[usize::from(i)] = palette.root_color
        }
    }

    colors
}
