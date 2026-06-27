pub mod menu;
pub mod view_settings;

use std::fmt::Write;

use bevy::prelude::*;
use flume::Receiver;
pub use menu::MenuResource;
pub use view_settings::ViewSettings;

use crate::control::LiveParameter;
use crate::pipeline::PipelineEvent;
use crate::toggle::Direction;
use crate::tuning_layout::OnScreenKeyboards;

#[derive(Resource)]
pub struct PipelineEventsResource(pub Receiver<PipelineEvent>);

pub fn build_menu() -> MenuResource {
    MenuResource::new()
        .add_setting(
            "Tuning",
            |out, engine_state, _backend_state, _view_settings| {
                write!(
                    out,
                    "{} - {}",
                    engine_state.scale_index + 1,
                    engine_state.curr_tuning_layout.scl.description()
                )
            },
            |engine, _view_settings, direction| {
                engine.switch_tuning(direction);
            },
        )
        .add_setting(
            "Tuning Mode",
            |out, engine_state, _backend_state, _view_settings| {
                write!(out, "{:?}", engine_state.tuning_mode.curr_option())
            },
            |engine, _view_settings, direction| {
                engine.switch_tuning_mode(direction);
            },
        )
        .add_spacer()
        .add_setting(
            "Output Target",
            |out, _engine_state, backend_state, _view_settings| {
                out.push_str(&backend_state.backend);
                Ok(())
            },
            |engine, _view_settings, direction| {
                engine.switch_backend(direction);
            },
        )
        .add_setting(
            "Bank",
            |out, _engine_state, backend_state, _view_settings| match &backend_state.bank {
                Some(bank) => {
                    out.push_str(bank);
                    Ok(())
                }
                None => write!(out, "-"),
            },
            |engine, _view_settings, direction| {
                engine.switch_bank(direction);
            },
        )
        .add_setting(
            "Program",
            |out, _engine_state, backend_state, _view_settings| match &backend_state.program {
                Some(program) => {
                    out.push_str(program);
                    Ok(())
                }
                None => write!(out, "-"),
            },
            |engine, _view_settings, direction| {
                engine.switch_program(direction);
            },
        )
        .add_setting(
            "Envelope",
            |out, _engine_state, backend_state, _view_settings| match &backend_state.envelope {
                Some(envelope) => {
                    out.push_str(envelope);
                    Ok(())
                }
                None => write!(out, "-"),
            },
            |engine, _view_settings, direction| {
                engine.switch_envelope_type(direction);
            },
        )
        .add_setting(
            "Legato",
            |out, engine_state, _backend_state, _view_settings| {
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
            |engine, _view_settings, direction| {
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
            |out, _engine_state, _backend_state, view_settings| match view_settings
                .on_screen_keyboard
                .curr_option()
            {
                OnScreenKeyboards::IsomorphicAndReference => write!(out, "Isomorphic + Reference"),
                OnScreenKeyboards::ScaleAndReference => write!(out, "Scale + Reference"),
                other => write!(out, "{:?}", other),
            },
            |_engine, view_settings, direction| {
                view_settings.on_screen_keyboard.switch(direction);
            },
        )
        .add_setting(
            "Compression",
            |out, engine_state, _backend_state, _view_settings| {
                write!(
                    out,
                    "{:?}",
                    engine_state.curr_tuning_layout.compression.curr_option()
                )
            },
            |engine, _view_settings, direction| {
                engine.switch_compression(direction);
            },
        )
        .add_setting(
            "Scale",
            |out, engine_state, _backend_state, _view_settings| {
                let (primary_step, secondary_step, sharpness) =
                    engine_state.curr_tuning_layout.scale_step_sizes();
                write!(
                    out,
                    "{} | primary = {}, secondary = {}, sharpness = {}",
                    engine_state.curr_tuning_layout.scale_name(),
                    primary_step,
                    secondary_step,
                    sharpness
                )
            },
            |engine, _view_settings, direction| {
                engine.switch_scale(direction);
            },
        )
        .add_setting(
            "Layout",
            |out, engine_state, _backend_state, _view_settings| {
                let (east_step, south_east_step) =
                    engine_state.curr_tuning_layout.layout_step_sizes();
                write!(
                    out,
                    "{} | east = {}, south-east = {}, north-east = {}",
                    engine_state.curr_tuning_layout.layout_name(),
                    east_step,
                    south_east_step,
                    east_step - south_east_step
                )
            },
            |engine, _view_settings, direction| {
                engine.switch_layout(direction);
            },
        )
        .add_setting(
            "Tilt",
            |out, _engine_state, _backend_state, view_settings| {
                write!(out, "{:?}", view_settings.tilt.curr_option())
            },
            |_engine, view_settings, direction| {
                view_settings.tilt.switch(direction);
            },
        )
        .add_setting(
            "Inclination",
            |out, _engine_state, _backend_state, view_settings| {
                write!(out, "{:?}", view_settings.inclination.curr_option())
            },
            |_engine, view_settings, direction| {
                view_settings.inclination.switch(direction);
            },
        )
        .add_spacer()
        .add_setting(
            "Root Note",
            |out, engine_state, _backend_state, _view_settings| {
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
            |engine, _view_settings, direction| {
                engine.switch_ref_note(direction);
            },
        )
        .add_setting(
            "Scale Offset",
            |out, engine_state, _backend_state, _view_settings| {
                write!(
                    out,
                    "{:+}",
                    engine_state.curr_tuning_layout.kbm.kbm_root().root_offset
                )
            },
            |engine, _view_settings, direction| {
                engine.switch_root_offset(direction);
            },
        )
        .add_spacer()
        .add_info(|out, _engine_state, _backend_state, view_settings| {
            write!(
                out,
                "Range [Scroll/Alt+Scroll]: {:.0}..{:.0} Hz",
                view_settings.viewport_left.as_hz(),
                view_settings.viewport_right.as_hz()
            )
        })
}
