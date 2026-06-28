use std::fmt;
use std::fmt::Display;
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
    pub layout: Toggle<Arc<VirtualKeyboard>>,
    pub schema: Toggle<Option<Arc<VirtualKeyboard>>>,
    pub compression: Toggle<Compression>,
}

pub struct VirtualKeyboard {
    pub name: String,
    pub mos: Mos,
    pub orig_mos: Mos,
    pub colors: Vec<Srgba>,
}

#[derive(Clone, Debug)]
pub enum Compression {
    Compressed,
    None,
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

        let mut layouts: Vec<_> = IsomorphicLayout::find_by_step_size(avg_step_size)
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

                Arc::new(VirtualKeyboard {
                    name: scale_name,
                    mos: mos.coprime(),
                    orig_mos: mos,
                    colors: generate_colors(&isomorphic_layout, palette),
                })
            })
            .collect();
        layouts.push({
            let mos = Mos::new(
                options.num_primary_steps,
                options.num_secondary_steps,
                options.primary_step,
                options.secondary_step,
            )
            .unwrap();

            Arc::new(VirtualKeyboard {
                name: options.layout_name,
                mos,
                orig_mos: mos,
                colors: options.colors.0,
            })
        });

        let mut schemas = vec![None];
        schemas.extend(layouts.iter().map(|layout| Some(layout.clone())));

        let compressions = vec![
            Compression::Compressed,
            Compression::None,
            Compression::Expanded,
        ];

        TuningLayout {
            scl,
            kbm,
            layout: layouts.into(),
            schema: schemas.into(),
            compression: Toggle::with_initial_index(compressions, 1),
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

    pub fn layout_step_sizes(&self) -> (i32, i32, i32) {
        let mos = &{
            let this = &self;
            this.layout.curr_option()
        }
        .mos;
        let primary_step = i32::from(mos.primary_step());
        let secondary_step = i32::from(mos.secondary_step());
        let secondary_step = match self.compression.curr_option() {
            Compression::None => secondary_step,
            Compression::Compressed => secondary_step + primary_step,
            Compression::Expanded => secondary_step - primary_step,
        };
        (primary_step, secondary_step, primary_step - secondary_step)
    }

    pub fn layout_step_counts(&self, tilt: &Tilt) -> (i32, i32) {
        match tilt {
            Tilt::Automatic => {
                let mos = {
                    let this = &self;
                    this.layout.curr_option()
                }
                .mos;
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

    pub fn fmt_layout(&self) -> impl Display {
        let (east, south_east, north_east) = self.layout_step_sizes();

        fmt::from_fn(move |f| {
            write!(
                f,
                "{} | east = {east}, south-east = {south_east}, north-east = {north_east}",
                self.layout.curr_option().name
            )
        })
    }

    pub fn get_key(&self, p: i16, s: i16) -> i32 {
        let p = match self.compression.curr_option() {
            Compression::None => p,
            Compression::Compressed => p + s,
            Compression::Expanded => p - s,
        };

        {
            let this = &self;
            this.layout.curr_option()
        }
        .mos
        .get_key(p, s)
    }

    fn schema_step_sizes(&self) -> (u16, u16, i32) {
        let mos = &self.curr_schema().orig_mos;
        (mos.primary_step(), mos.secondary_step(), mos.sharpness())
    }

    fn curr_schema(&self) -> &VirtualKeyboard {
        self.schema
            .curr_option()
            .as_deref()
            .unwrap_or(self.layout.curr_option())
    }

    pub fn colors(&self) -> &[Srgba] {
        &self.curr_schema().colors
    }

    pub fn fmt_schema(&self, replace_automatic: bool) -> impl Display {
        let (primary, secondary, sharpness) = self.schema_step_sizes();

        let schema_name = match self.schema.curr_index() == 0 && !replace_automatic {
            true => "Automatic",
            false => &self.curr_schema().name,
        };

        fmt::from_fn(move |f| {
            write!(
                f,
                "{} | primary = {primary}, secondary = {secondary}, sharpness = {sharpness}",
                schema_name
            )
        })
    }
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

#[derive(Clone, Copy, Debug)]
pub enum OnScreenKeyboards {
    None,
    Isomorphic,
    Scale,
    Reference,
    IsomorphicAndReference,
    ScaleAndReference,
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
