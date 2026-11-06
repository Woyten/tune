use std::sync::Arc;

use magnetron::automation::Automatable;
use magnetron::automation::AutomatableParam;
use magnetron::automation::Automated;
use magnetron::automation::AutomationFactory;
use magnetron::automation::CreationInfo;
use magnetron::buffer::BufferIndex;
use magnetron::stage::Stage;
use serde::Deserialize;
use serde::Serialize;

use crate::magnetron::effects::EffectSpec;
use crate::magnetron::filter::FilterSpec;
use crate::magnetron::noise::NoiseSpec;
use crate::magnetron::oscillator::ModOscillatorSpec;
use crate::magnetron::oscillator::OscillatorSpec;
use crate::magnetron::waveguide::WaveguideSpec;

mod util;

pub mod effects;
pub mod envelope;
pub mod filter;
pub mod noise;
pub mod oscillator;
pub mod source;
pub mod waveform;
pub mod waveguide;

#[derive(Clone, Deserialize, Serialize)]
pub struct FragmentSpec<A> {
    pub name: String,
    pub value: A,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "stage_type")]
pub enum StageType<A> {
    Generator(GeneratorSpec<A>),
    Processor(ProcessorSpec<A>),
    MergeProcessor(MergeProcessorSpec<A>),
    StereoProcessor(StereoProcessorSpec<A>),
}

impl<A: AutomatableParam> StageType<A> {
    pub fn create(&self, factory: &mut Factory<A>) -> Stage<A> {
        match self {
            StageType::Generator(spec) => spec.create(factory),
            StageType::Processor(spec) => spec.create(factory),
            StageType::MergeProcessor(spec) => spec.create(factory),
            StageType::StereoProcessor(spec) => spec.create(factory),
        }
    }
}

pub type BufferName = Arc<str>;

#[derive(Clone, Default)]
pub struct BufferNameMap {
    // Only a handful of buffers exist, so a linear scan is simpler than a hash map.
    names: Vec<BufferName>,
}

impl BufferNameMap {
    pub fn index(&mut self, name: &BufferName) -> usize {
        if let Some(index) = self.index_of(name) {
            return index;
        }
        self.names.push(Arc::clone(name));
        self.names.len() - 1
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.names
            .iter()
            .position(|existing| existing.as_ref() == name)
    }

    pub fn num_buffers(&self) -> usize {
        self.names.len()
    }

    pub fn names(&self) -> &[BufferName] {
        &self.names
    }

    fn clear(&mut self) {
        self.names.clear();
    }
}

pub struct Factory<A: CreationInfo> {
    automation: AutomationFactory<A>,
    internal_buffers: BufferNameMap,
    external_buffers: BufferNameMap,
}

impl<A: CreationInfo> Factory<A> {
    pub fn new(automation: AutomationFactory<A>) -> Self {
        Self {
            automation,
            internal_buffers: BufferNameMap::default(),
            external_buffers: BufferNameMap::default(),
        }
    }

    pub fn with_external_buffers(mut self, external_buffers: BufferNameMap) -> Self {
        self.external_buffers = external_buffers;
        self
    }

    pub fn automation(&mut self) -> &mut AutomationFactory<A> {
        &mut self.automation
    }

    pub fn automate<T: Automatable<A>>(&mut self, automatable: T) -> T::Output {
        self.automation.automate(automatable)
    }

    pub fn internal_buffer_index(&mut self, name: &BufferName) -> BufferIndex {
        BufferIndex::Internal(self.internal_buffers.index(name))
    }

    pub fn external_buffer_index(&mut self, name: &BufferName) -> BufferIndex {
        BufferIndex::External(self.external_buffers.index(name))
    }

    fn buffer_index(&mut self, name: &BufferName, external: bool) -> BufferIndex {
        if external {
            self.external_buffer_index(name)
        } else {
            self.internal_buffer_index(name)
        }
    }

    pub fn input_buffer_index(&mut self, name: &BufferName, external: Option<bool>) -> BufferIndex {
        self.buffer_index(name, external.unwrap_or_default())
    }

