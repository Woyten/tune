use std::fmt;
use std::fmt::Write;

use bevy::prelude::*;

use crate::app::state::BackendState;
use crate::app::state::ViewState;
use crate::app::state::view::OnScreenKeyboards;
use crate::control::LiveParameter;
use crate::piano::PianoEngine;
use crate::piano::PianoEngineState;
use crate::toggle::Direction;

pub fn build_menu() -> Menu {
    Menu::new()
        .add_setting(
            "Tuning",
            |ctx| {
                write!(
                    ctx.output,
                    "{} - {}",
                    ctx.engine_state.scale_index + 1,
                    ctx.engine_state.curr_tuning_layout.scl.description()
                )
            },
            |ctx, direction| {
                ctx.engine.switch_tuning(direction);
            },
        )
        .add_setting(
            "Tuning Mode",
            |ctx| write!(ctx.output, "{:?}", ctx.engine_state.tuning_mode),
            |ctx, direction| {
                ctx.engine.switch_tuning_mode(direction);
            },
        )
        .add_spacer()
        .add_setting(
            "Output Target",
            |ctx| write!(ctx.output, "{}", ctx.backend_state.backend),
            |ctx, direction| {
                ctx.engine.switch_backend(direction);
            },
        )
        .add_setting(
            "Bank",
            |ctx| match &ctx.backend_state.bank {
                Some(bank) => write!(ctx.output, "{bank}"),
                None => write!(ctx.output, "-"),
            },
            |ctx, direction| {
                ctx.engine.switch_bank(direction);
            },
        )
        .add_setting(
            "Program",
            |ctx| match &ctx.backend_state.program {
                Some(program) => write!(ctx.output, "{program}"),
                None => write!(ctx.output, "-"),
            },
            |ctx, direction| {
                ctx.engine.switch_program(direction);
            },
        )
        .add_setting(
            "Envelope",
            |ctx| match &ctx.backend_state.envelope {
                Some(envelope) => write!(ctx.output, "{envelope}"),
                None => write!(ctx.output, "-"),
            },
            |ctx, direction| {
                ctx.engine.switch_envelope_type(direction);
            },
        )
        .add_setting(
            "Legato",
            |ctx| {
                if ctx.engine_state.storage.is_active(LiveParameter::Legato) {
                    write!(
                        ctx.output,
                        "ON (cc {})",
                        ctx.engine_state
                            .mapper
                            .get_ccn(LiveParameter::Legato)
                            .unwrap()
                    )
                } else {
                    write!(ctx.output, "OFF")
                }
            },
            |ctx, direction| {
                let value = match direction {
                    Direction::Forward => 1.0,
                    Direction::Backward => 0.0,
                };
                ctx.engine.set_parameter(LiveParameter::Legato, value);
            },
        )
        .add_spacer()
        .add_setting(
            "On-Screen Kbd",
            |ctx| match ctx.view_state.on_screen_keyboard.curr_option() {
                OnScreenKeyboards::IsomorphicAndReference => {
                    write!(ctx.output, "Isomorphic + Reference")
                }
                OnScreenKeyboards::ScaleAndReference => write!(ctx.output, "Scale + Reference"),
                other => write!(ctx.output, "{:?}", other),
            },
            |ctx, direction| {
                ctx.view_state.on_screen_keyboard.switch(direction);
            },
        )
        .add_setting(
            "Layout",
            |ctx| {
                write!(
                    ctx.output,
                    "{}",
                    ctx.engine_state.curr_tuning_layout.fmt_layout()
                )
            },
            |ctx, direction| {
                ctx.engine.switch_layout(direction);
            },
        )
        .add_setting(
            "Schema",
            |ctx| {
                write!(
                    ctx.output,
                    "{}",
                    ctx.engine_state.curr_tuning_layout.fmt_schema(false)
                )
            },
            |ctx, direction| {
                ctx.engine.switch_scale(direction);
            },
        )
        .add_setting(
            "Compression",
            |ctx| {
                write!(
                    ctx.output,
                    "{:?}",
                    ctx.engine_state
                        .curr_tuning_layout
                        .compression
                        .curr_option()
                )
            },
            |ctx, direction| {
                ctx.engine.switch_compression(direction);
            },
        )
        .add_setting(
            "Tilt",
            |ctx| write!(ctx.output, "{:?}", ctx.view_state.tilt.curr_option()),
            |ctx, direction| {
                ctx.view_state.tilt.switch(direction);
            },
        )
        .add_setting(
            "Inclination",
            |ctx| write!(ctx.output, "{:?}", ctx.view_state.inclination.curr_option()),
            |ctx, direction| {
                ctx.view_state.inclination.switch(direction);
            },
        )
        .add_spacer()
        .add_setting(
            "Root Note",
            |ctx| {
                write!(
                    ctx.output,
                    "{}",
                    ctx.engine_state
                        .curr_tuning_layout
                        .kbm
                        .root
                        .ref_key
                        .midi_number()
                )
            },
            |ctx, direction| {
                ctx.engine.switch_ref_note(direction);
            },
        )
        .add_setting(
            "Scale Offset",
            |ctx| {
                write!(
                    ctx.output,
                    "{:+}",
                    ctx.engine_state.curr_tuning_layout.kbm.root.root_offset
                )
            },
            |ctx, direction| {
                ctx.engine.switch_root_offset(i32::from(direction.delta()));
            },
        )
        .add_setting(
            "Iso Offset",
            |ctx| {
                write!(
                    ctx.output,
                    "{:+}",
                    ctx.engine_state.curr_tuning_layout.isomorphic_offset
                )
            },
            |ctx, direction| {
                ctx.engine
                    .switch_isomorphic_offset(i32::from(direction.delta()));
            },
        )
        .add_setting(
            "  Inc/Dec by →",
            |_| Ok(()),
            |ctx, direction| {
                let (step, _, _) = ctx.engine_state.curr_tuning_layout.layout_step_sizes();
                ctx.engine
                    .switch_isomorphic_offset(i32::from(direction.delta()) * step);
            },
        )
        .add_setting(
            "  Inc/Dec by ↘",
            |_| Ok(()),
            |ctx, direction| {
                let (_, step, _) = ctx.engine_state.curr_tuning_layout.layout_step_sizes();
                ctx.engine
                    .switch_isomorphic_offset(i32::from(direction.delta()) * step);
            },
        )
        .add_spacer()
        .add_info(|ctx| {
            write!(
                ctx.output,
                "Range [Scroll/Alt+Scroll]: {:.0}..{:.0} Hz",
                ctx.view_state.viewport_left.as_hz(),
                ctx.view_state.viewport_right.as_hz()
            )
        })
}

