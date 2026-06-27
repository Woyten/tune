use std::fmt;
use std::fmt::Write;

use bevy::prelude::*;

use crate::app::resources::ViewSettings;
use crate::app::view::PipelineAggregate;
use crate::control::LiveParameter;
use crate::piano::PianoEngine;
use crate::piano::PianoEngineState;
use crate::toggle::Direction;

#[derive(Resource)]
pub struct MenuResource {
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
    dyn Fn(&mut String, &PianoEngineState, &PipelineAggregate, &ViewSettings) -> fmt::Result
        + Send
        + Sync,
>;

type ActionFn = Box<dyn Fn(&PianoEngine, &mut ResMut<ViewSettings>, Direction) + Send + Sync>;

impl MenuResource {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            selected_entry: 0,
            max_setting_width: 0,
        }
    }

    pub fn add_setting(
        mut self,
        name: &'static str,
        render: impl Fn(
            &mut String,
            &PianoEngineState,
            &PipelineAggregate,
            &ViewSettings,
        ) -> fmt::Result
        + Send
        + Sync
        + 'static,
        action: impl Fn(&PianoEngine, &mut ResMut<ViewSettings>, Direction) + Send + Sync + 'static,
    ) -> Self {
        self.max_setting_width = self.max_setting_width.max(name.len());
        self.entries.push(MenuEntry::Setting {
            name,
            render: Box::new(render),
            action: Box::new(action),
        });
        self
    }

    pub fn add_spacer(self) -> Self {
        self.add_info(|_, _, _, _| Ok(()))
    }

    pub fn add_info(
        mut self,
        render: impl Fn(
            &mut String,
            &PianoEngineState,
            &PipelineAggregate,
            &ViewSettings,
        ) -> fmt::Result
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
        view_settings: &mut ResMut<ViewSettings>,
        direction: Direction,
    ) {
        if let MenuEntry::Setting { action, .. } = &self.entries[self.selected_entry] {
            action(engine, view_settings, direction);
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
        backend_state: &PipelineAggregate,
        view_settings: &ViewSettings,
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

            render(output, engine_state, backend_state, view_settings).unwrap();
            writeln!(output).unwrap();
        }
        writeln!(output, "[Alt]+Letter = quick select").unwrap();
    }

    pub fn render_light(
        &self,
        output: &mut String,
        engine_state: &PianoEngineState,
        backend_state: &PipelineAggregate,
        _view_settings: &ViewSettings,
    ) {
        let layout = &engine_state.curr_tuning_layout;
        let (primary_step, secondary_step, sharpness) = layout.scale_step_sizes();
        let (east_step, south_east_step) = layout.layout_step_sizes();

        writeln!(output, "Tuning: {}", layout.scl.description()).unwrap();
        writeln!(
            output,
            "Scale: {} | primary = {}, secondary = {}, sharpness = {}",
            layout.scale_name(),
            primary_step,
            secondary_step,
            sharpness
        )
        .unwrap();
        writeln!(
            output,
            "Layout: {} | east = {}, south-east = {}, north-east = {}",
            layout.layout_name(),
            east_step,
            south_east_step,
            east_step - south_east_step
        )
        .unwrap();

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
