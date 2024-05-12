use cpal::SampleRate;
use flume::Sender;
use magnetron::{automation::AutomationFactory, envelope::EnvelopeSpec, stage::Stage};
use serde::{Deserialize, Serialize};
use std::{any::Any, collections::HashMap};
use tune_cli::{shared::error::ResultExt, CliResult};

use crate::{
    app::DynBackendInfo,
    assets,
    audio::AudioInSpec,
    backend::{Backends, IdleBackend},
    control::LiveParameter,
    fluid::FluidSpec,
    magnetron::{
        source::{LfSource, NoAccess},
        waveform::{NamedEnvelopeSpec, WaveformProperty},
        FragmentSpec, GeneratorSpec, MergeProcessorSpec, ProcessorSpec, StereoProcessorSpec,
    },
    midi::MidiOutSpec,
    piano::SourceId,
    portable,
    synth::MagnetronSpec,
};

#[derive(Deserialize, Serialize)]
pub struct MicrowaveProfile {
    pub num_buffers: usize,
    pub audio_buffers: (usize, usize),
    pub globals: Vec<FragmentSpec<MainAutomatableValue>>,
    pub templates: Vec<FragmentSpec<WaveformAutomatableValue>>,
    pub envelopes: Vec<NamedEnvelopeSpec<WaveformAutomatableValue>>,
    pub stages: Vec<AudioStageSpec>,
}

impl MicrowaveProfile {
    pub async fn load(file_name: &str) -> CliResult<Self> {
        if let Some(data) = portable::read_file(file_name).await? {
            log::info!("Loading config file `{}`", file_name);
            serde_yaml::from_reader(data).handle_error("Could not deserialize file")
        } else {
            log::info!("Config file not found. Creating `{}`", file_name);
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
    AudioIn(AudioInSpec<MainAutomatableValue>),
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
        factory: &mut AutomationFactory<MainAutomatableValue>,
        buffer_size: u32,
        sample_rate: SampleRate,
        info_updates: &Sender<DynBackendInfo>,
        templates: &HashMap<String, WaveformAutomatableValue>,
        envelopes: &HashMap<String, EnvelopeSpec<WaveformAutomatableValue>>,
        backends: &mut Backends<SourceId>,
        stages: &mut MainPipeline,
        resources: &mut Resources,
    ) -> CliResult {
        match self {
            AudioStageSpec::AudioIn(spec) => {
                spec.create(factory, buffer_size, sample_rate, stages, resources)
            }
            AudioStageSpec::Magnetron(spec) => spec.create(
                info_updates,
                buffer_size,
                sample_rate,
                templates,
                envelopes,
                backends,
                stages,
            ),
            AudioStageSpec::Fluid(spec) => {
                spec.create(info_updates, factory, sample_rate, backends, stages)
                    .await
            }
            AudioStageSpec::MidiOut(spec) => spec.create(info_updates, backends)?,
            AudioStageSpec::NoAudio => {
                backends.push(Box::new(IdleBackend::new(info_updates, NoAudioInfo)))
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
pub struct NoAudioInfo;

pub type Resources = Vec<Box<dyn Any>>;
