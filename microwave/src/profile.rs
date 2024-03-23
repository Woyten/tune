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
    pub main_templates: Vec<TemplateSpec<MainAutomationSpec>>,
    pub waveform_templates: Vec<TemplateSpec<WaveformAutomationSpec>>,
    pub waveform_envelopes: Vec<NamedEnvelopeSpec<WaveformAutomationSpec>>,
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
    AudioIn(AudioInSpec<MainAutomationSpec>),
    Magnetron(MagnetronSpec),
    Fluid(FluidSpec<MainAutomationSpec>),
    MidiOut(MidiOutSpec),
    NoAudio,
    Generator(GeneratorSpec<MainAutomationSpec>),
    Processor(ProcessorSpec<MainAutomationSpec>),
    MergeProcessor(MergeProcessorSpec<MainAutomationSpec>),
    StereoProcessor(StereoProcessorSpec<MainAutomationSpec>),
}

pub type MainAutomationSpec = LfSource<NoAccess, LiveParameter>;
pub type MainPipeline = Vec<Stage<MainAutomationSpec>>;
pub type WaveformAutomationSpec = LfSource<WaveformProperty, LiveParameter>;
pub type WaveformPipeline = Vec<Stage<WaveformAutomationSpec>>;

impl AudioStageSpec {
    pub async fn create(
        &self,
        creator: &Creator<MainAutomationSpec>,
        buffer_size: u32,
        sample_rate: SampleRate,
        info_updates: &Sender<DynBackendInfo>,
        waveform_templates: &HashMap<String, WaveformAutomationSpec>,
        waveform_envelopes: &HashMap<String, EnvelopeSpec<WaveformAutomationSpec>>,
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
