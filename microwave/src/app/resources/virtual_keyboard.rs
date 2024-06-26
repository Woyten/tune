use std::{
    cmp::Ordering,
    fmt::{self, Display},
    sync::Arc,
};

use bevy::prelude::*;
use tune::{layout::IsomorphicLayout, pergen::Mos, pitch::Ratio, scala::Scl};

use crate::{app::Toggle, CustomKeyboardOptions};

#[derive(Resource)]
pub struct VirtualKeyboardResource {
    pub on_screen_keyboard: Toggle<OnScreenKeyboards>,
    pub scale: Toggle<VirtualKeyboardScale>,
    pub layout: Toggle<Option<Arc<VirtualKeyboardLayout>>>,
    pub compression: Toggle<Compression>,
    pub tilt: Toggle<Tilt>,
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

impl VirtualKeyboardResource {
    pub fn new(scl: &Scl, options: CustomKeyboardOptions) -> VirtualKeyboardResource {
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
                    generate_colors(&isomorphic_layout),
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

        VirtualKeyboardResource {
            on_screen_keyboard: on_screen_keyboards.into(),
            scale: scales.into(),
            layout: layouts.into(),
            compression: compressions.into(),
            tilt: tilts.into(),
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
}

fn generate_colors(layout: &IsomorphicLayout) -> Vec<Color> {
    let color_indexes = layout.get_colors();

    let colors = [
        SHARP_COLOR,
        FLAT_COLOR,
        DOUBLE_SHARP_COLOR,
        DOUBLE_FLAT_COLOR,
        TRIPLE_SHARP_COLOR,
        TRIPLE_FLAT_COLOR,
    ];

    (0..layout.pergen().period())
        .map(|index| {
            const CYCLE_DARKNESS_FACTOR: f32 = 0.5;

            let generation = layout.pergen().get_generation(index);
            let degree = generation.degree;
            let color_index = color_indexes[usize::from(degree)];

            // The shade logic combines two requirements:
            // - High contrast in the sharp (north-east) direction => Alternation
            // - High contrast in the secondary (south-east) direction => Exception to the alternation rule for the middle cycle
            let cycle_darkness = match (generation.cycle.unwrap_or_default() * 2 + 1)
                .cmp(&layout.pergen().num_cycles())
            {
                Ordering::Less => {
                    CYCLE_DARKNESS_FACTOR * f32::from(generation.cycle.unwrap_or_default() % 2 != 0)
                }
                Ordering::Equal => CYCLE_DARKNESS_FACTOR / 2.0,
                Ordering::Greater => {
                    CYCLE_DARKNESS_FACTOR
                        * f32::from(
                            (layout.pergen().num_cycles() - generation.cycle.unwrap_or_default())
                                % 2
                                != 0,
                        )
                }
            };

            (match color_index {
                0 => NATURAL_COLOR,
                x => colors[(x - 1) % colors.len()],
            }) * (1.0 - cycle_darkness)
        })
        .collect()
}

const NATURAL_COLOR: Color = Color::WHITE;
const SHARP_COLOR: Color = Color::rgb(0.5, 0.0, 1.0);
const FLAT_COLOR: Color = Color::rgb(0.5, 1.0, 0.5);
const DOUBLE_SHARP_COLOR: Color = Color::rgb(0.5, 0.5, 1.0);
const DOUBLE_FLAT_COLOR: Color = Color::rgb(0.0, 0.5, 0.5);
const TRIPLE_SHARP_COLOR: Color = Color::rgb(0.5, 0.0, 0.5);
const TRIPLE_FLAT_COLOR: Color = Color::rgb(1.0, 0.0, 0.5);
