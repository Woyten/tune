use std::{iter, marker::PhantomData, mem};

use waveform::WaveformState;

use self::{
    control::Controller,
    waveform::{AutomationSpec, OutSpec, Waveform},
};

mod functions;
mod util;

pub mod control;
pub mod effects;
pub mod envelope;
pub mod filter;
pub mod oscillator;
pub mod signal;
pub mod source;
pub mod spec;
pub mod waveform;
pub mod waveguide;

pub struct Magnetron {
    buffers: BufferWriter,
}

impl Magnetron {
    pub fn new(sample_width_secs: f64, num_buffers: usize, buffer_size: usize) -> Self {
        Self {
            buffers: BufferWriter {
                sample_width_secs,
                readable: ReadableBuffers {
                    audio_in: WaveformBuffer::new(buffer_size),
                    intermediate: vec![WaveformBuffer::new(buffer_size); num_buffers],
                    audio_out: WaveformBuffer::new(buffer_size),
                    total: WaveformBuffer::new(buffer_size),
                    zeros: vec![0.0; buffer_size],
                },
                writeable: WaveformBuffer::new(0), // Empty Vec acting as a placeholder
            },
        }
    }

    pub fn clear(&mut self, len: usize) {
        self.buffers.readable.audio_in.clear(len);
        self.buffers.readable.total.clear(len);
    }

    pub fn set_audio_in(&mut self, mut buffer_content: impl FnMut() -> f64) {
        self.buffers
            .readable
            .audio_in
            .write(iter::from_fn(|| Some(buffer_content())));
    }

    pub fn write<A: AutomationSpec>(
        &mut self,
        waveform: &mut Waveform<A>,
        storage: &A::Storage,
        note_suspension: f64,
    ) -> bool {
        let buffers = &mut self.buffers;

        let len = buffers.readable.total.len;
        for buffer in &mut buffers.readable.intermediate {
            buffer.clear(len);
        }
        buffers.readable.audio_out.clear(len);

        let state = &mut waveform.state;

        let render_window_secs = buffers.sample_width_secs * len as f64;
        let context = AutomationContext {
            render_window_secs,
            state,
            storage,
        };

        for stage in &mut waveform.stages {
            stage.render(buffers, &context);
        }

        let out_buffer = buffers.readable.audio_out.read(&buffers.readable.zeros);

        let from_amplitude = waveform
            .envelope
            .get_value(state.secs_since_pressed, state.secs_since_released);

        state.secs_since_pressed += render_window_secs;
        state.secs_since_released += render_window_secs * (1.0 - note_suspension);

        let to_amplitude = waveform
            .envelope
            .get_value(state.secs_since_pressed, state.secs_since_released);

        let mut curr_amplitude = from_amplitude;
        let slope = (to_amplitude - from_amplitude) / len as f64;

        buffers.readable.total.write(out_buffer.iter().map(|src| {
            let result = src * curr_amplitude * state.velocity;
            curr_amplitude = (curr_amplitude + slope).clamp(0.0, 1.0);
            result
        }));

        waveform.envelope.is_active(state.secs_since_released)
    }

    pub fn total(&self) -> &[f64] {
        self.buffers
            .readable
            .total
            .read(&self.buffers.readable.zeros)
    }
}

pub struct BufferWriter {
    sample_width_secs: f64,
    readable: ReadableBuffers,
    writeable: WaveformBuffer,
}

impl BufferWriter {
    pub fn sample_width_secs(&self) -> f64 {
        self.sample_width_secs
    }

    pub fn read_0_and_write(
        &mut self,
        out_buffer: OutBuffer,
        out_level: f64,
        mut f: impl FnMut() -> f64,
    ) {
        self.read_n_and_write(out_buffer, |_, write_access| {
            write_access.write(iter::repeat_with(|| f() * out_level))
        });
    }

    pub fn read_1_and_write(
        &mut self,
        in_buffer: InBuffer,
        out_buffer: OutBuffer,
        out_level: f64,
        mut f: impl FnMut(f64) -> f64,
    ) {
        self.read_n_and_write(out_buffer, |read_access, write_access| {
            write_access.write(
                read_access
                    .read(in_buffer)
                    .iter()
                    .map(|&src| f(src) * out_level),
            )
        });
    }

