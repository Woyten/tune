use std::{iter, mem};

use ringbuf::Consumer;
use tune::pitch::Ratio;
use waveform::{AudioIn, WaveformProperties};

use self::waveform::{AudioOut, InBuffer, OutBuffer, OutSpec, Waveform};

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
    sample_width_secs: f64,
    audio_in_synchronized: bool,
    readable: ReadableBuffers,
    writeable: WaveformBuffer,
    pitch_bend: Ratio,
}

impl Magnetron {
    pub fn new(sample_width_secs: f64, num_buffers: usize, buffer_size: usize) -> Self {
        Self {
            sample_width_secs,
            audio_in_synchronized: false,
            readable: ReadableBuffers {
                audio_in: WaveformBuffer::new(buffer_size),
                buffers: vec![WaveformBuffer::new(buffer_size); num_buffers],
                audio_out: WaveformBuffer::new(buffer_size),
                total: WaveformBuffer::new(buffer_size),
                zeros: vec![0.0; buffer_size],
            },
            writeable: WaveformBuffer::new(0), // Empty Vec acting as a placeholder
            pitch_bend: Default::default(),
        }
    }

    pub fn clear(&mut self, len: usize) {
        self.readable.audio_in.clear(len);
        self.readable.total.clear(len);
    }

    pub fn set_audio_in(&mut self, len: usize, audio_source: &mut Consumer<f64>) {
        let audio_in_buffer = &mut self.readable.audio_in;
        if audio_source.len() >= 2 * len {
            for element in &mut audio_in_buffer.storage[0..len] {
                let l = audio_source.pop().unwrap_or_default();
                let r = audio_source.pop().unwrap_or_default();
                *element = l + r / 2.0;
            }
            audio_in_buffer.dirty = false;
            if !self.audio_in_synchronized {
                self.audio_in_synchronized = true;
                println!("[INFO] Audio-in synchronized");
            }
        } else if self.audio_in_synchronized {
            println!("[WARNING] Exchange buffer underrun - Waiting for audio-in to be in sync with audio-out");
        }
    }

    pub fn set_pitch_bend(&mut self, pitch_bend: Ratio) {
        self.pitch_bend = pitch_bend
    }

    pub fn write<S>(
        &mut self,
        waveform: &mut Waveform<S>,
        storage: &S,
        note_suspension: f64,
    ) -> bool {
        let len = self.readable.total.len;
        for buffer in &mut self.readable.buffers {
            buffer.clear(len);
        }
        self.readable.audio_out.clear(len);

        let properties = &mut waveform.properties;

        let render_window_secs = self.sample_width_secs * len as f64;
        let context = AutomationContext {
            render_window_secs,
            pitch_bend: self.pitch_bend,
            properties,
            storage,
        };

        for stage in &mut waveform.stages {
            stage.render(self, &context);
        }

        let out_buffer = self.readable.audio_out.read(&self.readable.zeros);

        let from_amplitude = waveform.envelope.get_value(
            properties.secs_since_pressed,
            properties.secs_since_released,
        );

        properties.secs_since_pressed += render_window_secs;
        properties.secs_since_released += render_window_secs * (1.0 - note_suspension);

        let to_amplitude = waveform.envelope.get_value(
            properties.secs_since_pressed,
            properties.secs_since_released,
        );

        let mut curr_amplitude = from_amplitude;
        let slope = (to_amplitude - from_amplitude) / len as f64;

        self.readable.total.write(out_buffer.iter().map(|src| {
            let result = src * curr_amplitude * properties.velocity;
            curr_amplitude = (curr_amplitude + slope).clamp(0.0, 1.0);
            result
        }));

        waveform.envelope.is_active(properties.secs_since_released)
    }

    pub fn total(&self) -> &[f64] {
        self.readable.total.read(&self.readable.zeros)
    }

    fn read_0_and_write(
        &mut self,
        out_buffer: &OutBuffer,
        out_level: f64,
        mut f: impl FnMut() -> f64,
    ) {
        self.rw_access_split(out_buffer, |_, write_access| {
            write_access.write(iter::repeat_with(|| f() * out_level))
        });
    }

    fn read_1_and_write(
        &mut self,
        in_buffer: &InBuffer,
        out_buffer: &OutBuffer,
        out_level: f64,
        mut f: impl FnMut(f64) -> f64,
    ) {
        self.rw_access_split(out_buffer, |read_access, write_access| {
            write_access.write(
                read_access
                    .read(in_buffer)
                    .iter()
                    .map(|&src| f(src) * out_level),
            )
        });
    }

    fn read_2_and_write(
        &mut self,
        in_buffers: &(InBuffer, InBuffer),
        out_buffer: &OutBuffer,
        out_level: f64,
        mut f: impl FnMut(f64, f64) -> f64,
    ) {
        self.rw_access_split(out_buffer, |read_access, write_access| {
            write_access.write(
                read_access
                    .read(&in_buffers.0)
                    .iter()
                    .zip(read_access.read(&in_buffers.1))
                    .map(|(&src_0, &src_1)| f(src_0, src_1) * out_level),
            )
        });
    }

    fn rw_access_split(
        &mut self,
        out_buffer: &OutBuffer,
        mut rw_access_fn: impl FnMut(&ReadableBuffers, &mut WaveformBuffer),
    ) {
        self.readable.swap(out_buffer, &mut self.writeable);
        rw_access_fn(&self.readable, &mut self.writeable);
        self.readable.swap(out_buffer, &mut self.writeable);
    }
}

