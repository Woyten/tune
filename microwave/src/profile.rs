use cpal::SampleRate;
use crossbeam::channel::Sender;
use log::info;
use magnetron::{creator::Creator, envelope::EnvelopeSpec};
use serde::{Deserialize, Serialize};
use std::{any::Any, collections::HashMap};
use tune_cli::{CliError, CliResult};

use crate::{
    app::DynViewInfo,
    assets,
    audio::{AudioInSpec, AudioStage},
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
    pub waveform_templates: Vec<TemplateSpec<LfSource<WaveformProperty, LiveParameter>>>,
    pub waveform_envelopes: Vec<NamedEnvelopeSpec<LfSource<WaveformProperty, LiveParameter>>>,
    pub effect_templates: Vec<TemplateSpec<LfSource<NoAccess, LiveParameter>>>,
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
    AudioIn(AudioInSpec<ProfileAutomationSpec>),
    Magnetron(MagnetronSpec),
    Fluid(FluidSpec<ProfileAutomationSpec>),
    MidiOut(MidiOutSpec),
    NoAudio,
    Generator(GeneratorSpec<ProfileAutomationSpec>),
    Processor(ProcessorSpec<ProfileAutomationSpec>),
    MergeProcessor(MergeProcessorSpec<ProfileAutomationSpec>),
    StereoProcessor(StereoProcessorSpec<ProfileAutomationSpec>),
}

type ProfileAutomationSpec = LfSource<NoAccess, LiveParameter>;

impl AudioStageSpec {
    #[allow(clippy::too_many_arguments)]
    pub async fn create(
        &self,
        creator: &Creator<LfSource<NoAccess, LiveParameter>>,
        buffer_size: u32,
        sample_rate: SampleRate,
        info_updates: &Sender<DynViewInfo>,
        waveform_templates: &HashMap<String, LfSource<WaveformProperty, LiveParameter>>,
        waveform_envelopes: &HashMap<
            String,
            EnvelopeSpec<LfSource<WaveformProperty, LiveParameter>>,
        >,
        backends: &mut Backends<SourceId>,
        stages: &mut Vec<AudioStage>,
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
