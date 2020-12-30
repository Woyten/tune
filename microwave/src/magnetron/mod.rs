use std::mem;

use ringbuf::Consumer;
use tune::pitch::Pitch;

use self::{
    control::Controller,
    waveform::{Destination, OutBuffer, Source, Waveform},
};

mod functions;

pub mod control;
pub mod envelope;
pub mod filter;
pub mod oscillator;
pub mod source;
pub mod waveform;

pub struct Magnetron {
    audio_in_sychronized: bool,
    readable: ReadableBuffers,
    writeable: WaveformBuffer,
}

impl Magnetron {
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
        }
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

    pub fn write<S>(&mut self, waveform: &mut Waveform<S>, storage: &S, sample_width_in_s: f64) {
        let len = self.readable.zeros.len();
        self.readable.buffer0.clear(len);
        self.readable.buffer1.clear(len);
        self.readable.out.clear(len);

        let control = WaveformControl {
            pitch: waveform.pitch(),
            sample_secs: sample_width_in_s,
            buffer_secs: sample_width_in_s * len as f64,
            total_secs: waveform.total_time_in_s,
            storage,
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

    fn write_1_read_0<C: Controller>(
        &mut self,
        destination: &mut Destination<C>,
        control: &WaveformControl<C::Storage>,
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

    fn write_1_read_1<C: Controller>(
        &mut self,
        destination: &mut Destination<C>,
        source: &Source,
        control: &WaveformControl<C::Storage>,
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

    fn write_1_read_2<C: Controller>(
        &mut self,
        destination: &mut Destination<C>,
        sources: &(Source, Source),
        control: &WaveformControl<C::Storage>,
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

struct ReadableBuffers {
    audio_in: WaveformBuffer,
    buffer0: WaveformBuffer,
    buffer1: WaveformBuffer,
    out: WaveformBuffer,
    total: WaveformBuffer,
    zeros: Vec<f64>,
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

pub struct WaveformControl<'a, S> {
    pitch: Pitch,
    sample_secs: f64,
    buffer_secs: f64,
    total_secs: f64,
    storage: &'a S,
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use assert_approx_eq::assert_approx_eq;

    use crate::synth::SynthControl;

    use super::{
        control::NoControl,
        envelope::EnvelopeType,
        filter::RingModulator,
        oscillator::{Modulation, Oscillator, OscillatorKind},
        source::{LfSource, LfSourceExpr},
        waveform::{Destination, StageSpec, WaveformSpec},
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
  source: Buffer0
  destination:
    buffer: AudioOut
    intensity: 1.0";
        serde_yaml::from_str::<StageSpec<SynthControl>>(yml).unwrap();
    }

    const NUM_SAMPLES: usize = 44100;
    const SAMPLE_SECS: f64 = 1.0 / 44100.0;

    #[test]
    fn clear_and_resize_buffers() {
        let mut buffers = Magnetron::new();
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
        let mut buffers = Magnetron::new();
        let mut waveform = spec(vec![]).create_waveform(Pitch::from_hz(440.0), 1.0, None);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.total(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &(), SAMPLE_SECS);
        assert_eq!(buffers.total(), &[0f64; NUM_SAMPLES]);
    }

    #[test]
    fn write_waveform_and_clear() {
        let mut buffers = Magnetron::new();
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

        buffers.write(&mut waveform, &(), SAMPLE_SECS);
        assert_buffer_total_is(&buffers, |t| (TAU * 440.0 * t).sin());

        buffers.clear(128);
        assert_eq!(buffers.total(), &[0f64; 128]);
    }

    #[test]
    fn mix_two_wavforms() {
        let mut buffers = Magnetron::new();

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

        buffers.write(&mut waveform1, &(), SAMPLE_SECS);
        assert_buffer_total_is(&buffers, |t| 0.7 * (440.0 * TAU * t).sin());

        buffers.write(&mut waveform2, &(), SAMPLE_SECS);
        assert_buffer_total_is(&buffers, |t| {
            0.7 * (440.0 * TAU * t).sin() + 0.8 * (660.0 * TAU * t).sin()
        });
    }

    #[test]
    fn modulate_by_frequency() {
        let mut buffers = Magnetron::new();

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

        buffers.write(&mut waveform, &(), SAMPLE_SECS);
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
        let mut buffers = Magnetron::new();
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

        buffers.write(&mut waveform, &(), SAMPLE_SECS);
        assert_buffer_total_is(&buffers, |t| {
            ((550.0 * t + (330.0 * TAU * t).sin() * 0.44) * TAU).sin()
        });
    }

    #[test]
    fn ring_modulation() {
        let mut buffers = Magnetron::new();
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

        buffers.write(&mut waveform, &(), SAMPLE_SECS);
        assert_buffer_total_is(&buffers, |t| {
            (440.0 * t * TAU).sin() * (660.0 * t * TAU).sin()
        });
    }

    fn spec(stages: Vec<StageSpec<NoControl>>) -> WaveformSpec<NoControl> {
        WaveformSpec {
            name: String::new(),
            envelope_type: EnvelopeType::Organ,
            stages,
        }
    }

    fn assert_buffer_total_is(buffers: &Magnetron, mut f: impl FnMut(f64) -> f64) {
        let mut time = 0.0;
        for sample in buffers.total() {
            assert_approx_eq!(sample, f(time));
            time += SAMPLE_SECS;
        }
    }
}