#[derive(Resource)]
pub struct Menu {
    entries: Vec<MenuEntry>,
    selected_entry: usize,
    max_setting_width: usize,
}

enum MenuEntry {
    Setting {
        name: &'static str,
        render: RenderFn,
        action: ActionFn,
    },
    Info {
        render: RenderFn,
    },
}

type RenderFn = Box<dyn Fn(RenderContext<'_>) -> fmt::Result + Send + Sync>;

type ActionFn = Box<dyn Fn(ActionContext<'_, '_>, Direction) + Send + Sync>;

pub struct RenderContext<'a> {
    pub output: &'a mut String,
    pub engine_state: &'a PianoEngineState,
    pub backend_state: &'a BackendState,
    pub view_state: &'a ViewState,
}

pub struct ActionContext<'a, 'b> {
    pub engine: &'a PianoEngine,
    pub engine_state: &'a PianoEngineState,
    pub view_state: &'a mut ResMut<'b, ViewState>,
}

impl Menu {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
            selected_entry: 0,
            max_setting_width: 0,
        }
    }

    fn add_setting(
        mut self,
        name: &'static str,
        render: impl Fn(RenderContext<'_>) -> fmt::Result + Send + Sync + 'static,
        action: impl Fn(ActionContext<'_, '_>, Direction) + Send + Sync + 'static,
    ) -> Self {
        self.max_setting_width = self.max_setting_width.max(name.len());
        self.entries.push(MenuEntry::Setting {
            name,
            render: Box::new(render),
            action: Box::new(action),
        });
        self
    }

    fn add_spacer(self) -> Self {
        self.add_info(|_| Ok(()))
    }

    fn add_info(
        mut self,
        render: impl Fn(RenderContext<'_>) -> fmt::Result + Send + Sync + 'static,
    ) -> Self {
        self.entries.push(MenuEntry::Info {
            render: Box::new(render),
        });
        self
    }

    pub fn select_next(&mut self) {
        if let Some(next) =
            (self.selected_entry + 1..self.entries.len()).find(|&i| self.setting_name(i).is_some())
        {
            self.selected_entry = next;
        }
    }

    pub fn select_prev(&mut self) {
        if let Some(prev) = (0..self.selected_entry)
            .rev()
            .find(|&i| self.setting_name(i).is_some())
        {
            self.selected_entry = prev;
        }
    }

    pub fn switch(&self, ctx: ActionContext<'_, '_>, direction: Direction) {
        if let MenuEntry::Setting { action, .. } = &self.entries[self.selected_entry] {
            action(ctx, direction);
        }
    }

    pub fn select_by_initial(&mut self, c: char) {
        let c = c.to_ascii_lowercase();
        let matching: Vec<usize> = self
            .entries
            .iter()
            .enumerate()
            .filter(|&(i, _)| {
                self.setting_name(i).is_some_and(|name| {
                    name.chars()
                        .next()
                        .is_some_and(|first| first.to_ascii_lowercase() == c)
                })
            })
            .map(|(i, _)| i)
            .collect();
        if matching.is_empty() {
            return;
        }
        if let Some(pos) = matching.iter().position(|&i| i == self.selected_entry) {
            self.selected_entry = matching[(pos + 1) % matching.len()];
        } else {
            self.selected_entry = matching[0];
        }
    }

    fn setting_name(&self, index: usize) -> Option<&str> {
        match &self.entries[index] {
            MenuEntry::Setting { name, .. } => Some(*name),
            MenuEntry::Info { .. } => None,
        }
    }

    pub fn render_full(&self, ctx: RenderContext<'_>) {
        for (i, entry) in self.entries.iter().enumerate() {
            let render = match entry {
                MenuEntry::Setting { name, render, .. } => {
                    let selector = if i == self.selected_entry { "> " } else { "  " };
                    write!(
                        ctx.output,
                        "{}{:width$}",
                        selector,
                        name,
                        width = self.max_setting_width + 4
                    )
                    .unwrap();
                    render
                }
                MenuEntry::Info { render } => render,
            };

            render(RenderContext {
                output: ctx.output,
                engine_state: ctx.engine_state,
                backend_state: ctx.backend_state,
                view_state: ctx.view_state,
            })
            .unwrap();
            writeln!(ctx.output).unwrap();
        }
        writeln!(ctx.output, "[Alt]+Letter = Quick Select").unwrap();
    }

    pub fn render_light(&self, ctx: RenderContext<'_>) {
        let layout = &ctx.engine_state.curr_tuning_layout;

        writeln!(ctx.output, "Tuning: {}", layout.scl.description()).unwrap();
        writeln!(ctx.output, "Layout: {}", layout.fmt_layout()).unwrap();
        writeln!(ctx.output, "Schema: {}", layout.fmt_schema(true)).unwrap();

        let effects = [
            LiveParameter::Sound1,
            LiveParameter::Sound2,
            LiveParameter::Sound3,
            LiveParameter::Sound4,
            LiveParameter::Sound5,
            LiveParameter::Sound6,
            LiveParameter::Sound7,
            LiveParameter::Sound8,
            LiveParameter::Sound9,
            LiveParameter::Sound10,
        ]
        .into_iter()
        .enumerate()
        .filter(|&(_, p)| ctx.engine_state.storage.is_active(p))
        .map(|(i, p)| {
            format!(
                "{} (cc {})",
                i + 1,
                ctx.engine_state.mapper.get_ccn(p).unwrap()
            )
        })
        .collect::<Vec<_>>()
        .join(", ");

        if !effects.is_empty() {
            writeln!(ctx.output, "Effects: {}", effects).unwrap();
        }

        for recorder_detail in ctx.backend_state.recorder_details.values() {
            writeln!(ctx.output, "{}", recorder_detail).unwrap();
        }

        writeln!(ctx.output, "Press [F1-F10] for effects").unwrap();
        writeln!(ctx.output, "Press [Alt] for options").unwrap();
    }
}
