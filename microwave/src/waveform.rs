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
                let signal = oscillator_fn(phase);
                phase = (phase + control.sample_secs * frequency).rem_euclid(1.0);
                signal
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
                let signal = oscillator_fn((phase + s).rem_euclid(1.0));
                phase = (phase + control.sample_secs * frequency).rem_euclid(1.0);
                signal
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
                let signal = oscillator_fn(phase);
                phase = (phase + control.sample_secs * (frequency + s)).rem_euclid(1.0);
                signal
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
    /// LPF implementation as described in http://shepazu.github.io/Audio-EQ-Cookbook/audio-eq-cookbook.html.
    LowPass2 {
        resonance: LfSource,
        quality: LfSource,
    },
    /// Filter as described in https://en.wikipedia.org/wiki/High-pass_filter#Discrete-time_realization.
    HighPass {
        cutoff: LfSource,
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
                    let omega_0 = TAU * cutoff * control.sample_secs;
                    let alpha = (1.0 + omega_0.recip()).recip();
                    buffers.write_1_read_1(&mut destination, &source, control, |input| {
                        out += alpha * (input - out);
                        out
                    });
                })
            }
            FilterKind::LowPass2 { resonance, quality } => {
                let mut resonance = resonance.clone();
                let mut quality = quality.clone();

                let (mut y1, mut y2, mut x1, mut x2) = Default::default();
                Box::new(move |buffers, control| {
                    let resonance = resonance.next(control);
                    let quality = quality.next(control).max(1e-10);

                    // Restrict f0 for stability
                    let f0 = (resonance * control.sample_secs).max(0.0).min(0.25);
                    let (sin, cos) = (TAU * f0).sin_cos();
                    let alpha = sin / 2.0 / quality;

                    let b1 = 1.0 - cos;
                    let b0 = b1 / 2.0;
                    let b2 = b0;
                    let a0 = 1.0 + alpha;
                    let a1 = -2.0 * cos;
                    let a2 = 1.0 - alpha;

                    buffers.write_1_read_1(&mut destination, &source, control, |x0| {
                        let y0 = (b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2) / a0;
                        x2 = x1;
                        x1 = x0;
                        y2 = y1;
                        y1 = y0;
                        y0
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
    (((0.75 + phase).fract() - 0.5).abs() - 0.25) * 4.0
}

fn square(phase: f64) -> f64 {
    (0.5 - phase).signum()
}

fn sawtooth(phase: f64) -> f64 {
    ((0.5 + phase).fract() - 0.5) * 2.0
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

    pub fn total(&self) -> &[f64] {
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

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use super::*;

    const NUM_SAMPLES: usize = 44100;
    const SAMPLE_SECS: f64 = 1.0 / 44100.0;

    #[test]
    fn clear_and_resize_buffers() {
        let mut buffers = Buffers::new();
        assert_eq!(buffers.total(), &[0f64; 0]);

        buffers.clear(128);
        assert_eq!(buffers.total(), &[0f64; 128]);

        buffers.clear(256);
        assert_eq!(buffers.total(), &[0f64; 256]);

        buffers.clear(64);
        assert_eq!(buffers.total(), &[0f64; 64]);
    }

    #[test]
    fn empty_spec() {
        let mut buffers = Buffers::new();
        let mut waveform = spec(vec![]).create_waveform(Pitch::from_hz(440.0), 1.0, None);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.total(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, SAMPLE_SECS);
        assert_eq!(buffers.total(), &[0f64; NUM_SAMPLES]);
    }

    #[test]
    fn write_waveform_and_clear() {
        let mut buffers = Buffers::new();
        let mut waveform = spec(vec![StageSpec::Oscillator(Oscillator {
            kind: OscillatorKind::Sin,
            frequency: LfSourceExpr::WaveformPitch.into(),
            modulation: Modulation::None,
            destination: Destination {
                buffer: OutBuffer::AudioOut,
                intensity: LfSource::Value(1.0),
            },
        })])
        .create_waveform(Pitch::from_hz(440.0), 1.0, None);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.total(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, SAMPLE_SECS);
        assert_buffer_total_is(&buffers, |t| (TAU * 440.0 * t).sin());

        buffers.clear(128);
        assert_eq!(buffers.total(), &[0f64; 128]);
    }

    #[test]
    fn mix_two_wavforms() {
        let mut buffers = Buffers::new();

        let spec = spec(vec![StageSpec::Oscillator(Oscillator {
            kind: OscillatorKind::Sin,
            frequency: LfSourceExpr::WaveformPitch.into(),
            modulation: Modulation::None,
            destination: Destination {
                buffer: OutBuffer::AudioOut,
                intensity: LfSource::Value(1.0),
            },
        })]);

        let mut waveform1 = spec.create_waveform(Pitch::from_hz(440.0), 0.7, None);
        let mut waveform2 = spec.create_waveform(Pitch::from_hz(660.0), 0.8, None);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.total(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform1, SAMPLE_SECS);
        assert_buffer_total_is(&buffers, |t| 0.7 * (440.0 * TAU * t).sin());

        buffers.write(&mut waveform2, SAMPLE_SECS);
        assert_buffer_total_is(&buffers, |t| {
            0.7 * (440.0 * TAU * t).sin() + 0.8 * (660.0 * TAU * t).sin()
        });
    }

    #[test]
    fn modulate_by_frequency() {
        let mut buffers = Buffers::new();

        let mut waveform = spec(vec![
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSource::Value(330.0),
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::Buffer0,
                    intensity: LfSource::Value(440.0),
                },
            }),
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSourceExpr::WaveformPitch.into(),
                modulation: Modulation::ByFrequency(Source::Buffer0),
                destination: Destination {
                    buffer: OutBuffer::AudioOut,
                    intensity: LfSource::Value(1.0),
                },
            }),
        ])
        .create_waveform(Pitch::from_hz(550.0), 1.0, None);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.total(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, SAMPLE_SECS);
        assert_buffer_total_is(&buffers, {
            let mut mod_phase = 0.0;
            move |t| {
                let signal = ((550.0 * t + mod_phase) * TAU).sin();
                mod_phase += (330.0 * TAU * t).sin() * 440.0 * SAMPLE_SECS;
                signal
            }
        });
    }

    #[test]
    fn modulate_by_phase() {
        let mut buffers = Buffers::new();
        let mut waveform = spec(vec![
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSource::Value(330.0),
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::Buffer0,
                    intensity: LfSource::Value(0.44),
                },
            }),
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSourceExpr::WaveformPitch.into(),
                modulation: Modulation::ByPhase(Source::Buffer0),
                destination: Destination {
                    buffer: OutBuffer::AudioOut,
                    intensity: LfSource::Value(1.0),
                },
            }),
        ])
        .create_waveform(Pitch::from_hz(550.0), 1.0, None);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.total(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, SAMPLE_SECS);
        assert_buffer_total_is(&buffers, |t| {
            ((550.0 * t + (330.0 * TAU * t).sin() * 0.44) * TAU).sin()
        });
    }

    #[test]
    fn ring_modulation() {
        let mut buffers = Buffers::new();
        let mut waveform = spec(vec![
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSourceExpr::WaveformPitch.into(),
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::Buffer0,
                    intensity: LfSource::Value(1.0),
                },
            }),
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSource::Value(1.5) * LfSourceExpr::WaveformPitch.into(),

                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::Buffer1,
                    intensity: LfSource::Value(1.0),
                },
            }),
            StageSpec::RingModulator(RingModulator {
                sources: (Source::Buffer0, Source::Buffer1),
                destination: Destination {
                    buffer: OutBuffer::AudioOut,
                    intensity: LfSource::Value(1.0),
                },
            }),
        ])
        .create_waveform(Pitch::from_hz(440.0), 1.0, None);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.total(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, SAMPLE_SECS);
        assert_buffer_total_is(&buffers, |t| {
            (440.0 * t * TAU).sin() * (660.0 * t * TAU).sin()
        });
    }

    #[test]
    fn waveform_correctness() {
        let eps = 1e-10;

        assert_approx_eq!(sin(0.0 / 8.0), 0.0);
        assert_approx_eq!(sin(1.0 / 8.0), (1.0f64 / 2.0).sqrt());
        assert_approx_eq!(sin(2.0 / 8.0), 1.0);
        assert_approx_eq!(sin(3.0 / 8.0), (1.0f64 / 2.0).sqrt());
        assert_approx_eq!(sin(4.0 / 8.0), 0.0);
        assert_approx_eq!(sin(5.0 / 8.0), -(1.0f64 / 2.0).sqrt());
        assert_approx_eq!(sin(6.0 / 8.0), -1.0);
        assert_approx_eq!(sin(7.0 / 8.0), -(1.0f64 / 2.0).sqrt());

        assert_approx_eq!(sin3(0.0 / 8.0), 0.0);
        assert_approx_eq!(sin3(1.0 / 8.0), (1.0f64 / 8.0).sqrt());
        assert_approx_eq!(sin3(2.0 / 8.0), 1.0);
        assert_approx_eq!(sin3(3.0 / 8.0), (1.0f64 / 8.0).sqrt());
        assert_approx_eq!(sin3(4.0 / 8.0), 0.0);
        assert_approx_eq!(sin3(5.0 / 8.0), -(1.0f64 / 8.0).sqrt());
        assert_approx_eq!(sin3(6.0 / 8.0), -1.0);
        assert_approx_eq!(sin3(7.0 / 8.0), -(1.0f64 / 8.0).sqrt());

        assert_approx_eq!(triangle(0.0 / 8.0), 0.0);
        assert_approx_eq!(triangle(1.0 / 8.0), 0.5);
        assert_approx_eq!(triangle(2.0 / 8.0), 1.0);
        assert_approx_eq!(triangle(3.0 / 8.0), 0.5);
        assert_approx_eq!(triangle(4.0 / 8.0), 0.0);
        assert_approx_eq!(triangle(5.0 / 8.0), -0.5);
        assert_approx_eq!(triangle(6.0 / 8.0), -1.0);
        assert_approx_eq!(triangle(7.0 / 8.0), -0.5);

        assert_approx_eq!(square(0.0 / 8.0 + eps), 1.0);
        assert_approx_eq!(square(1.0 / 8.0), 1.0);
        assert_approx_eq!(square(2.0 / 8.0), 1.0);
        assert_approx_eq!(square(3.0 / 8.0), 1.0);
        assert_approx_eq!(square(4.0 / 8.0 - eps), 1.0);
        assert_approx_eq!(square(4.0 / 8.0 + eps), -1.0);
        assert_approx_eq!(square(5.0 / 8.0), -1.0);
        assert_approx_eq!(square(6.0 / 8.0), -1.0);
        assert_approx_eq!(square(7.0 / 8.0), -1.0);
        assert_approx_eq!(square(8.0 / 8.0 - eps), -1.0);

        assert_approx_eq!(sawtooth(0.0 / 8.0), 0.0);
        assert_approx_eq!(sawtooth(1.0 / 8.0), 0.25);
        assert_approx_eq!(sawtooth(2.0 / 8.0), 0.5);
        assert_approx_eq!(sawtooth(3.0 / 8.0), 0.75);
        assert_approx_eq!(sawtooth(4.0 / 8.0 - eps), 1.0);
        assert_approx_eq!(sawtooth(4.0 / 8.0 + eps), -1.0);
        assert_approx_eq!(sawtooth(5.0 / 8.0), -0.75);
        assert_approx_eq!(sawtooth(6.0 / 8.0), -0.5);
        assert_approx_eq!(sawtooth(7.0 / 8.0), -0.25);
    }

    fn spec(stages: Vec<StageSpec>) -> WaveformSpec {
        WaveformSpec {
            name: String::new(),
            envelope_type: EnvelopeType::Organ,
            stages,
        }
    }

    fn assert_buffer_total_is(buffers: &Buffers, mut f: impl FnMut(f64) -> f64) {
        let mut time = 0.0;
        for sample in buffers.total() {
            assert_approx_eq!(sample, f(time));
            time += SAMPLE_SECS;
        }
    }
}
