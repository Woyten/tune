use std::fmt;
use std::fmt::Display;
use std::fmt::Formatter;
use std::sync::Arc;

use bevy::prelude::*;
use tune::layout::IsomorphicLayout;
use tune::layout::Layer;
use tune::pergen::Mos;
use tune::pitch::Ratio;
use tune::scala::Kbm;
use tune::scala::Scl;

use crate::CustomKeyboardOptions;
use crate::profile::ColorPalette;
use crate::toggle::Toggle;

#[derive(Clone)]
pub struct TuningLayout {
    pub scl: Scl,
    pub kbm: Kbm,
    pub scale: Toggle<VirtualKeyboardScale>,
    pub layout: Toggle<Option<Arc<VirtualKeyboardLayout>>>,
    pub compression: Toggle<Compression>,
}

#[derive(Clone)]
pub struct VirtualKeyboardScale {
    pub layout: Arc<VirtualKeyboardLayout>,
    pub colors: Vec<Srgba>,
}

pub struct VirtualKeyboardLayout {
    pub scale_name: String,
    pub mos: Mos,
    pub orig_mos: Mos,
}

#[derive(Clone, Debug)]
pub enum Compression {
    None,
    Compressed,
    Expanded,
}

impl TuningLayout {
    pub fn new(
        scl: Scl,
        kbm: Kbm,
        options: CustomKeyboardOptions,
        palette: &ColorPalette,
    ) -> TuningLayout {
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
                    isomorphic_layout.genchain(),
                    isomorphic_layout.get_scale_name(),
                    if isomorphic_layout.b_val() {
                        " | b-val"
                    } else {
                        Default::default()
                    },
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

        TuningLayout {
            scl,
            kbm,
            scale: scales.into(),
            layout: layouts.into(),
            compression: compressions.into(),
        }
    }

    pub fn avg_step_size(&self) -> Ratio {
        if self.scl.period().is_negligible() {
            Ratio::from_octaves(1)
        } else {
            self.scl.period()
        }
        .divided_into_equal_steps(self.scl.num_items())
    }

    pub fn scale_name(&self) -> &str {
        &self.scale.curr_option().layout.scale_name
    }

    pub fn colors(&self) -> &[Srgba] {
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

    pub fn layout_step_counts(&self, tilt: &Tilt) -> (i32, i32) {
        match tilt {
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

    pub fn layout_step_sizes(&self) -> (i32, i32) {
        let mos = &self.curr_layout().mos;
        let primary_step = i32::from(mos.primary_step());
        let secondary_step = i32::from(mos.secondary_step());
        let secondary_step = match self.compression.curr_option() {
            Compression::None => secondary_step,
            Compression::Compressed => secondary_step + primary_step,
            Compression::Expanded => secondary_step - primary_step,
        };
        (primary_step, secondary_step)
    }

    pub fn curr_layout(&self) -> &VirtualKeyboardLayout {
        self.layout
            .curr_option()
            .as_ref()
            .unwrap_or_else(|| &self.scale.curr_option().layout)
    }

    pub fn get_key(&self, p: i16, s: i16) -> i32 {
        let p = match self.compression.curr_option() {
            Compression::None => p,
            Compression::Compressed => p + s,
            Compression::Expanded => p - s,
        };

        self.curr_layout().mos.get_key(p, s)
    }
}

#[derive(Debug)]
pub enum Tilt {
    Automatic,
    Lumatone,
    None,
}

impl Display for Tilt {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Tilt::Automatic => write!(f, "Automatic"),
            Tilt::Lumatone => write!(f, "Lumatone"),
            Tilt::None => write!(f, "None"),
        }
    }
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

impl Display for Inclination {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Inclination::Lumatone => write!(f, "Lumatone"),
            Inclination::None => write!(f, "None"),
        }
    }
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
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
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

fn generate_colors(layout: &IsomorphicLayout, palette: &ColorPalette) -> Vec<Srgba> {
    layout
        .get_layers()
        .into_iter()
        .map(|layer| {
            let get_color = |colors: &[Srgba], index| colors[usize::from(index) % colors.len()];

            match layer {
                Layer::Natural => palette.natural_color,
                Layer::Sharp(index) => get_color(&palette.sharp_colors, index),
                Layer::Flat(index) => get_color(&palette.flat_colors, index),
                Layer::Enharmonic(index) => get_color(&palette.enharmonic_colors, index),
            }
        })
        .collect()
}
