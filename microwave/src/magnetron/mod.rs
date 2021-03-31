use std::mem;

use ringbuf::Consumer;
use waveform::{AudioIn, WaveformProperties};

use self::{
    control::Controller,
    waveform::{AudioOut, Destination, OutBuffer, Source, Waveform},
};

mod functions;
mod util;

pub mod control;
pub mod effects;
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
    pub fn new(num_buffers: usize, buffer_size: usize) -> Self {
        Self {
            audio_in_sychronized: false,
            readable: ReadableBuffers {
                audio_in: WaveformBuffer::new(buffer_size),
                buffers: vec![WaveformBuffer::new(buffer_size); num_buffers],
                audio_out: WaveformBuffer::new(buffer_size),
                total: WaveformBuffer::new(buffer_size),
                zeros: vec![0.0; buffer_size],
            },
            writeable: WaveformBuffer::new(0), // Empty Vec acting as a placeholder
        }
    }

    pub fn clear(&mut self, len: usize) {
        self.readable.audio_in.clear(len);
        self.readable.total.clear(len);
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
        let len = self.readable.total.len;
        for buffer in &mut self.readable.buffers {
            buffer.clear(len);
        }
        self.readable.audio_out.clear(len);

        let properties = &mut waveform.properties;

        let control = WaveformControl {
            sample_secs: sample_width_in_s,
            buffer_secs: sample_width_in_s * len as f64,
            properties,
            storage,
        };

        for stage in &mut waveform.stages {
            stage(self, &control);
        }

        let out_buffer = self.readable.audio_out.read(&self.readable.zeros);
        let change_per_sample = properties.amplitude_change_rate_hz * sample_width_in_s;

        match self.readable.total.write() {
            WriteableBuffer::Dirty(total_buffer) => {
                for (total, out) in total_buffer.iter_mut().zip(&*out_buffer) {
                    properties.curr_amplitude = (properties.curr_amplitude + change_per_sample)
                        .max(0.0)
                        .min(1.0);
                    *total = *out * properties.curr_amplitude
                }
            }
            WriteableBuffer::Clean(total_buffer) => {
                for (total, out) in total_buffer.iter_mut().zip(&*out_buffer) {
                    properties.curr_amplitude = (properties.curr_amplitude + change_per_sample)
                        .max(0.0)
                        .min(1.0);
                    *total += *out * properties.curr_amplitude
                }
            }
        }

        properties.total_time_in_s += sample_width_in_s * len as f64;
    }

    pub fn total(&self) -> &[f64] {
        &self.readable.total.read(&self.readable.zeros)
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
            &OutBuffer::Buffer(index) => self
                .readable
                .buffers
                .get_mut(index)
                .unwrap_or_else(|| report_index_out_of_range(index)),
            OutBuffer::AudioOut(AudioOut::AudioOut) => &mut self.readable.audio_out,
        };
        mem::swap(buffer, &mut self.writeable);
        f(&mut self.writeable, &self.readable);
        let buffer = match out_buffer {
            &OutBuffer::Buffer(index) => self
                .readable
                .buffers
                .get_mut(index)
                .unwrap_or_else(|| report_index_out_of_range(index)),
            OutBuffer::AudioOut(AudioOut::AudioOut) => &mut self.readable.audio_out,
        };
        mem::swap(buffer, &mut self.writeable);
        buffer.dirty = false;
    }
}

fn report_index_out_of_range(index: usize) -> ! {
    panic!(
        "Index {} out of range. Please allocate more waveform buffers.",
        index
    )
}

struct ReadableBuffers {
    audio_in: WaveformBuffer,
    buffers: Vec<WaveformBuffer>,
    audio_out: WaveformBuffer,
    total: WaveformBuffer,
    zeros: Vec<f64>,
}

