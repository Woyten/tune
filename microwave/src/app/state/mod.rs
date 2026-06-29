mod backend;
mod menu;
mod view;

use std::fmt;
use std::fmt::Display;

pub use backend::BackendState;
use bevy::prelude::*;
use flume::Receiver;
pub use menu::Menu;
use tune_cli::shared::midi::TuningMethod;
pub use view::ViewState;

use crate::piano::PianoEngine;
use crate::piano::PianoEngineState;
use crate::pipeline::NoAudioEvent;
use crate::pipeline::PipelineEvent;

pub struct StatePlugin {
    pub engine: PianoEngine,
    pub events: Receiver<PipelineEvent>,
    pub odd_limit: u16,
}

impl Plugin for StatePlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(self.engine.clone())
            .insert_resource(self.engine.capture_state())
            .insert_resource(PipelineEventsResource(self.events.clone()))
            .insert_resource(BackendState::default())
            .insert_resource(ViewState::new(self.odd_limit))
            .insert_resource(menu::build_menu())
            .add_systems(PreUpdate, (handle_engine_state, handle_backend_state));
    }
}

#[derive(Resource)]
struct PipelineEventsResource(Receiver<PipelineEvent>);

fn handle_engine_state(engine: Res<PianoEngine>, mut engine_state: ResMut<PianoEngineState>) {
    engine.capture_state_into(&mut engine_state);
}

fn handle_backend_state(events: Res<PipelineEventsResource>, mut aggregate: ResMut<BackendState>) {
    fn fmt_option<T: Display>(opt: &Option<T>) -> impl Display {
        fmt::from_fn(move |f| match opt {
            Some(v) => write!(f, "{}", v),
            None => write!(f, "-"),
        })
    }

    for event in events.0.try_iter() {
        match event {
            PipelineEvent::WavRecorder(event) => match event.file_name {
                Some(file_name) => {
                    aggregate.recorder_details.insert(
                        event.index,
                        format!(
                            "Recording buffers {} and {} into {}",
                            event.in_buffers.0, event.in_buffers.1, file_name
                        ),
                    );
                }
                None => {
                    aggregate.recorder_details.remove(&event.index);
                }
            },
            PipelineEvent::Magnetron(event) => {
                aggregate.backend = "Magnetron".to_owned();
                aggregate.program = Some(format!(
                    "{} - {}",
                    event.waveform_number, event.waveform_name
                ));
                aggregate.bank = None;
                aggregate.envelope = Some(format!(
                    "{}{}",
                    event.envelope_name,
                    if event.is_default_envelope {
                        " (default)"
                    } else {
                        ""
                    }
                ));
            }
            PipelineEvent::Fluid(event) => {
                aggregate.backend = format!(
                    "Fluid | {} | {}",
                    event.soundfont_location,
                    match event.is_tuned {
                        true => "Single Note Tuning Change",
                        false => "Warning: Tuning channels exceeded! Change tuning mode.",
                    },
                );
                aggregate.program = event
                    .program
                    .as_ref()
                    .map(|(number, name)| format!("{number} - {name}"));
                aggregate.bank = None;
                aggregate.envelope = None;
            }
            PipelineEvent::FluidError(error) => {
                aggregate.backend = format!(
                    "Fluid | {} | Error: {}",
                    error.soundfont_location, error.error_message
                );
                aggregate.program = None;
                aggregate.bank = None;
                aggregate.envelope = None;
            }
            PipelineEvent::MidiOut(event) => {
                aggregate.backend = format!(
                    "MIDI Out | {} | {}",
                    event.device,
                    match event.tuning_method {
                        Some(TuningMethod::FullKeyboard) => "Single Note Tuning Change",
                        Some(TuningMethod::FullKeyboardRt) =>
                            "Single Note Tuning Change (real-time)",
                        Some(TuningMethod::Octave1) => "Scale/Octave Tuning (1-Byte)",
                        Some(TuningMethod::Octave1Rt) => "Scale/Octave Tuning (1-Byte) (real-time)",
                        Some(TuningMethod::Octave2) => "Scale/Octave Tuning (2-Byte)",
                        Some(TuningMethod::Octave2Rt) => "Scale/Octave Tuning (2-Byte) (real-time)",
                        Some(TuningMethod::ChannelFineTuning) => "Channel Fine Tuning",
                        Some(TuningMethod::PitchBend) => "Pitch Bend",
                        None => "Warning: Tuning channels exceeded! Change tuning mode.",
                    },
                );
                aggregate.program = Some(format!("{}", event.program_number));
                aggregate.bank = Some(format!(
                    "{}/{}",
                    fmt_option(&event.bank_msb),
                    fmt_option(&event.bank_lsb)
                ));
                aggregate.envelope = None;
            }
            PipelineEvent::MidiOutError(error) => {
                aggregate.backend = format!("MIDI Out | Error: {}", error.error_message);
                aggregate.program = None;
                aggregate.bank = None;
                aggregate.envelope = None;
            }
            PipelineEvent::NoAudio(NoAudioEvent) => {
                aggregate.backend = "No Audio".to_owned();
                aggregate.program = None;
                aggregate.bank = None;
                aggregate.envelope = None;
            }
        }
    }
}