    pub fn input_buffer_indices(
        &mut self,
        names: (&BufferName, &BufferName),
        external: Option<(bool, bool)>,
    ) -> (BufferIndex, BufferIndex) {
        let external = external.unwrap_or_default();
        (
            self.buffer_index(names.0, external.0),
            self.buffer_index(names.1, external.1),
        )
    }

    fn buffer_indices(
        &mut self,
        names: (&BufferName, &BufferName),
        external: bool,
    ) -> (BufferIndex, BufferIndex) {
        (
            self.buffer_index(names.0, external),
            self.buffer_index(names.1, external),
        )
    }

    pub fn internal_buffer_indices(
        &mut self,
        names: (&BufferName, &BufferName),
    ) -> (BufferIndex, BufferIndex) {
        self.buffer_indices(names, false)
    }

    pub fn external_buffer_indices(
        &mut self,
        names: (&BufferName, &BufferName),
    ) -> (BufferIndex, BufferIndex) {
        self.buffer_indices(names, true)
    }

    pub fn internal_buffers(&mut self) -> &mut BufferNameMap {
        &mut self.internal_buffers
    }

    pub fn external_buffers(&self) -> &BufferNameMap {
        &self.external_buffers
    }

    pub fn num_internal_buffers(&self) -> usize {
        self.internal_buffers.num_buffers()
    }