    pub fn read_2_and_write(
        &mut self,
        in_buffers: (InBuffer, InBuffer),
        out_buffer: OutBuffer,
        out_level: f64,
        mut f: impl FnMut(f64, f64) -> f64,
    ) {
        self.read_n_and_write(out_buffer, |read_access, write_access| {
            write_access.write(
                read_access
                    .read(in_buffers.0)
                    .iter()
                    .zip(read_access.read(in_buffers.1))
                    .map(|(&src_0, &src_1)| f(src_0, src_1) * out_level),
            )
        });
    }

    fn read_n_and_write(
        &mut self,
        out_buffer: OutBuffer,
        mut rw_access_fn: impl FnMut(&ReadableBuffers, &mut WaveformBuffer),
    ) {
        self.readable.swap(out_buffer, &mut self.writeable);
        rw_access_fn(&self.readable, &mut self.writeable);
        self.readable.swap(out_buffer, &mut self.writeable);
    }
}

#[derive(Copy, Clone, Debug)]
pub enum InBuffer {
    Buffer(usize),
    AudioIn,
}

#[derive(Copy, Clone, Debug)]
pub enum OutBuffer {
    Buffer(usize),
    AudioOut,
}

struct ReadableBuffers {
    audio_in: WaveformBuffer,
    intermediate: Vec<WaveformBuffer>,
    audio_out: WaveformBuffer,
    total: WaveformBuffer,
    zeros: Vec<f64>,
}

impl ReadableBuffers {
    fn swap(&mut self, buffer_a: OutBuffer, buffer_b: &mut WaveformBuffer) {
        let buffer_a = match buffer_a {
            OutBuffer::Buffer(index) => self.intermediate.get_mut(index).unwrap_or_else(|| {
                panic!(
                    "Index {} out of range. Please allocate more waveform buffers.",
                    index
                )
            }),
            OutBuffer::AudioOut => &mut self.audio_out,
        };
        mem::swap(buffer_a, buffer_b);
    }

    fn read(&self, in_buffer: InBuffer) -> &[f64] {
        match in_buffer {
            InBuffer::Buffer(index) => &self.intermediate[index],
            InBuffer::AudioIn => &self.audio_in,
        }
        .read(&self.zeros)
    }
}

#[derive(Clone)]
struct WaveformBuffer {
    storage: Vec<f64>,
    len: usize,
    dirty: bool,
}

impl WaveformBuffer {
    fn new(buffer_size: usize) -> Self {
        Self {
            storage: vec![0.0; buffer_size],
            len: 0,
            dirty: false,
        }
    }

    fn clear(&mut self, len: usize) {
        self.len = len;
        self.dirty = true;
    }

    fn read<'a>(&'a self, if_empty: &'a [f64]) -> &'a [f64] {
        match self.dirty {
            true => &if_empty[..self.len],
            false => &self.storage[..self.len],
        }
    }

    fn write(&mut self, items: impl Iterator<Item = f64>) {
        match self.dirty {
            true => {
                for (dest, src) in self.storage[..self.len].iter_mut().zip(items) {
                    *dest = src
                }
                self.dirty = false;
            }
            false => {
                for (dest, src) in self.storage[..self.len].iter_mut().zip(items) {
                    *dest += src
                }
            }
        }
    }
}

pub struct AutomationContext<'a, S> {
    pub render_window_secs: f64,
    pub state: &'a WaveformState,
    pub storage: &'a S,
}

impl<'a, S> AutomationContext<'a, S> {
    pub fn read<V: AutomatedValue<Storage = S>>(&self, value: &mut V) -> V::Value {
        value.use_context(self)
    }
}

pub trait AutomatedValue {
    type Storage;
    type Value;

    fn use_context(&mut self, context: &AutomationContext<Self::Storage>) -> Self::Value;
}

impl<C: Controller> AutomatedValue for PhantomData<C> {
    type Storage = C::Storage;
    type Value = ();

    fn use_context(&mut self, _context: &AutomationContext<Self::Storage>) -> Self::Value {}
}

impl<A1: AutomatedValue, A2: AutomatedValue<Storage = A1::Storage>> AutomatedValue for (A1, A2) {
    type Storage = A1::Storage;
    type Value = (A1::Value, A2::Value);

