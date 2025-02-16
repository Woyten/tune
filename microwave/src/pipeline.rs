use std::any::Any;
use std::collections::HashMap;
use std::mem;

use flume::Receiver;
use flume::Sender;
use magnetron::automation::Automated;
use magnetron::automation::AutomatedValue;
use magnetron::automation::AutomationFactory;
use magnetron::buffer::BufferWriter;
use magnetron::stage::Stage;
use magnetron::Magnetron;
use serde::Deserialize;
use serde::Serialize;
use tune_cli::CliResult;

use crate::audio::AudioInSpec;
use crate::backend::Backends;
use crate::backend::DynBackend;
use crate::backend::IdleBackend;
use crate::control::LiveParameterStorage;
use crate::fluid::FluidError;
use crate::fluid::FluidEvent;
use crate::fluid::FluidSpec;
use crate::magnetron::envelope::EnvelopeSpec;
use crate::magnetron::GeneratorSpec;
use crate::magnetron::MergeProcessorSpec;
use crate::magnetron::ProcessorSpec;
use crate::magnetron::StereoProcessorSpec;
use crate::midi::MidiOutError;
use crate::midi::MidiOutEvent;
use crate::midi::MidiOutSpec;
use crate::piano::SourceId;
use crate::profile::MicrowaveProfile;
use crate::profile::PipelineParam;
use crate::profile::WaveformParam;
use crate::recorder::WavRecorderEvent;
use crate::recorder::WavRecorderSpec;
use crate::synth::MagnetronEvent;
use crate::synth::MagnetronSpec;
use crate::Resources;

#[derive(Deserialize, Serialize)]
#[serde(tag = "stage_type")]
pub enum PipelineStageSpec {
    Reset(PipelineParam),
    AudioIn(AudioInSpec<PipelineParam>),
    WavRecorder(WavRecorderSpec<PipelineParam>),
    Magnetron(MagnetronSpec),
    Fluid(FluidSpec<PipelineParam>),
    MidiOut(MidiOutSpec),
    NoAudio,
    Generator(GeneratorSpec<PipelineParam>),
    Processor(ProcessorSpec<PipelineParam>),
    MergeProcessor(MergeProcessorSpec<PipelineParam>),
    StereoProcessor(StereoProcessorSpec<PipelineParam>),
}

pub struct AudioPipeline {
    magnetron: Magnetron,
    audio_buffers: (usize, usize),
    stages: Vec<Stage<PipelineParam>>,
    storage: LiveParameterStorage,
    storage_updates: Receiver<LiveParameterStorage>,
    globals: Vec<(String, AutomatedValue<PipelineParam>)>,
    globals_evaluated: HashMap<String, f64>,
}

impl AudioPipeline {
    pub async fn create(
        resources: &mut Resources,
        buffer_size: u32,
        sample_rate: u32,
        profile: MicrowaveProfile,
        inital_storage: LiveParameterStorage,
    ) -> CliResult<(
        Self,
        Vec<DynBackend<SourceId>>,
        Sender<LiveParameterStorage>,
        Receiver<PipelineEvent>,
    )> {
        let (storage_send, storage_recv) = flume::unbounded();
        let (events_send, events_recv) = flume::unbounded();

        let mut backends = Vec::new();
        let mut stages = Vec::new();

        let mut factory = AutomationFactory::new(HashMap::new());

        let globals = profile
            .globals
            .into_iter()
            .map(|spec| (spec.name, factory.automate(spec.value)))
            .collect::<Vec<_>>();

        let templates = profile
            .templates
            .into_iter()
            .map(|spec| (spec.name, spec.value))
            .collect();

        let envelopes = profile
            .envelopes
            .into_iter()
            .map(|spec| (spec.name, spec.spec))
            .collect();

        for stage in profile.stages {
            stage
                .create(
                    resources,
                    buffer_size,
                    sample_rate,
                    &mut factory,
                    &templates,
                    &envelopes,
                    &mut stages,
                    &mut backends,
                    &events_send,
                )
                .await?;
        }

        Ok((
            Self {
                magnetron: Magnetron::new(
                    f64::from(sample_rate).recip(),
                    profile.num_buffers,
                    2 * usize::try_from(buffer_size).unwrap(),
                ), // The first invocation of cpal uses the double buffer size
                audio_buffers: profile.audio_buffers,
                stages,
                storage: inital_storage,
                storage_updates: storage_recv,
                globals_evaluated: globals
                    .iter()
                    .map(|(name, _)| (name.to_owned(), 0.0))
                    .collect(),
                globals,
            },
            backends,
            storage_send,
            events_recv,
        ))
    }

    pub fn audio_buffers(&self) -> (usize, usize) {
        self.audio_buffers
    }

    pub fn render(&mut self, num_samples: usize) -> BufferWriter<'_> {
        for storage_update in self.storage_updates.try_iter() {
            self.storage = storage_update;
        }

        let mut buffers = self.magnetron.prepare(num_samples);

        let render_window_secs = buffers.render_window_secs();
        for (name, global) in &mut self.globals {
            let curr_value = global.query(
                render_window_secs,
                (&(), &self.storage, &self.globals_evaluated),
            );

            if let Some(global_evaluated) = self.globals_evaluated.get_mut(name) {
                *global_evaluated = curr_value;
            }
        }

        buffers.process(
            (&(), &self.storage, &self.globals_evaluated),
            self.stages.iter_mut(),
        );

        buffers
    }
}

impl PipelineStageSpec {
    async fn create(
        &self,
        resources: &mut Vec<Box<dyn Any>>,
        buffer_size: u32,
        sample_rate: u32,
        factory: &mut AutomationFactory<PipelineParam>,
        templates: &HashMap<String, WaveformParam>,
        envelopes: &HashMap<String, EnvelopeSpec<WaveformParam>>,
        stages: &mut Vec<Stage<PipelineParam>>,
        backends: &mut Backends<SourceId>,
        events: &Sender<PipelineEvent>,
    ) -> CliResult {
        match self {
            PipelineStageSpec::Reset(reset) => {
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
            PipelineStageSpec::AudioIn(spec) => {
                spec.create(resources, buffer_size, sample_rate, factory, stages)
            }
            PipelineStageSpec::WavRecorder(spec) => spec.create(factory, stages, events),
            PipelineStageSpec::Magnetron(spec) => spec.create(
                buffer_size,
                sample_rate,
                templates,
                envelopes,
                stages,
                backends,
                events,
            ),
            PipelineStageSpec::Fluid(spec) => {
                spec.create(sample_rate, factory, stages, backends, events)
                    .await
            }
            PipelineStageSpec::MidiOut(spec) => spec.create(backends, events)?,
            PipelineStageSpec::NoAudio => {
                backends.push(Box::new(IdleBackend::new(events, NoAudioEvent)))
            }
            PipelineStageSpec::Generator(spec) => stages.push(spec.create(factory)),
            PipelineStageSpec::Processor(spec) => stages.push(spec.create(factory)),
            PipelineStageSpec::MergeProcessor(spec) => stages.push(spec.create(factory)),
            PipelineStageSpec::StereoProcessor(spec) => stages.push(spec.create(factory)),
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NoAudioEvent;

#[derive(Debug, PartialEq, Eq)]
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