impl ReadableBuffers {
    fn read_from_buffer(&self, source: &Source) -> &[f64] {
        match source {
            Source::AudioIn(AudioIn::AudioIn) => &self.audio_in,
            &Source::Buffer(index) => &self.buffers[index],
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

    fn write(&mut self) -> WriteableBuffer<'_> {
        match self.dirty {
            true => {
                self.dirty = false;
                WriteableBuffer::Dirty(&mut self.storage[..self.len])
            }
            false => WriteableBuffer::Clean(&mut self.storage[..self.len]),
        }
    }
}

enum WriteableBuffer<'a> {
    Dirty(&'a mut [f64]),
    Clean(&'a mut [f64]),
}

pub struct WaveformControl<'a, S> {
    sample_secs: f64,
    buffer_secs: f64,
    properties: &'a WaveformProperties,
    storage: &'a S,
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use assert_approx_eq::assert_approx_eq;
    use tune::pitch::Pitch;

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
  source: 0
  destination:
    buffer: AudioOut
    intensity: 1.0";
        serde_yaml::from_str::<StageSpec<SynthControl>>(yml).unwrap();
    }

    const NUM_SAMPLES: usize = 44100;
    const SAMPLE_SECS: f64 = 1.0 / 44100.0;

    #[test]
    fn clear_and_resize_buffers() {
        let mut buffers = Magnetron::new(2, 100000);
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
        let mut buffers = Magnetron::new(2, 100000);
        let mut waveform = spec(vec![]).create_waveform(Pitch::from_hz(440.0), 1.0, None);

        buffers.clear(NUM_SAMPLES);
        assert_eq!(buffers.total(), &[0.0; NUM_SAMPLES]);

        buffers.write(&mut waveform, &(), SAMPLE_SECS);
        assert_eq!(buffers.total(), &[0f64; NUM_SAMPLES]);
    }

    #[test]
    fn write_waveform_and_clear() {
        let mut buffers = Magnetron::new(2, 100000);
        let mut waveform = spec(vec![StageSpec::Oscillator(Oscillator {
            kind: OscillatorKind::Sin,
            frequency: LfSourceExpr::WaveformPitch.into(),
            modulation: Modulation::None,
            destination: Destination {
                buffer: OutBuffer::audio_out(),
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
        let mut buffers = Magnetron::new(2, 100000);

        let spec = spec(vec![StageSpec::Oscillator(Oscillator {
            kind: OscillatorKind::Sin,
            frequency: LfSourceExpr::WaveformPitch.into(),
            modulation: Modulation::None,
            destination: Destination {
                buffer: OutBuffer::audio_out(),
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
        let mut buffers = Magnetron::new(2, 100000);

        let mut waveform = spec(vec![
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSource::Value(330.0),
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::Buffer(0),
                    intensity: LfSource::Value(440.0),
                },
            }),
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSourceExpr::WaveformPitch.into(),
                modulation: Modulation::ByFrequency(Source::Buffer(0)),
                destination: Destination {
                    buffer: OutBuffer::audio_out(),
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
        let mut buffers = Magnetron::new(2, 100000);
        let mut waveform = spec(vec![
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSource::Value(330.0),
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::Buffer(0),
                    intensity: LfSource::Value(0.44),
                },
            }),
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSourceExpr::WaveformPitch.into(),
                modulation: Modulation::ByPhase(Source::Buffer(0)),
                destination: Destination {
                    buffer: OutBuffer::audio_out(),
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
        let mut buffers = Magnetron::new(2, 100000);
        let mut waveform = spec(vec![
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSourceExpr::WaveformPitch.into(),
                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::Buffer(0),
                    intensity: LfSource::Value(1.0),
                },
            }),
            StageSpec::Oscillator(Oscillator {
                kind: OscillatorKind::Sin,
                frequency: LfSource::Value(1.5) * LfSourceExpr::WaveformPitch.into(),

                modulation: Modulation::None,
                destination: Destination {
                    buffer: OutBuffer::Buffer(1),
                    intensity: LfSource::Value(1.0),
                },
            }),
            StageSpec::RingModulator(RingModulator {
                sources: (Source::Buffer(0), Source::Buffer(1)),
                destination: Destination {
                    buffer: OutBuffer::audio_out(),
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
