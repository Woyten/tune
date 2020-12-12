use std::{
    f64::consts::TAU,
    mem,
    ops::{Add, Mul},
};

use ringbuf::Consumer;
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
            total_time_in_s: 0.0,
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
    pub frequency: LfSource,
    pub modulation: Modulation,
    pub destination: Destination,
}

#[derive(Clone, Deserialize, Serialize)]
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
    ByPhase(Source),
    ByFrequency(Source),
}

impl Oscillator {
    fn create_stage(&self) -> Stage {
        match self.kind {
            OscillatorKind::Sin => self.apply_signal_fn(sin),
            OscillatorKind::Sin3 => self.apply_signal_fn(sin3),
            OscillatorKind::Triangle => self.apply_signal_fn(triangle),
            OscillatorKind::Square => self.apply_signal_fn(square),
            OscillatorKind::Sawtooth => self.apply_signal_fn(sawtooth),
        }
    }

    fn apply_signal_fn(&self, oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static) -> Stage {
        match &self.modulation {
            Modulation::None => self.apply_no_modulation(oscillator_fn, 0.0),
            Modulation::ByPhase(source) => self.apply_variable_phase(oscillator_fn, source.clone()),
            Modulation::ByFrequency(source) => {
                self.apply_variable_frequency(oscillator_fn, source.clone())
            }
        }
    }

    fn apply_no_modulation(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        mut phase: f64,
    ) -> Stage {
        let mut frequency = self.frequency.clone();
        let mut destination = self.destination.clone();

        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            buffers.write_1_read_0(&mut destination, control, || {
                phase = (phase + control.sample_secs * frequency).rem_euclid(1.0);
                oscillator_fn(phase)
            })
        })
    }

    fn apply_variable_phase(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        source: Source,
    ) -> Stage {
        let mut frequency = self.frequency.clone();
        let mut destination = self.destination.clone();

        let mut phase = 0.0;
        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            buffers.write_1_read_1(&mut destination, &source, control, |s| {
                phase = (phase + control.sample_secs * frequency).rem_euclid(1.0);
                oscillator_fn((phase + s).rem_euclid(1.0))
            })
        })
    }

    fn apply_variable_frequency(
        &self,
        mut oscillator_fn: impl FnMut(f64) -> f64 + Send + 'static,
        source: Source,
    ) -> Stage {
        let mut destination = self.destination.clone();
        let mut frequency = self.frequency.clone();

        let mut phase = 0.0;
        Box::new(move |buffers, control| {
            let frequency = frequency.next(control);
            buffers.write_1_read_1(&mut destination, &source, control, |s| {
                phase = (phase + control.sample_secs * (frequency + s)).rem_euclid(1.0);
                oscillator_fn(phase)
            })
        })
    }
}

