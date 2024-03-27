use cpal::SampleRate;
use crossbeam::channel::Sender;
use log::info;
use magnetron::{creator::Creator, envelope::EnvelopeSpec, stage::Stage};
use serde::{Deserialize, Serialize};
use std::{any::Any, collections::HashMap};
use tune_cli::{CliError, CliResult};

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
        GeneratorSpec, MergeProcessorSpec, ProcessorSpec, StereoProcessorSpec, TemplateSpec,
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
    pub main_templates: Vec<TemplateSpec<MainAutomatableValue>>,
    pub waveform_templates: Vec<TemplateSpec<WaveformAutomatableValue>>,
    pub waveform_envelopes: Vec<NamedEnvelopeSpec<WaveformAutomatableValue>>,
    pub stages: Vec<AudioStageSpec>,
}

impl MicrowaveProfile {
    pub async fn load(file_name: &str) -> CliResult<Self> {
        if let Some(data) = portable::read_file(file_name).await? {
            info!("Loading config file `{}`", file_name);
            serde_yaml::from_reader(data)
                .map_err(|err| CliError::CommandError(format!("Could not deserialize file: {err}")))
        } else {
            info!("Config file not found. Creating `{}`", file_name);
            let profile = assets::get_default_profile();
            let file = portable::write_file(file_name).await?;
            serde_yaml::to_writer(file, &profile).map_err(|err| {
                CliError::CommandError(format!("Could not serialize file: {err}"))
            })?;
            Ok(profile)
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
        creator: &Creator<MainAutomatableValue>,
        buffer_size: u32,
        sample_rate: SampleRate,
        info_updates: &Sender<DynBackendInfo>,
        waveform_templates: &HashMap<String, WaveformAutomatableValue>,
        waveform_envelopes: &HashMap<String, EnvelopeSpec<WaveformAutomatableValue>>,
        backends: &mut Backends<SourceId>,
        stages: &mut MainPipeline,
        resources: &mut Resources,
    ) -> CliResult {
        match self {
            AudioStageSpec::AudioIn(spec) => {
                spec.create(creator, buffer_size, sample_rate, stages, resources)
            }
            AudioStageSpec::Magnetron(spec) => spec.create(
                info_updates,
                buffer_size,
                sample_rate,
                waveform_templates,
                waveform_envelopes,
                backends,
                stages,
            ),
            AudioStageSpec::Fluid(spec) => {
                spec.create(info_updates, creator, sample_rate, backends, stages)
                    .await
            }
            AudioStageSpec::MidiOut(spec) => spec.create(info_updates, backends)?,
            AudioStageSpec::NoAudio => {
                backends.push(Box::new(IdleBackend::new(info_updates, NoAudioInfo)))
            }
            AudioStageSpec::Generator(spec) => stages.push(spec.use_creator(creator)),
            AudioStageSpec::Processor(spec) => stages.push(spec.use_creator(creator)),
            AudioStageSpec::MergeProcessor(spec) => stages.push(spec.use_creator(creator)),
            AudioStageSpec::StereoProcessor(spec) => stages.push(spec.use_creator(creator)),
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NoAudioInfo;

pub type Resources = Vec<Box<dyn Any>>;
