use bevy::prelude::*;
use cpal::SampleRate;
use flume::Sender;
use magnetron::{
    automation::{Automated, AutomationFactory},
    stage::Stage,
};
use serde::{Deserialize, Serialize};
use std::{any::Any, collections::HashMap, mem};
use tune_cli::{shared::error::ResultExt, CliResult};

use crate::{
    assets,
    audio::AudioInSpec,
    backend::{Backends, IdleBackend},
    control::LiveParameter,
    fluid::{FluidError, FluidEvent, FluidSpec},
    magnetron::{
        envelope::EnvelopeSpec,
        source::{LfSource, NoAccess},
        waveform::{NamedEnvelopeSpec, WaveformProperty},
        FragmentSpec, GeneratorSpec, MergeProcessorSpec, ProcessorSpec, StereoProcessorSpec,
    },
    midi::{MidiOutError, MidiOutEvent, MidiOutSpec},
    piano::SourceId,
    portable,
    recorder::{WavRecorderEvent, WavRecorderSpec},
    synth::{MagnetronEvent, MagnetronSpec},
};

#[derive(Deserialize, Serialize)]
pub struct MicrowaveProfile {
    pub num_buffers: usize,
    pub audio_buffers: (usize, usize),
    pub globals: Vec<FragmentSpec<MainAutomatableValue>>,
    pub templates: Vec<FragmentSpec<WaveformAutomatableValue>>,
    pub envelopes: Vec<NamedEnvelopeSpec<WaveformAutomatableValue>>,
    pub stages: Vec<AudioStageSpec>,
    pub color_palette: ColorPalette,
}

impl MicrowaveProfile {
    pub async fn load(file_name: &str) -> CliResult<Self> {
        if let Some(data) = portable::read_file(file_name).await? {
            log::info!("Loading config file `{file_name}`");
            serde_yaml::from_reader(data).handle_error("Could not deserialize file")
        } else {
            log::info!("Config file not found. Creating `{file_name}`");
            let profile = assets::get_default_profile();
            let file = portable::write_file(file_name).await?;
            serde_yaml::to_writer(file, &profile)
                .map(|()| profile)
                .handle_error("Could not serialize file")
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "stage_type")]
pub enum AudioStageSpec {
    Reset(MainAutomatableValue),
    AudioIn(AudioInSpec<MainAutomatableValue>),
    WavRecorder(WavRecorderSpec<MainAutomatableValue>),
    Magnetron(MagnetronSpec),
    Fluid(FluidSpec<MainAutomatableValue>),
    MidiOut(MidiOutSpec),
    NoAudio,
    Generator(GeneratorSpec<MainAutomatableValue>),
    Processor(ProcessorSpec<MainAutomatableValue>),
    MergeProcessor(MergeProcessorSpec<MainAutomatableValue>),
    StereoProcessor(StereoProcessorSpec<MainAutomatableValue>),
}

pub type MainAutomatableValue = LfSource<NoAccess, LiveParameter>;
pub type MainPipeline = Vec<Stage<MainAutomatableValue>>;
pub type WaveformAutomatableValue = LfSource<WaveformProperty, LiveParameter>;
pub type WaveformPipeline = Vec<Stage<WaveformAutomatableValue>>;

impl AudioStageSpec {
    pub async fn create(
        &self,
        buffer_size: u32,
        sample_rate: SampleRate,
        factory: &mut AutomationFactory<MainAutomatableValue>,
        templates: &HashMap<String, WaveformAutomatableValue>,
        envelopes: &HashMap<String, EnvelopeSpec<WaveformAutomatableValue>>,
        stages: &mut MainPipeline,
        backends: &mut Backends<SourceId>,
        resources: &mut Resources,
        events: &Sender<PipelineEvent>,
    ) -> CliResult {
        match self {
            AudioStageSpec::Reset(reset) => {
                stages.push(factory.automate(reset).into_stage({
                    let mut is_above_threshold = false;
                    move |buffers, reset| {
                        let was_above_threshold =
                            mem::replace(&mut is_above_threshold, reset >= 0.5);

                        if is_above_threshold && !was_above_threshold {
                            buffers.set_reset();
                        }

                        magnetron::stage::StageActivity::Observer
                    }
                }));
            }
            AudioStageSpec::AudioIn(spec) => {
                spec.create(buffer_size, sample_rate, factory, stages, resources)
            }
            AudioStageSpec::WavRecorder(spec) => spec.create(factory, stages, events),
            AudioStageSpec::Magnetron(spec) => spec.create(
                buffer_size,
                sample_rate,
                templates,
                envelopes,
                stages,
                backends,
                events,
            ),
            AudioStageSpec::Fluid(spec) => {
                spec.create(sample_rate, factory, stages, backends, events)
                    .await
            }
            AudioStageSpec::MidiOut(spec) => spec.create(backends, events)?,
            AudioStageSpec::NoAudio => {
                backends.push(Box::new(IdleBackend::new(events, NoAudioEvent)))
            }
            AudioStageSpec::Generator(spec) => stages.push(spec.create(factory)),
            AudioStageSpec::Processor(spec) => stages.push(spec.create(factory)),
            AudioStageSpec::MergeProcessor(spec) => stages.push(spec.create(factory)),
            AudioStageSpec::StereoProcessor(spec) => stages.push(spec.create(factory)),
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ColorPalette {
    pub root_color: Srgba,
    pub natural_color: Srgba,
    pub sharp_colors: Vec<Srgba>,
    pub flat_colors: Vec<Srgba>,
    pub enharmonic_colors: Vec<Srgba>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NoAudioEvent;

pub type Resources = Vec<Box<dyn Any>>;

pub enum PipelineEvent {
    WaveRecorder(WavRecorderEvent),
    Magnetron(MagnetronEvent),
    Fluid(FluidEvent),
    FluidError(FluidError),
    MidiOut(MidiOutEvent),
    MidiOutError(MidiOutError),
    NoAudio(NoAudioEvent),
}

impl From<WavRecorderEvent> for PipelineEvent {
    fn from(event: WavRecorderEvent) -> Self {
        PipelineEvent::WaveRecorder(event)
    }
}

impl From<MagnetronEvent> for PipelineEvent {
    fn from(event: MagnetronEvent) -> Self {
        PipelineEvent::Magnetron(event)
    }
}

impl From<FluidEvent> for PipelineEvent {
    fn from(event: FluidEvent) -> Self {
        PipelineEvent::Fluid(event)
    }
}

impl From<FluidError> for PipelineEvent {
    fn from(error: FluidError) -> Self {
        PipelineEvent::FluidError(error)
    }
}

impl From<MidiOutEvent> for PipelineEvent {
    fn from(event: MidiOutEvent) -> Self {
        PipelineEvent::MidiOut(event)
    }
}

impl From<MidiOutError> for PipelineEvent {
    fn from(event: MidiOutError) -> Self {
        PipelineEvent::MidiOutError(event)
    }
}

impl From<NoAudioEvent> for PipelineEvent {
    fn from(event: NoAudioEvent) -> Self {
        PipelineEvent::NoAudio(event)
    }
}
