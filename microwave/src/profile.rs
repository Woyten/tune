use cpal::SampleRate;
use crossbeam::channel::Sender;
use magnetron::{creator::Creator, envelope::EnvelopeSpec};
use serde::{Deserialize, Serialize};
use std::{any::Any, collections::HashMap, fs::File, path::Path};
use tune_cli::{CliError, CliResult};

use crate::{
    assets,
    audio::{AudioInSpec, AudioStage},
    control::LiveParameter,
    fluid::FluidSpec,
    magnetron::{
        effects::EffectSpec,
        source::{LfSource, NoAccess},
        NamedEnvelopeSpec, TemplateSpec, WaveformProperty,
    },
    midi::MidiOutSpec,
    model::SourceId,
    piano::{Backend, DummyBackend},
    synth::MagnetronSpec,
    view::DynViewInfo,
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
    pub fn load(location: &Path) -> CliResult<Self> {
        if location.exists() {
            println!("[INFO] Loading config file `{}`", location.display());
            let file = File::open(location)?;
            serde_yaml::from_reader(file)
                .map_err(|err| CliError::CommandError(format!("Could not deserialize file: {err}")))
        } else {
            println!(
                "[INFO] Config file not found. Creating `{}`",
                location.display()
            );
            let profile = assets::get_default_profile();
            let file = File::create(location)?;
            serde_yaml::to_writer(file, &profile).map_err(|err| {
                CliError::CommandError(format!("Could not serialize file: {err}"))
            })?;
            Ok(profile)
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum AudioStageSpec {
    NoAudio,
    Magnetron(MagnetronSpec),
    Fluid(FluidSpec),
    MidiOut(MidiOutSpec),
    Effect(EffectSpec<LfSource<NoAccess, LiveParameter>>),
    AudioIn(AudioInSpec),
}

impl AudioStageSpec {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        &self,
        creator: &Creator<LfSource<NoAccess, LiveParameter>>,
        buffer_size: u32,
        sample_rate: SampleRate,
        info_sender: &Sender<DynViewInfo>,
        waveform_templates: &HashMap<String, LfSource<WaveformProperty, LiveParameter>>,
        waveform_envelopes: &HashMap<
            String,
            EnvelopeSpec<LfSource<WaveformProperty, LiveParameter>>,
        >,
        backends: &mut Vec<Box<dyn Backend<SourceId>>>,
        stages: &mut Vec<AudioStage>,
        resources: &mut Resources,
    ) -> CliResult {
        match self {
            AudioStageSpec::NoAudio => {
                backends.push(Box::new(DummyBackend::new(info_sender, NoAudioInfo)))
            }
            AudioStageSpec::AudioIn(spec) => {
                spec.create(creator, buffer_size, sample_rate, stages, resources)
            }
            AudioStageSpec::Magnetron(spec) => spec.create(
                info_sender,
                buffer_size,
                sample_rate,
                waveform_templates,
                waveform_envelopes,
                backends,
                stages,
            ),
            AudioStageSpec::Fluid(spec) => spec.create(info_sender, sample_rate, backends, stages),
            AudioStageSpec::MidiOut(spec) => spec.create(info_sender, backends)?,
            AudioStageSpec::Effect(spec) => stages.push(spec.use_creator(creator)),
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct NoAudioInfo;

pub type Resources = Vec<Box<dyn Any>>;