    fn use_context(&mut self, context: &AutomationContext<Self::Storage>) -> Self::Value {
        (context.read(&mut self.0), context.read(&mut self.1))
    }
}

impl<
        A1: AutomatedValue,
        A2: AutomatedValue<Storage = A1::Storage>,
        A3: AutomatedValue<Storage = A1::Storage>,
    > AutomatedValue for (A1, A2, A3)
{
    type Storage = A1::Storage;
    type Value = (A1::Value, A2::Value, A3::Value);

    fn use_context(&mut self, context: &AutomationContext<Self::Storage>) -> Self::Value {
        (
            context.read(&mut self.0),
            context.read(&mut self.1),
            context.read(&mut self.2),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::{collections::HashMap, f64::consts::TAU};

    use assert_approx_eq::assert_approx_eq;
    use tune::pitch::Pitch;

    use crate::{
        magnetron::waveform::{InBufferSpec, OutBufferSpec},
        synth::LiveParameter,
    };

    use super::{
        control::NoControl,
        filter::RingModulator,
        oscillator::{Modulation, Oscillator, OscillatorKind},
        source::{LfSource, LfSourceUnit},
        spec::{EnvelopeSpec, StageSpec, WaveformSpec},
        waveform::{Creator, OutSpec},
        *,
    };

    #[test]
    fn deserialize_stage() {
        let yml = r"
Filter:
  kind: LowPass2
  resonance:
    Controller:
      kind: Modulation
      from: 0.0
      to: 10000.0
  quality: 5.0
  in_buffer: 0
  out_buffer: AudioOut
  out_level: 1.0";
        serde_yaml::from_str::<StageSpec<LfSource<LiveParameter>>>(yml).unwrap();
    }

    const NUM_SAMPLES: usize = 44100;
    const SAMPLE_WIDTH_SECS: f64 = 1.0 / 44100.0;

    #[test]
    fn clear_and_resize_buffers() {
        let mut buffers = magnetron();

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
        let mut buffers = magnetron();
        let mut waveform = create_waveform(&spec(vec![]), Pitch::from_hz(440.0), 1.0);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.total(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &(), 1.0);
        assert_eq!(buffers.total(), &[0f64; NUM_SAMPLES]);
    }

    #[test]
    fn write_waveform_and_clear() {
        let mut buffers = magnetron();
        let mut waveform = create_waveform(
            &spec(vec![StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSourceUnit::WaveformPitch.wrap(),
                modulation: Modulation::None,
                out_spec: OutSpec {
                    out_buffer: OutBufferSpec::audio_out(),
                    out_level: LfSource::Value(1.0),
                },
            })]),
            Pitch::from_hz(440.0),
            1.0,
        );

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.total(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &(), 1.0);
        assert_buffer_total_is(&buffers, |t| (TAU * 440.0 * t).sin());

        buffers.clear(128);
        assert_eq!(buffers.total(), &[0f64; 128]);
    }

    #[test]
    fn mix_two_waveforms() {
        let mut buffers = magnetron();

        let spec = spec(vec![StageSpec::Oscillator(Oscillator {
            kind: OscillatorKind::Sin,
            frequency: LfSourceUnit::WaveformPitch.wrap(),
            modulation: Modulation::None,
            out_spec: OutSpec {
                out_buffer: OutBufferSpec::audio_out(),
                out_level: LfSource::Value(1.0),
            },
        })]);

        let mut waveform1 = create_waveform(&spec, Pitch::from_hz(440.0), 0.7);
        let mut waveform2 = create_waveform(&spec, Pitch::from_hz(660.0), 0.8);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.total(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform1, &(), 1.0);
        assert_buffer_total_is(&buffers, |t| 0.7 * (440.0 * TAU * t).sin());

        buffers.write(&mut waveform2, &(), 1.0);
        assert_buffer_total_is(&buffers, |t| {
            0.7 * (440.0 * TAU * t).sin() + 0.8 * (660.0 * TAU * t).sin()
        });
    }

    #[test]
    fn modulate_by_frequency() {
        let mut buffers = magnetron();

        let spec = spec(vec![
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSource::Value(330.0),
                modulation: Modulation::None,
                out_spec: OutSpec {
                    out_buffer: OutBufferSpec::Buffer(0),
                    out_level: LfSource::Value(440.0),
                },
            }),
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSourceUnit::WaveformPitch.wrap(),
                modulation: Modulation::ByFrequency {
                    mod_buffer: InBufferSpec::Buffer(0),
                },
                out_spec: OutSpec {
                    out_buffer: OutBufferSpec::audio_out(),
                    out_level: LfSource::Value(1.0),
                },
            }),
        ]);

        let mut waveform = create_waveform(&spec, Pitch::from_hz(550.0), 1.0);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.total(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &(), 1.0);
        assert_buffer_total_is(&buffers, {
            let mut mod_phase = 0.0;
            move |t| {
                let signal = ((550.0 * t + mod_phase) * TAU).sin();
                mod_phase += (330.0 * TAU * t).sin() * 440.0 * SAMPLE_WIDTH_SECS;
                signal
            }
        });
    }

    #[test]
    fn modulate_by_phase() {
        let mut buffers = magnetron();

        let spec = spec(vec![
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSource::Value(330.0),
                modulation: Modulation::None,
                out_spec: OutSpec {
                    out_buffer: OutBufferSpec::Buffer(0),
                    out_level: LfSource::Value(0.44),
                },
            }),
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSourceUnit::WaveformPitch.wrap(),
                modulation: Modulation::ByPhase {
                    mod_buffer: InBufferSpec::Buffer(0),
                },
                out_spec: OutSpec {
                    out_buffer: OutBufferSpec::audio_out(),
                    out_level: LfSource::Value(1.0),
                },
            }),
        ]);

        let mut waveform = create_waveform(&spec, Pitch::from_hz(550.0), 1.0);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.total(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &(), 1.0);
        assert_buffer_total_is(&buffers, |t| {
            ((550.0 * t + (330.0 * TAU * t).sin() * 0.44) * TAU).sin()
        });
    }

    #[test]
    fn ring_modulation() {
        let mut buffers = magnetron();

        let spec = spec(vec![
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSourceUnit::WaveformPitch.wrap(),
                modulation: Modulation::None,
                out_spec: OutSpec {
                    out_buffer: OutBufferSpec::Buffer(0),
                    out_level: LfSource::Value(1.0),
                },
            }),
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSource::Value(1.5) * LfSourceUnit::WaveformPitch.wrap(),

                modulation: Modulation::None,
                out_spec: OutSpec {
                    out_buffer: OutBufferSpec::Buffer(1),
                    out_level: LfSource::Value(1.0),
                },
            }),
            StageSpec::RingModulator(RingModulator {
                in_buffers: (InBufferSpec::Buffer(0), InBufferSpec::Buffer(1)),
                out_spec: OutSpec {
                    out_buffer: OutBufferSpec::audio_out(),
                    out_level: LfSource::Value(1.0),
                },
            }),
        ]);

        let mut waveform = create_waveform(&spec, Pitch::from_hz(440.0), 1.0);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.total(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &(), 1.0);
        assert_buffer_total_is(&buffers, |t| {
            (440.0 * t * TAU).sin() * (660.0 * t * TAU).sin()
        });
    }

    fn magnetron() -> Magnetron {
        Magnetron::new(SAMPLE_WIDTH_SECS, 2, 100000)
    }

    fn spec(stages: Vec<StageSpec<LfSource<NoControl>>>) -> WaveformSpec<LfSource<NoControl>> {
        WaveformSpec {
            name: String::new(),
            envelope: "Organ".to_owned(),
            stages,
        }
    }

    fn create_waveform(
        spec: &WaveformSpec<LfSource<NoControl>>,
        pitch: Pitch,
        velocity: f64,
    ) -> Waveform<LfSource<NoControl>> {
        let mut envelope_map = HashMap::new();
        envelope_map.insert(
            spec.envelope.to_owned(),
            EnvelopeSpec {
                name: spec.envelope.to_owned(),
                attack_time: -1e-10,
                release_time: 1e-10,
                decay_rate: 0.0,
            },
        );
        Creator::new(envelope_map)
            .create(spec.with_pitch_and_velocity(pitch, velocity))
            .unwrap()
    }

    fn assert_buffer_total_is(buffers: &Magnetron, mut f: impl FnMut(f64) -> f64) {
        let mut time = 0.0;
        for sample in buffers.total() {
            assert_approx_eq!(sample, f(time));
            time += SAMPLE_WIDTH_SECS;
        }
    }
}