#[derive(Deserialize, Serialize)]
pub struct Filter {
    #[serde(flatten)]
    pub kind: FilterKind,
    pub source: Source,
    pub destination: Destination,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum FilterKind {
    Copy,
    Pow3,
    Clip {
        limit: LfSource,
    },
    /// Filter as described in https://en.wikipedia.org/wiki/Low-pass_filter#Discrete-time_realization.
    LowPass {
        cutoff: LfSource,
    },
    /// Filter as described in https://en.wikipedia.org/wiki/High-pass_filter#Discrete-time_realization.
    HighPass {
        cutoff: LfSource,
    },
    /// Filter based on the differential equation d2out_dt2 = omega^2*input - out - omega*damping*dout_dt.
    Resonance {
        resonance: LfSource,
        damping: LfSource,
    },
}

impl Filter {
    fn create_stage(&self) -> Stage {
        let source = self.source.clone();
        let mut destination = self.destination.clone();
        match &self.kind {
            FilterKind::Copy => Box::new(move |buffers, control| {
                buffers.write_1_read_1(&mut destination, &source, control, |s| s)
            }),
            FilterKind::Pow3 => Box::new(move |buffers, control| {
                buffers.write_1_read_1(&mut destination, &source, control, |s| s * s * s)
            }),
            FilterKind::Clip { limit } => {
                let mut limit = limit.clone();
                Box::new(move |buffers, control| {
                    let limit = limit.next(control);
                    buffers.write_1_read_1(&mut destination, &source, control, |s| {
                        s.max(-limit).min(limit)
                    })
                })
            }
            FilterKind::LowPass { cutoff } => {
                let mut cutoff = cutoff.clone();

                let mut out = 0.0;
                Box::new(move |buffers, control| {
                    let cutoff = cutoff.next(control);
                    let alpha = 1.0 / (1.0 + (TAU * control.sample_secs * cutoff).recip());
                    buffers.write_1_read_1(&mut destination, &source, control, |input| {
                        out += alpha * (input - out);
                        out
                    });
                })
            }
            FilterKind::HighPass { cutoff } => {
                let mut cutoff = cutoff.clone();

                let mut out = 0.0;
                let mut last_input = 0.0;
                Box::new(move |buffers, control| {
                    let cutoff = cutoff.next(control);
                    let alpha = 1.0 / (1.0 + TAU * control.sample_secs * cutoff);
                    buffers.write_1_read_1(&mut destination, &source, control, |input| {
                        out = alpha * (out + input - last_input);
                        last_input = input;
                        out
                    });
                })
            }
            FilterKind::Resonance { resonance, damping } => {
                let mut resonance = resonance.clone();
                let mut damping = damping.clone();

                let mut out = 0.0;
                let mut dout_dt = 0.0;
                Box::new(move |buffers, control| {
                    let resonance = resonance.next(control);
                    let damping = damping.next(control);
                    // Filter is unstable when d_phase is larger than a quarter period
                    let alpha = (resonance * control.sample_secs).min(0.25);
                    buffers.write_1_read_1(&mut destination, &source, control, |input| {
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
        let mut destination = self.destination.clone();
        Box::new(move |buffers, control| {
            buffers.write_1_read_2(&mut destination, &sources, control, |source_1, source_2| {
                source_1 * source_2
            })
        })
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Destination {
    pub buffer: OutBuffer,
    pub intensity: LfSource,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum OutBuffer {
    Buffer0,
    Buffer1,
    AudioOut,
}

#[derive(Clone, Deserialize, Serialize)]
pub enum Source {
    AudioIn,
    Buffer0,
    Buffer1,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum LfSource {
    Value(f64),
    Expr(LfSourceExpr),
}

#[derive(Clone, Deserialize, Serialize)]
pub enum LfSourceExpr {
    Add(Box<LfSource>, Box<LfSource>),
    Mul(Box<LfSource>, Box<LfSource>),
    Time {
        start: Box<LfSource>,
        end: Box<LfSource>,
        from: Box<LfSource>,
        to: Box<LfSource>,
    },
    Oscillator {
        kind: OscillatorKind,
        phase: f64,
        frequency: Box<LfSource>,
        baseline: Box<LfSource>,
        amplitude: Box<LfSource>,
    },
    Controller {
        controller: Controller,
        from: Box<LfSource>,
        to: Box<LfSource>,
    },
    WaveformPitch,
}

impl From<LfSourceExpr> for LfSource {
    fn from(v: LfSourceExpr) -> Self {
        LfSource::Expr(v)
    }
}

impl LfSource {
    fn next(&mut self, control: &WaveformControl) -> f64 {
        match self {
            LfSource::Value(constant) => *constant,
            LfSource::Expr(LfSourceExpr::Add(a, b)) => a.next(control) + b.next(control),
            LfSource::Expr(LfSourceExpr::Mul(a, b)) => a.next(control) * b.next(control),
            LfSource::Expr(LfSourceExpr::Time {
                start,
                end,
                from,
                to,
            }) => {
                let start = start.next(control);
                let end = end.next(control);
                let from = from.next(control);
                let to = to.next(control);

                let curr_time = control.total_secs;
                if curr_time <= start && curr_time <= end {
                    from
                } else if curr_time >= start && curr_time >= end {
                    to
                } else {
                    from + (to - from) * (control.total_secs - start) / (end - start)
                }
            }
            LfSource::Expr(LfSourceExpr::Oscillator {
                kind,
                phase,
                frequency,
                baseline,
                amplitude,
            }) => {
                let signal = match kind {
                    OscillatorKind::Sin => sin(*phase),
                    OscillatorKind::Sin3 => sin3(*phase),
                    OscillatorKind::Triangle => triangle(*phase),
                    OscillatorKind::Square => square(*phase),
                    OscillatorKind::Sawtooth => sawtooth(*phase),
                };

                *phase = (*phase + frequency.next(control) * control.buffer_secs).rem_euclid(1.0);

                baseline.next(control) + signal * amplitude.next(control)
            }
            LfSource::Expr(LfSourceExpr::Controller {
                controller,
                from,
                to,
            }) => {
                let from = from.next(control);
                let to = to.next(control);
                from + control.controllers.get(*controller) * (to - from)
            }
            LfSource::Expr(LfSourceExpr::WaveformPitch) => control.pitch.as_hz(),
        }
    }
}

fn sin(phase: f64) -> f64 {
    (phase * TAU).sin()
}

fn sin3(phase: f64) -> f64 {
    let sin = sin(phase);
    sin * sin * sin
}

fn triangle(phase: f64) -> f64 {
    ((phase + 0.75) % 1.0 - 0.5).abs() * 4.0 - 1.0
}

fn square(phase: f64) -> f64 {
    (phase - 0.5).signum()
}

fn sawtooth(phase: f64) -> f64 {
    (phase + 0.5).fract() * 2.0 - 1.0
}

impl Add for LfSource {
    type Output = LfSource;

    fn add(self, rhs: Self) -> Self::Output {
        LfSourceExpr::Add(self.into(), rhs.into()).into()
    }
}

impl Mul for LfSource {
    type Output = LfSource;

    fn mul(self, rhs: Self) -> Self::Output {
        LfSourceExpr::Mul(self.into(), rhs.into()).into()
    }
}

pub struct Buffers {
    audio_in_sychronized: bool,
    readable: ReadableBuffers,
    writeable: WaveformBuffer,
    controllers: Controllers,
}

struct ReadableBuffers {
    audio_in: WaveformBuffer,
    buffer0: WaveformBuffer,
    buffer1: WaveformBuffer,
    out: WaveformBuffer,
    total: WaveformBuffer,
    zeros: Vec<f64>,
}

impl Buffers {
    pub fn new() -> Self {
        Self {
            audio_in_sychronized: false,
            readable: ReadableBuffers {
                audio_in: WaveformBuffer::new(),
                buffer0: WaveformBuffer::new(),
                buffer1: WaveformBuffer::new(),
                out: WaveformBuffer::new(),
                total: WaveformBuffer::new(),
                zeros: Vec::new(),
            },
            writeable: WaveformBuffer::new(), // Empty Vec acting as a placeholder
            controllers: Controllers {
                modulation: 0.0,
                breath: 0.0,
                expression: 0.0,
                mouse_y: 0.0,
            },
        }
    }

    pub fn controllers(&mut self) -> &mut Controllers {
        &mut self.controllers
    }

    pub fn clear(&mut self, len: usize) {
        self.readable.audio_in.clear(len);
        self.readable.total.clear(len);
        self.readable.zeros.resize(len, 0.0);
    }

    pub fn set_audio_in(&mut self, audio_source: &mut Consumer<f32>) {
        let audio_in_buffer = &mut self.readable.audio_in;
        if audio_source.len() >= 2 * audio_in_buffer.storage.len() {
            for element in &mut audio_in_buffer.storage {
                let l = f64::from(audio_source.pop().unwrap_or_default());
                let r = f64::from(audio_source.pop().unwrap_or_default());
                *element = l + r / 2.0;
            }
            audio_in_buffer.dirty = false;
            self.audio_in_sychronized = true;
        } else if self.audio_in_sychronized {
            println!("[WARNING] Exchange buffer underrun - Waiting for audio-in to be in sync with audio-out");
        }
    }

    pub fn write(&mut self, waveform: &mut Waveform, sample_width_in_s: f64) {
        let len = self.readable.zeros.len();
        self.readable.buffer0.clear(len);
        self.readable.buffer1.clear(len);
        self.readable.out.clear(len);

        let control = WaveformControl {
            pitch: waveform.pitch,
            sample_secs: sample_width_in_s,
            buffer_secs: sample_width_in_s * len as f64,
            total_secs: waveform.total_time_in_s,
            controllers: self.controllers.clone(),
        };

        for stage in &mut waveform.stages {
            stage(self, &control);
        }

        let out_buffer = self.readable.out.read().unwrap_or(&self.readable.zeros);
        let change_per_sample = waveform.amplitude_change_rate_hz * sample_width_in_s;

        match self.readable.total.write() {
            WriteableBuffer::Dirty(total_buffer) => {
                for (total, out) in total_buffer.iter_mut().zip(&*out_buffer) {
                    waveform.curr_amplitude = (waveform.curr_amplitude + change_per_sample)
                        .max(0.0)
                        .min(1.0);
                    *total = *out * waveform.curr_amplitude
                }
            }
            WriteableBuffer::Clean(total_buffer) => {
                for (total, out) in total_buffer.iter_mut().zip(&*out_buffer) {
                    waveform.curr_amplitude = (waveform.curr_amplitude + change_per_sample)
                        .max(0.0)
                        .min(1.0);
                    *total += *out * waveform.curr_amplitude
                }
            }
        }

        waveform.total_time_in_s += sample_width_in_s * len as f64;
    }

    pub fn total(&mut self) -> &[f64] {
        &self.readable.total.read().unwrap_or(&self.readable.zeros)
    }

    fn write_1_read_0(
        &mut self,
        destination: &mut Destination,
        control: &WaveformControl,
        mut f: impl FnMut() -> f64,
    ) {
        let intensity = destination.intensity.next(control);

        self.write_to_buffer(&destination.buffer, |write, _| match write.write() {
            WriteableBuffer::Dirty(target_buffer) => {
                for target_sample in target_buffer.iter_mut() {
                    *target_sample = f() * intensity
                }
            }
            WriteableBuffer::Clean(target_buffer) => {
                for target_sample in target_buffer.iter_mut() {
                    *target_sample += f() * intensity
                }
            }
        });
    }

    fn write_1_read_1(
        &mut self,
        destination: &mut Destination,
        source: &Source,
        control: &WaveformControl,
        mut f: impl FnMut(f64) -> f64,
    ) {
        let intensity = destination.intensity.next(control);

        self.write_to_buffer(&destination.buffer, |write, read| {
            let source = read.read_from_buffer(source);
            match write.write() {
                WriteableBuffer::Dirty(target_buffer) => {
                    for (target_sample, source_sample) in target_buffer.iter_mut().zip(&*source) {
                        *target_sample = f(*source_sample) * intensity
                    }
                }
                WriteableBuffer::Clean(target_buffer) => {
                    for (target_sample, source_sample) in target_buffer.iter_mut().zip(&*source) {
                        *target_sample += f(*source_sample) * intensity
                    }
                }
            }
        });
    }

    fn write_1_read_2(
        &mut self,
        destination: &mut Destination,
        sources: &(Source, Source),
        control: &WaveformControl,
        mut f: impl FnMut(f64, f64) -> f64,
    ) {
        let intensity = destination.intensity.next(control);

        self.write_to_buffer(&destination.buffer, |target, read| {
            let sources = (
                read.read_from_buffer(&sources.0),
                read.read_from_buffer(&sources.1),
            );
            match target.write() {
                WriteableBuffer::Dirty(target_buffer) => {
                    for (target_sample, source_samples) in target_buffer
                        .iter_mut()
                        .zip(sources.0.iter().zip(&*sources.1))
                    {
                        *target_sample = f(*source_samples.0, *source_samples.1) * intensity
                    }
                }
                WriteableBuffer::Clean(target_buffer) => {
                    for (target_sample, source_samples) in target_buffer
                        .iter_mut()
                        .zip(sources.0.iter().zip(&*sources.1))
                    {
                        *target_sample += f(*source_samples.0, *source_samples.1) * intensity
                    }
                }
            }
        });
    }

    fn write_to_buffer(
        &mut self,
        out_buffer: &OutBuffer,
        mut f: impl FnMut(&mut WaveformBuffer, &ReadableBuffers),
    ) {
        let buffer = match out_buffer {
            OutBuffer::Buffer0 => &mut self.readable.buffer0,
            OutBuffer::Buffer1 => &mut self.readable.buffer1,
            OutBuffer::AudioOut => &mut self.readable.out,
        };
        mem::swap(buffer, &mut self.writeable);
        f(&mut self.writeable, &self.readable);
        let buffer = match out_buffer {
            OutBuffer::Buffer0 => &mut self.readable.buffer0,
            OutBuffer::Buffer1 => &mut self.readable.buffer1,
            OutBuffer::AudioOut => &mut self.readable.out,
        };
        mem::swap(buffer, &mut self.writeable);
        buffer.dirty = false;
    }
}

impl ReadableBuffers {
    fn read_from_buffer(&self, source: &Source) -> &[f64] {
        match source {
            Source::AudioIn => &self.audio_in,
            Source::Buffer0 => &self.buffer0,
            Source::Buffer1 => &self.buffer1,
        }
        .read()
        .unwrap_or(&self.zeros)
    }
}

struct WaveformBuffer {
    storage: Vec<f64>,
    dirty: bool,
}

impl WaveformBuffer {
    fn new() -> Self {
        Self {
            storage: Vec::new(),
            dirty: false,
        }
    }

    fn clear(&mut self, len: usize) {
        self.storage.resize(len, 0.0);
        self.dirty = true;
    }

    fn read(&self) -> Option<&[f64]> {
        match self.dirty {
            true => None,
            false => Some(&self.storage),
        }
    }

    fn write(&mut self) -> WriteableBuffer<'_> {
        match self.dirty {
            true => {
                self.dirty = false;
                WriteableBuffer::Dirty(&mut self.storage)
            }
            false => WriteableBuffer::Clean(&mut self.storage),
        }
    }
}

enum WriteableBuffer<'a> {
    Dirty(&'a mut [f64]),
    Clean(&'a mut [f64]),
}

pub struct Waveform {
    envelope_type: EnvelopeType,
    stages: Vec<Stage>,
    pitch: Pitch,
    total_time_in_s: f64,
    curr_amplitude: f64,
    amplitude_change_rate_hz: f64,
}

type Stage = Box<dyn FnMut(&mut Buffers, &WaveformControl) + Send>;

struct WaveformControl {
    pitch: Pitch,
    sample_secs: f64,
    buffer_secs: f64,
    total_secs: f64,
    controllers: Controllers,
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

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub enum Controller {
    Modulation,
    Breath,
    Expression,
    MouseY,
}

#[derive(Clone)]
pub struct Controllers {
    modulation: f64,
    breath: f64,
    expression: f64,
    mouse_y: f64,
}

impl Controllers {
    pub fn set(&mut self, controller: Controller, value: f64) {
        *match controller {
            Controller::Modulation => &mut self.modulation,
            Controller::Breath => &mut self.breath,
            Controller::Expression => &mut self.expression,
            Controller::MouseY => &mut self.mouse_y,
        } = value;
    }

    fn get(&self, controller: Controller) -> f64 {
        match controller {
            Controller::Modulation => self.modulation,
            Controller::Breath => self.breath,
            Controller::Expression => self.expression,
            Controller::MouseY => self.mouse_y,
        }
    }
}
