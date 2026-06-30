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
            |out, engine_state, _backend_state, _view_state| {
                write!(
                    out,
                    "{} - {}",
                    engine_state.scale_index + 1,
                    engine_state.curr_tuning_layout.scl.description()
                )
            },
            |engine, _view_state, direction| {
                engine.switch_tuning(direction);
            },
        )
        .add_setting(
            "Tuning Mode",
            |out, engine_state, _backend_state, _view_state| {
                write!(out, "{:?}", engine_state.tuning_mode)
            },
            |engine, _view_state, direction| {
                engine.switch_tuning_mode(direction);
            },
        )
        .add_spacer()
        .add_setting(
            "Output Target",
            |out, _engine_state, backend_state, _view_state| {
                out.push_str(&backend_state.backend);
                Ok(())
            },
            |engine, _view_state, direction| {
                engine.switch_backend(direction);
            },
        )
        .add_setting(
            "Bank",
            |out, _engine_state, backend_state, _view_state| match &backend_state.bank {
                Some(bank) => {
                    out.push_str(bank);
                    Ok(())
                }
                None => write!(out, "-"),
            },
            |engine, _view_state, direction| {
                engine.switch_bank(direction);
            },
        )
        .add_setting(
            "Program",
            |out, _engine_state, backend_state, _view_state| match &backend_state.program {
                Some(program) => {
                    out.push_str(program);
                    Ok(())
                }
                None => write!(out, "-"),
            },
            |engine, _view_state, direction| {
                engine.switch_program(direction);
            },
        )
        .add_setting(
            "Envelope",
            |out, _engine_state, backend_state, _view_state| match &backend_state.envelope {
                Some(envelope) => {
                    out.push_str(envelope);
                    Ok(())
                }
                None => write!(out, "-"),
            },
            |engine, _view_state, direction| {
                engine.switch_envelope_type(direction);
            },
        )
        .add_setting(
            "Legato",
            |out, engine_state, _backend_state, _view_state| {
                if engine_state.storage.is_active(LiveParameter::Legato) {
                    write!(
                        out,
                        "ON (cc {})",
                        engine_state.mapper.get_ccn(LiveParameter::Legato).unwrap()
                    )
                } else {
                    write!(out, "OFF")
                }
            },
            |engine, _view_state, direction| {
                let value = match direction {
                    Direction::Forward => 1.0,
                    Direction::Backward => 0.0,
                };
                engine.set_parameter(LiveParameter::Legato, value);
            },
        )
        .add_spacer()
        .add_setting(
            "On-Screen Kbd",
            |out, _engine_state, _backend_state, view_state| match view_state
                .on_screen_keyboard
                .curr_option()
            {
                OnScreenKeyboards::IsomorphicAndReference => write!(out, "Isomorphic + Reference"),
                OnScreenKeyboards::ScaleAndReference => write!(out, "Scale + Reference"),
                other => write!(out, "{:?}", other),
            },
            |_engine, view_state, direction| {
                view_state.on_screen_keyboard.switch(direction);
            },
        )
        .add_setting(
            "Layout",
            |out, engine_state, _backend_state, _view_state| {
                write!(out, "{}", engine_state.curr_tuning_layout.fmt_layout())
            },
            |engine, _view_state, direction| {
                engine.switch_layout(direction);
            },
        )
        .add_setting(
            "Schema",
            |out, engine_state, _backend_state, _view_state| {
                write!(out, "{}", engine_state.curr_tuning_layout.fmt_schema(false))
            },
            |engine, _view_state, direction| {
                engine.switch_scale(direction);
            },
        )
        .add_setting(
            "Compression",
            |out, engine_state, _backend_state, _view_state| {
                write!(
                    out,
                    "{:?}",
                    engine_state.curr_tuning_layout.compression.curr_option()
                )
            },
            |engine, _view_state, direction| {
                engine.switch_compression(direction);
            },
        )
        .add_setting(
            "Tilt",
            |out, _engine_state, _backend_state, view_state| {
                write!(out, "{:?}", view_state.tilt.curr_option())
            },
            |_engine, view_state, direction| {
                view_state.tilt.switch(direction);
            },
        )
        .add_setting(
            "Inclination",
            |out, _engine_state, _backend_state, view_state| {
                write!(out, "{:?}", view_state.inclination.curr_option())
            },
            |_engine, view_state, direction| {
                view_state.inclination.switch(direction);
            },
        )
        .add_spacer()
        .add_setting(
            "Root Note",
            |out, engine_state, _backend_state, _view_state| {
                write!(
                    out,
                    "{}",
                    engine_state
                        .curr_tuning_layout
                        .kbm
                        .kbm_root()
                        .ref_key
                        .midi_number()
                )
            },
            |engine, _view_state, direction| {
                engine.switch_ref_note(direction);
            },
        )
        .add_setting(
            "Scale Offset",
            |out, engine_state, _backend_state, _view_state| {
                write!(
                    out,
                    "{:+}",
                    engine_state.curr_tuning_layout.kbm.kbm_root().root_offset
                )
            },
            |engine, _view_state, direction| {
                engine.switch_root_offset(direction);
            },
        )
        .add_spacer()
        .add_info(|out, _engine_state, _backend_state, view_state| {
            write!(
                out,
                "Range [Scroll/Alt+Scroll]: {:.0}..{:.0} Hz",
                view_state.viewport_left.as_hz(),
                view_state.viewport_right.as_hz()
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

type RenderFn = Box<
    dyn Fn(&mut String, &PianoEngineState, &BackendState, &ViewState) -> fmt::Result + Send + Sync,
>;

type ActionFn = Box<dyn Fn(&PianoEngine, &mut ResMut<ViewState>, Direction) + Send + Sync>;

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
        render: impl Fn(&mut String, &PianoEngineState, &BackendState, &ViewState) -> fmt::Result
        + Send
        + Sync
        + 'static,
        action: impl Fn(&PianoEngine, &mut ResMut<ViewState>, Direction) + Send + Sync + 'static,
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
        self.add_info(|_, _, _, _| Ok(()))
    }

    fn add_info(
        mut self,
        render: impl Fn(&mut String, &PianoEngineState, &BackendState, &ViewState) -> fmt::Result
        + Send
        + Sync
        + 'static,
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

    pub fn switch(
        &self,
        engine: &PianoEngine,
        view_state: &mut ResMut<ViewState>,
        direction: Direction,
    ) {
        if let MenuEntry::Setting { action, .. } = &self.entries[self.selected_entry] {
            action(engine, view_state, direction);
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

    pub fn render_full(
        &self,
        output: &mut String,
        engine_state: &PianoEngineState,
        backend_state: &BackendState,
        view_state: &ViewState,
    ) {
        for (i, entry) in self.entries.iter().enumerate() {
            let render = match entry {
                MenuEntry::Setting { name, render, .. } => {
                    let selector = if i == self.selected_entry { "> " } else { "  " };
                    write!(
                        output,
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

            render(output, engine_state, backend_state, view_state).unwrap();
            writeln!(output).unwrap();
        }
        writeln!(output, "[Alt]+Letter = quick select").unwrap();
    }

    pub fn render_light(
        &self,
        output: &mut String,
        engine_state: &PianoEngineState,
        backend_state: &BackendState,
        _view_state: &ViewState,
    ) {
        let layout = &engine_state.curr_tuning_layout;

        writeln!(output, "Tuning: {}", layout.scl.description()).unwrap();
        writeln!(output, "Layout: {}", layout.fmt_layout()).unwrap();
        writeln!(output, "Schema: {}", layout.fmt_schema(true)).unwrap();

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
        .filter(|&(_, p)| engine_state.storage.is_active(p))
        .map(|(i, p)| format!("{} (cc {})", i + 1, engine_state.mapper.get_ccn(p).unwrap()))
        .collect::<Vec<_>>()
        .join(", ");

        if !effects.is_empty() {
            writeln!(output, "Effects: {}", effects).unwrap();
        }

        for recorder_detail in backend_state.recorder_details.values() {
            writeln!(output, "{}", recorder_detail).unwrap();
        }

        writeln!(output, "Press [F1-F10] for effects").unwrap();
        writeln!(output, "Press [Alt] for options").unwrap();
    }
}
