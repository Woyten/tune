use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};
use tune::pitch::Pitch;

#[derive(Deserialize, Serialize)]
pub struct WaveformSpec {
    pub name: String,
    pub envelope_type: EnvelopeType,
    pub stages: Vec<StageSpec>,
}

impl WaveformSpec {
    pub fn create_waveform(
        &self,
        pitch: Pitch,
        amplitude: f64,
        envelope_type: Option<EnvelopeType>,
    ) -> Waveform {
        let envelope_type = envelope_type.unwrap_or(self.envelope_type);
        Waveform {
            envelope_type,
            stages: self.stages.iter().map(StageSpec::create_stage).collect(),
            pitch,
            curr_amplitude: amplitude,
            amplitude_change_rate_hz: -amplitude * envelope_type.decay_rate_hz(),
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn envelope_type(&self) -> EnvelopeType {
        self.envelope_type
    }
}

#[derive(Deserialize, Serialize)]
pub enum StageSpec {
    Oscillator(Oscillator),
    Filter(Filter),
    RingModulator(RingModulator),
}

impl StageSpec {
    fn create_stage(&self) -> Stage {
        match self {
            StageSpec::Oscillator(oscillation) => oscillation.create_stage(),
            StageSpec::Filter(filter) => filter.create_stage(),
            StageSpec::RingModulator(ring_modulator) => ring_modulator.create_stage(),
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct Oscillator {
    pub kind: OscillatorKind,
    pub frequency: f64,
    pub modulation: Modulation,
    pub destination: Destination,
}

#[derive(Deserialize, Serialize)]
pub enum OscillatorKind {
    Sin,
    Sin3,
    Triangle,
    Square,
    Sawtooth,
}

#[derive(Deserialize, Serialize)]
pub enum Modulation {
    None,
    ByPhase {
        source: Source,
        normalization: f64,
    },
    ByFrequency {
        source: Source,
        normalization_in_hz: f64,
    },
}

impl Oscillator {
    fn create_stage(&self) -> Stage {
        match self.kind {
            OscillatorKind::Sin => self.apply_signal_fn(|phase| (phase * TAU).sin()),
            OscillatorKind::Sin3 => self.apply_signal_fn(|phase| {
                let sin = (phase * TAU).sin();
                sin * sin * sin
            }),
            OscillatorKind::Triangle => {
                self.apply_signal_fn(|phase| ((phase + 0.75) % 1.0 - 0.5).abs() * 4.0 - 1.0)
            }
            OscillatorKind::Square => self.apply_signal_fn(|phase| (phase - 0.5).signum()),
            OscillatorKind::Sawtooth => {
                self.apply_signal_fn(|phase| (phase + 0.5).fract() * 2.0 - 1.0)
            }
        }
    }

    fn apply_signal_fn(&self, oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static) -> Stage {
        match &self.modulation {
            Modulation::None => self.apply_no_modulation(oscillator_fn, 0.0),
            Modulation::ByPhase {
                source,
                normalization,
            } => self.apply_variable_phase(oscillator_fn, source, *normalization),
            Modulation::ByFrequency {
                source,
                normalization_in_hz,
            } => self.apply_variable_frequency(oscillator_fn, source, *normalization_in_hz),
        }
    }

    fn apply_no_modulation(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        mut phase: f64,
    ) -> Stage {
        let frequency = self.frequency;
        let destination = self.destination.clone();

        Box::new(move |buffers, delta| {
            buffers.write_1_read_0(&destination, || {
                phase = (phase + delta.phase * frequency).rem_euclid(1.0);
                oscillator_fn(phase)
            })
        })
    }

    fn apply_variable_phase(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        source: &Source,
        normalization: f64,
    ) -> Stage {
        let frequency = self.frequency;
        let destination = self.destination.clone();
        let source = source.clone();

        let mut phase = 0.0;
        Box::new(move |buffers, delta| {
            buffers.write_1_read_1(&destination, &source, |s| {
                phase = (phase + delta.phase * frequency).rem_euclid(1.0);
                oscillator_fn((phase + s * normalization).rem_euclid(1.0))
            })
        })
    }

    fn apply_variable_frequency(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        source: &Source,
        normalization_in_hz: f64,
    ) -> Stage {
        let destination = self.destination.clone();
        let frequency = self.frequency;
        let source = source.clone();

        let mut phase = 0.0;
        Box::new(move |buffers, delta| {
            buffers.write_1_read_1(&destination, &source, |s| {
                phase =
                    (phase + delta.phase * frequency + s * delta.time_in_s * normalization_in_hz)
                        .rem_euclid(1.0);
                oscillator_fn(phase)
            })
        })
    }
}

#[derive(Deserialize, Serialize)]
pub struct Filter {
    pub kind: FilterKind,
    pub source: Source,
    pub destination: Destination,
}

#[derive(Deserialize, Serialize)]
pub enum FilterKind {
    Pow3,
    Clip {
        limit: f64,
    },
    /// Filter as described in https://en.wikipedia.org/wiki/Low-pass_filter#Discrete-time_realization.
    LowPass {
        cutoff: LfSource,
    },
    /// Filter based on the differential equation d2out_dt2 = omega^2*input - out - omega*damping*dout_dt.
    Resonance {
        cutoff: LfSource,
        damping: LfSource,
    },
}

impl Filter {
    fn create_stage(&self) -> Stage {
        let source = self.source.clone();
        let target = self.destination.clone();
        match &self.kind {
            FilterKind::Pow3 => {
                Box::new(move |buffers, _| buffers.write_1_read_1(&target, &source, |s| s * s * s))
            }
            FilterKind::Clip { limit } => {
                let limit = *limit;
                Box::new(move |buffers, _| {
                    buffers.write_1_read_1(&target, &source, |s| s.max(-limit).min(limit))
                })
            }
            FilterKind::LowPass { cutoff } => {
                let mut cutoff = cutoff.clone();

                let mut out = 0.0;
                Box::new(move |buffers, delta| {
                    let cutoff = cutoff.next(delta);
                    let alpha = 1.0 / (1.0 + (TAU * delta.phase * cutoff).recip());
                    buffers.write_1_read_1(&target, &source, |input| {
                        out += alpha * (input - out);
                        out
                    });
                })
            }
            FilterKind::Resonance { cutoff, damping } => {
                let mut cutoff = cutoff.clone();
                let mut damping = damping.clone();

                let mut out = 0.0;
                let mut dout_dt = 0.0;
                Box::new(move |buffers, delta| {
                    // Filter is unstable when d_phase is larger than a quarter period
                    let cutoff = cutoff.next(delta);
                    let damping = damping.next(delta);
                    let alpha = (cutoff * delta.phase).min(0.25);
                    buffers.write_1_read_1(&target, &source, |input| {
                        let d2out_dt2 = input - out - damping * dout_dt;
                        dout_dt += d2out_dt2 * TAU * alpha;
                        out += dout_dt * TAU * alpha;
                        out
                    });
                })
            }
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct RingModulator {
    pub sources: (Source, Source),
    pub destination: Destination,
}

impl RingModulator {
    fn create_stage(&self) -> Stage {
        let sources = self.sources.clone();
        let destination = self.destination.clone();
        Box::new(move |buffers, _| {
            buffers.write_1_read_2(&destination, &sources, |source_1, source_2| {
                source_1 * source_2
            })
        })
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Destination {
    pub buffer: OutBuffer,
    pub intensity: f64,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum OutBuffer {
    Buffer0,
    Buffer1,
    AudioOut,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum Source {
    Constant(f64),
    Buffer0,
    Buffer1,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum LfSource {
    Value(f64),
    Slope {
        from: f64,
        to: f64,
        change_per_s: f64,
    },
}

impl LfSource {
    fn next(&mut self, delta: &DeltaTime) -> f64 {
        match self {
            LfSource::Value(constant) => *constant,
            LfSource::Slope {
                from,
                to,
                change_per_s,
            } => {
                if from < to {
                    *from = (*from + delta.buffer_time_in_s * *change_per_s).min(*to);
                } else {
                    *from = (*from - delta.buffer_time_in_s * *change_per_s).max(*to);
                }
                *from
            }
        }
    }
}

pub struct Buffers {
    // in: Vec<u8>
    buffer0: Vec<f64>,
    buffer1: Vec<f64>,
    out: Vec<f64>,
    total: Vec<f64>,
}

impl Buffers {
    pub fn new() -> Self {
        Self {
            buffer0: vec![],
            buffer1: vec![],
            out: vec![],
            total: vec![],
        }
    }

    pub fn clear(&mut self, len: usize) {
        self.total.clear();
        self.total.resize(len, 0.0);
    }

    pub fn total(&mut self) -> &[f64] {
        &self.total
    }

    fn write_1_read_0(&mut self, destination: &Destination, mut f: impl FnMut() -> f64) {
        let target_buffer = match destination.buffer {
            OutBuffer::Buffer0 => &mut self.buffer0,
            OutBuffer::Buffer1 => &mut self.buffer1,
            OutBuffer::AudioOut => &mut self.out,
        };

        for target_sample in target_buffer.iter_mut() {
            *target_sample += f() * destination.intensity
        }
    }

    fn write_1_read_1(
        &mut self,
        destination: &Destination,
        source: &Source,
        mut f: impl FnMut(f64) -> f64,
    ) {
        let (target_buffer, source_buffer) = match (&destination.buffer, source) {
            (OutBuffer::Buffer0, Source::Buffer1) => (&mut self.buffer0, &self.buffer1),
            (OutBuffer::Buffer1, Source::Buffer0) => (&mut self.buffer1, &self.buffer0),
            (OutBuffer::AudioOut, Source::Buffer0) => (&mut self.out, &self.buffer0),
            (OutBuffer::AudioOut, Source::Buffer1) => (&mut self.out, &self.buffer1),
            _ => unimplemented!(
                "This combination of target and destination buffers is not supported yet"
            ),
        };

        for (target_sample, source_sample) in target_buffer.iter_mut().zip(source_buffer.iter()) {
            *target_sample += f(*source_sample) * destination.intensity
        }
    }

    fn write_1_read_2(
        &mut self,
        destination: &Destination,
        sources: &(Source, Source),
        mut f: impl FnMut(f64, f64) -> f64,
    ) {
        let (target_buffer, source_buffers) = match (&destination.buffer, sources) {
            (OutBuffer::AudioOut, (Source::Buffer0, Source::Buffer1)) => {
                (&mut self.out, (&self.buffer0, &self.buffer1))
            }
            (OutBuffer::AudioOut, (Source::Buffer1, Source::Buffer0)) => {
                (&mut self.out, (&self.buffer1, &self.buffer0))
            }
            _ => unimplemented!(
                "This combination of target and destination buffers is not supported yet"
            ),
        };

        for (target_sample, source_samples) in target_buffer
            .iter_mut()
            .zip(source_buffers.0.iter().zip(source_buffers.1.iter()))
        {
            *target_sample += f(*source_samples.0, *source_samples.1) * destination.intensity
        }
    }
}

pub struct Waveform {
    envelope_type: EnvelopeType,
    stages: Vec<Stage>,
    pitch: Pitch,
    curr_amplitude: f64,
    amplitude_change_rate_hz: f64,
}

type Stage = Box<dyn FnMut(&mut Buffers, &DeltaTime) + Send>;

struct DeltaTime {
    phase: f64,
    time_in_s: f64,
    buffer_time_in_s: f64,
}

impl Waveform {
    pub fn pitch(&self) -> Pitch {
        self.pitch
    }

    pub fn set_pitch(&mut self, pitch: Pitch) {
        self.pitch = pitch;
    }

    pub fn set_fade(&mut self, decay_amount: f64) {
        let interpolation = (1.0 - decay_amount) * self.envelope_type.release_rate_hz()
            + decay_amount * self.envelope_type.decay_rate_hz();
        self.amplitude_change_rate_hz = -self.curr_amplitude * interpolation;
    }

    pub fn amplitude(&self) -> f64 {
        self.curr_amplitude
    }

    pub fn write(&mut self, buffers: &mut Buffers, sample_width_in_s: f64) {
        let len = buffers.total.len();
        buffers.buffer0.clear();
        buffers.buffer0.resize(len, 0.0);
        buffers.buffer1.clear();
        buffers.buffer1.resize(len, 0.0);
        buffers.out.clear();
        buffers.out.resize(len, 0.0);

        let phase = sample_width_in_s * self.pitch.as_hz();
        let delta = DeltaTime {
            phase,
            time_in_s: sample_width_in_s,
            buffer_time_in_s: sample_width_in_s * buffers.out.len() as f64,
        };
        for stage in &mut self.stages {
            stage(buffers, &delta);
        }

        let change_per_sample = self.amplitude_change_rate_hz * sample_width_in_s;
        for (total, out) in buffers.total.iter_mut().zip(buffers.out.iter()) {
            self.curr_amplitude = (self.curr_amplitude + change_per_sample).max(0.0).min(1.0);
            *total += *out * self.curr_amplitude
        }
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub enum EnvelopeType {
    Organ,
    Piano,
    Pad,
    Bell,
}

impl EnvelopeType {
    fn decay_rate_hz(&self) -> f64 {
        match self {
            EnvelopeType::Organ => 0.0,
            EnvelopeType::Piano => 0.2,
            EnvelopeType::Pad => 0.0,
            EnvelopeType::Bell => 0.33,
        }
    }

    fn release_rate_hz(&self) -> f64 {
        match self {
            EnvelopeType::Organ => 100.0,
            EnvelopeType::Piano => 10.0,
            EnvelopeType::Pad => 0.5,
            EnvelopeType::Bell => 0.33,
        }
    }
}