struct ReadableBuffers {
    audio_in: WaveformBuffer,
    buffers: Vec<WaveformBuffer>,
    audio_out: WaveformBuffer,
    total: WaveformBuffer,
    zeros: Vec<f64>,
}

impl ReadableBuffers {
    fn swap(&mut self, buffer_a_ref: &OutBuffer, buffer_b: &mut WaveformBuffer) {
        let buffer_a = match buffer_a_ref {
            &OutBuffer::Buffer(index) => self.buffers.get_mut(index).unwrap_or_else(|| {
                panic!(
                    "Index {} out of range. Please allocate more waveform buffers.",
                    index
                )
            }),
            OutBuffer::AudioOut(AudioOut::AudioOut) => &mut self.audio_out,
        };
        mem::swap(buffer_a, buffer_b);
    }

    fn read(&self, in_buffer: &InBuffer) -> &[f64] {
        match *in_buffer {
            InBuffer::AudioIn(AudioIn::AudioIn) => &self.audio_in,
            InBuffer::Buffer(index) => &self.buffers[index],
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
    render_window_secs: f64,
    pitch_bend: Ratio,
    properties: &'a WaveformProperties,
    storage: &'a S,
}

impl<'a, S> AutomationContext<'a, S> {
    pub fn read<V: AutomatedValue<S>>(&self, value: &mut V) -> V::Value {
        value.use_context(self)
    }
}

pub trait AutomatedValue<S> {
    type Value;

    fn use_context(&mut self, context: &AutomationContext<S>) -> Self::Value;
}

impl<S, A1: AutomatedValue<S>, A2: AutomatedValue<S>> AutomatedValue<S> for (A1, A2) {
    type Value = (A1::Value, A2::Value);

    fn use_context(&mut self, context: &AutomationContext<S>) -> Self::Value {
        (context.read(&mut self.0), context.read(&mut self.1))
    }
}

impl<S, A1: AutomatedValue<S>, A2: AutomatedValue<S>, A3: AutomatedValue<S>> AutomatedValue<S>
    for (A1, A2, A3)
{
    type Value = (A1::Value, A2::Value, A3::Value);

    fn use_context(&mut self, context: &AutomationContext<S>) -> Self::Value {
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

    use crate::synth::SynthControl;

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
    Control:
      controller: Modulation
      from: 0.0
      to: 10000.0
  quality: 5.0
  in_buffer: 0
  out_buffer: AudioOut
  out_level: 1.0";
        serde_yaml::from_str::<StageSpec<SynthControl>>(yml).unwrap();
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
                frequency: LfSourceUnit::WaveformPitch.into(),
                modulation: Modulation::None,
                out_spec: OutSpec {
                    out_buffer: OutBuffer::audio_out(),
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
            frequency: LfSourceUnit::WaveformPitch.into(),
            modulation: Modulation::None,
            out_spec: OutSpec {
                out_buffer: OutBuffer::audio_out(),
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
                    out_buffer: OutBuffer::Buffer(0),
                    out_level: LfSource::Value(440.0),
                },
            }),
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSourceUnit::WaveformPitch.into(),
                modulation: Modulation::ByFrequency {
                    mod_buffer: InBuffer::Buffer(0),
                },
                out_spec: OutSpec {
                    out_buffer: OutBuffer::audio_out(),
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
                    out_buffer: OutBuffer::Buffer(0),
                    out_level: LfSource::Value(0.44),
                },
            }),
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSourceUnit::WaveformPitch.into(),
                modulation: Modulation::ByPhase {
                    mod_buffer: InBuffer::Buffer(0),
                },
                out_spec: OutSpec {
                    out_buffer: OutBuffer::audio_out(),
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
                frequency: LfSourceUnit::WaveformPitch.into(),
                modulation: Modulation::None,
                out_spec: OutSpec {
                    out_buffer: OutBuffer::Buffer(0),
                    out_level: LfSource::Value(1.0),
                },
            }),
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSource::Value(1.5) * LfSourceUnit::WaveformPitch.into(),

                modulation: Modulation::None,
                out_spec: OutSpec {
                    out_buffer: OutBuffer::Buffer(1),
                    out_level: LfSource::Value(1.0),
                },
            }),
            StageSpec::RingModulator(RingModulator {
                in_buffers: (InBuffer::Buffer(0), InBuffer::Buffer(1)),
                out_spec: OutSpec {
                    out_buffer: OutBuffer::audio_out(),
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

    fn spec(stages: Vec<StageSpec<NoControl>>) -> WaveformSpec<NoControl> {
        WaveformSpec {
            name: String::new(),
            envelope: "Organ".to_owned(),
            stages,
        }
    }

    fn create_waveform(
        spec: &WaveformSpec<NoControl>,
        pitch: Pitch,
        velocity: f64,
    ) -> Waveform<()> {
        let mut envelope_map = HashMap::new();
        envelope_map.insert(
            "test".to_owned(),
            EnvelopeSpec {
                name: "test".to_owned(),
                attack_time: -1e-10,
                release_time: 1e-10,
                decay_rate: 0.0,
            },
        );
        Creator::new(envelope_map)
            .create_waveform(spec, pitch, velocity, "test")
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