    pub fn reset_internal_buffers(&mut self) {
        self.internal_buffers.clear();
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct GeneratorSpec<A> {
    pub out_buffer: BufferName,
    pub out_level: Option<A>,
    #[serde(flatten)]
    pub generator_type: GeneratorType<A>,
}

impl<A: AutomatableParam> GeneratorSpec<A> {
    pub fn create(&self, factory: &mut Factory<A>) -> Stage<A> {
        let out_buffer = factory.internal_buffer_index(&self.out_buffer);
        self.generator_type
            .create(factory.automation(), out_buffer, self.out_level.as_ref())
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProcessorSpec<A> {
    pub in_buffer: BufferName,
    pub in_external: Option<bool>,
    pub out_buffer: BufferName,
    pub out_level: Option<A>,
    #[serde(flatten)]
    pub processor_type: ProcessorType<A>,
}

impl<A: AutomatableParam> ProcessorSpec<A> {
    pub fn create(&self, factory: &mut Factory<A>) -> Stage<A> {
        let in_buffer = factory.input_buffer_index(&self.in_buffer, self.in_external);
        let out_buffer = factory.internal_buffer_index(&self.out_buffer);
        self.processor_type.create(
            factory.automation(),
            in_buffer,
            out_buffer,
            self.out_level.as_ref(),
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct MergeProcessorSpec<A> {
    pub in_buffers: (BufferName, BufferName),
    pub in_external: Option<(bool, bool)>,
    pub out_buffer: BufferName,
    pub out_level: Option<A>,
    #[serde(flatten)]
    pub processor_type: MergeProcessorType,
}

impl<A: AutomatableParam> MergeProcessorSpec<A> {
    pub fn create(&self, factory: &mut Factory<A>) -> Stage<A> {
        let in_buffers = factory
            .input_buffer_indices((&self.in_buffers.0, &self.in_buffers.1), self.in_external);
        let out_buffer = factory.internal_buffer_index(&self.out_buffer);
        self.processor_type.create(
            factory.automation(),
            in_buffers,
            out_buffer,
            self.out_level.as_ref(),
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StereoProcessorSpec<A> {
    pub in_buffers: (BufferName, BufferName),
    pub in_external: Option<(bool, bool)>,
    pub out_buffers: (BufferName, BufferName),
    pub out_levels: Option<(A, A)>,
    #[serde(flatten)]
    pub processor_type: StereoProcessorType<A>,
}

impl<A: AutomatableParam> StereoProcessorSpec<A> {
    pub fn create(&self, factory: &mut Factory<A>) -> Stage<A> {
        let in_buffers = factory
            .input_buffer_indices((&self.in_buffers.0, &self.in_buffers.1), self.in_external);
        let out_buffers =
            factory.internal_buffer_indices((&self.out_buffers.0, &self.out_buffers.1));
        self.processor_type.create(
            factory.automation(),
            in_buffers,
            out_buffers,
            self.out_levels.as_ref(),
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "generator_type")]
pub enum GeneratorType<A> {
    Oscillator(OscillatorSpec<A>),
    Noise(NoiseSpec),
}

impl<A: AutomatableParam> GeneratorType<A> {
    fn create(
        &self,
        factory: &mut AutomationFactory<A>,
        out_buffer: BufferIndex,
        out_level: Option<&A>,
    ) -> Stage<A> {
        match self {
            GeneratorType::Oscillator(spec) => spec.create(factory, out_buffer, out_level),
            GeneratorType::Noise(spec) => spec.create(factory, out_buffer, out_level),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "processor_type")]
pub enum ProcessorType<A> {
    Pass,
    Pow3,
    Clip { limit: A },

    Filter(FilterSpec<A>),
    Oscillator(ModOscillatorSpec<A>),
    Waveguide(WaveguideSpec<A>),
}

impl<A: AutomatableParam> ProcessorType<A> {
    fn create(
        &self,
        factory: &mut AutomationFactory<A>,
        in_buffer: BufferIndex,
        out_buffer: BufferIndex,
        out_level: Option<&A>,
    ) -> Stage<A> {
        match self {
            ProcessorType::Pass => {
                create_1_to_1_processor(factory, in_buffer, out_buffer, out_level, |s| s)
            }
            ProcessorType::Pow3 => {
                create_1_to_1_processor(factory, in_buffer, out_buffer, out_level, |s| s * s * s)
            }
            ProcessorType::Clip { limit } => factory.automate((out_level, limit)).into_stage(
                move |buffers, (out_level, limit)| {
                    let limit = limit.abs();
                    buffers.read_1_write_1(in_buffer, out_buffer, out_level, |s| {
                        s.max(-limit).min(limit)
                    })
                },
            ),
            ProcessorType::Filter(spec) => spec.create(factory, in_buffer, out_buffer, out_level),
            ProcessorType::Oscillator(spec) => {
                spec.create(factory, in_buffer, out_buffer, out_level)
            }
            ProcessorType::Waveguide(spec) => {
                spec.create(factory, in_buffer, out_buffer, out_level)
            }
        }
    }
}

fn create_1_to_1_processor<A: AutomatableParam>(
    factory: &mut AutomationFactory<A>,
    in_buffer: BufferIndex,
    out_buffer: BufferIndex,
    out_level: Option<&A>,
    mut f: impl FnMut(f64) -> f64 + Send + 'static,
) -> Stage<A> {
    factory
        .automate(out_level)
        .into_stage(move |buffers, out_level| {
            buffers.read_1_write_1(in_buffer, out_buffer, out_level, &mut f)
        })
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "processor_type")]
pub enum MergeProcessorType {
    RingModulator,
}

impl MergeProcessorType {
    fn create<A: AutomatableParam>(
        &self,
        factory: &mut AutomationFactory<A>,
        in_buffers: (BufferIndex, BufferIndex),
        out_buffer: BufferIndex,
        out_level: Option<&A>,
    ) -> Stage<A> {
        let MergeProcessorType::RingModulator = self;
        factory
            .automate(out_level)
            .into_stage(move |buffers, out_level| {
                buffers.read_2_write_1(in_buffers, out_buffer, out_level, |source_1, source_2| {
                    source_1 * source_2
                })
            })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "processor_type")]
pub enum StereoProcessorType<A> {
    Effect(EffectSpec<A>),
}

impl<A: AutomatableParam> StereoProcessorType<A> {
    fn create(
        &self,
        factory: &mut AutomationFactory<A>,
        in_buffers: (BufferIndex, BufferIndex),
        out_buffers: (BufferIndex, BufferIndex),
        out_levels: Option<&(A, A)>,
    ) -> Stage<A> {
        let StereoProcessorType::Effect(spec) = self;
        spec.create(factory, in_buffers, out_buffers, out_levels)
    }
}
