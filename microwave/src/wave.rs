use crate::effects::{LowPassFilter, ResonanceFilter};
use std::f64::consts::PI;
use tune::pitch::Pitch;

pub fn all_waveforms() -> Vec<Patch> {
    vec![
        Patch {
            name: "Sine",
            envelope_type: EnvelopeType::Organ,
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| o.sine(),
            },
        },
        Patch {
            name: "Clipped Sine",
            envelope_type: EnvelopeType::Organ,
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| {
                    let loudness_correction = 2.0;
                    (2.0 * o.sine()).max(-1.0).min(1.0) / loudness_correction
                },
            },
        },
        Patch {
            name: "Triangle",
            envelope_type: EnvelopeType::Organ,
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| o.triangle(),
            },
        },
        Patch {
            name: "Triangle³",
            envelope_type: EnvelopeType::Organ,
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| (o.triangle()).powi(3),
            },
        },
        Patch {
            name: "Square",
            envelope_type: EnvelopeType::Organ,
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| {
                    let loudness_correction = 4.0;
                    o.square() / loudness_correction
                },
            },
        },
        Patch {
            name: "Sawtooth",
            envelope_type: EnvelopeType::Organ,
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| {
                    let loudness_correction = 2.0;
                    o.sawtooth() / loudness_correction
                },
            },
        },
        Patch {
            name: "Fat Sawtooth 1",
            envelope_type: EnvelopeType::Organ,
            waveform_type: PatchProperties::Complex {
                phase_fn: |oscis, d_phase, _duration_secs| {
                    oscis.o0.advance_phase(d_phase * 1.005);
                    oscis.o1.advance_phase(d_phase / 1.005);
                },
                signal_fn: |oscis| {
                    let loudness_correction = 4.0;
                    (oscis.o0.sawtooth() + oscis.o1.sawtooth()) / loudness_correction
                },
            },
        },
        Patch {
            name: "Fat Sawtooth 2",
            envelope_type: EnvelopeType::Organ,
            waveform_type: PatchProperties::Complex {
                phase_fn: |oscis, d_phase, _duration_secs| {
                    oscis.o0.advance_phase(d_phase / 1.005);
                    oscis.o1.advance_phase(d_phase * 1.005 * 2.0);
                },
                signal_fn: |oscis| {
                    let loudness_correction = 4.0;
                    (oscis.o0.sawtooth() + oscis.o1.sawtooth()) / loudness_correction
                },
            },
        },
        Patch {
            // This sound implicitly depends on the frequency (d_phase + ...)
            name: "Electric Piano",
            envelope_type: EnvelopeType::Piano,
            waveform_type: PatchProperties::Complex {
                phase_fn: |oscis, d_phase, _duration_secs| {
                    oscis.o0.advance_phase(d_phase);
                    oscis.o1.advance_phase(d_phase + oscis.o0.sine() / 100.0);
                },
                signal_fn: |oscis| oscis.o1.sine(),
            },
        },
        Patch {
            // This sound implicitly depends on the frequency (d_phase + ...)
            name: "Clavinet",
            envelope_type: EnvelopeType::Piano,
            waveform_type: PatchProperties::Complex {
                phase_fn: |oscis, d_phase, _duration_secs| {
                    oscis.o0.advance_phase(d_phase);
                    oscis.o1.advance_phase(d_phase + oscis.o0.sine() / 100.0);
                },
                signal_fn: |oscis| oscis.o1.triangle(),
            },
        },
        Patch {
            name: "Organ 1",
            envelope_type: EnvelopeType::Organ,
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| {
                    let loudness_correction = 1.875;
                    (o.sine() - o.mul(2).sine() / 2.0 + o.mul(4).sine() / 4.0
                        - o.mul(8).sine() / 8.0)
                        / loudness_correction
                },
            },
        },
        Patch {
            name: "Organ 2",
            envelope_type: EnvelopeType::Organ,
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| {
                    let loudness_correction = 1.875;
                    (o.sine() - o.mul(2).sine() / 2.0 + o.mul(4).sine() / 4.0
                        - o.mul(6).sine() / 8.0)
                        / loudness_correction
                },
            },
        },
        Patch {
            name: "Bell 1",
            envelope_type: EnvelopeType::Bell,
            waveform_type: PatchProperties::Complex {
                phase_fn: |oscis, d_phase, _duration_secs| {
                    oscis.o0.advance_phase(1.0 * d_phase);
                    oscis.o1.advance_phase(3.0 * d_phase);
                    oscis.o2.advance_phase(5.0 * d_phase);
                    oscis.o3.advance_phase(7.0 * d_phase);
                    oscis.o4.advance_phase(9.0 * d_phase);
                },
                signal_fn: |oscis| {
                    let loudness_correction = 1.9375;
                    (oscis.o0.sine() - oscis.o1.sine() / 2.0 + oscis.o2.sine() / 4.0
                        - oscis.o3.sine() / 8.0
                        + oscis.o4.sine() / 16.0)
                        / loudness_correction
                },
            },
        },
        Patch {
            name: "Bell 2 (12-EDO)",
            envelope_type: EnvelopeType::Bell,
            waveform_type: PatchProperties::Complex {
                phase_fn: |oscis, d_phase, _duration_secs| {
                    oscis.o0.advance_phase(1.0000 * d_phase);
                    oscis.o1.advance_phase(2.9966 * d_phase);
                    oscis.o2.advance_phase(5.3394 * d_phase);
                    oscis.o3.advance_phase(7.1272 * d_phase);
                    oscis.o4.advance_phase(8.9797 * d_phase);
                },
                signal_fn: |oscis| {
                    let loudness_correction = 1.9375;
                    (oscis.o0.sine() - oscis.o1.sine() / 2.0 + oscis.o2.sine() / 4.0
                        - oscis.o3.sine() / 8.0
                        + oscis.o4.sine() / 16.0)
                        / loudness_correction
                },
            },
        },
        Patch {
            name: "Ring Modulation 1",
            envelope_type: EnvelopeType::Organ,
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| {
                    let loudness_correction = 2.0;
                    o.sine() * (1.0 + o.mul(6).sine()) / loudness_correction
                },
            },
        },
        Patch {
            name: "Ring Modulation 2",
            envelope_type: EnvelopeType::Organ,
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| {
                    let loudness_correction = 1.125;
                    o.sine() * (1.0 + o.mul(6).sine() / 8.0) / loudness_correction
                },
            },
        },
        Patch {
            name: "Bright Pad",
            envelope_type: EnvelopeType::Pad,
            waveform_type: PatchProperties::Complex {
                phase_fn: |oscis, d_phase, duration_secs| {
                    oscis.o0.advance_phase(d_phase);
                    let max_value = 10.0;
                    let slope = 5.0; // attack time = 2s
                    let ratio = (duration_secs * slope).min(max_value);
                    oscis
                        .low_pass
                        .advance_phase(oscis.o0.sawtooth(), d_phase * ratio);
                },
                signal_fn: |oscis| {
                    let loudness_correction = 2.0;
                    oscis.low_pass.signal() / loudness_correction
                },
            },
        },
        Patch {
            name: "Resonance Pad",
            envelope_type: EnvelopeType::Pad,
            waveform_type: PatchProperties::Complex {
                phase_fn: |oscis, d_phase, duration_secs| {
                    oscis.o0.advance_phase(d_phase);
                    let max_value = 32.0; // 5 octaves
                    let slope = 16.0; // attack time = 2s
                    let ratio = 1.0 + (duration_secs * slope).min(max_value);
                    oscis
                        .resonance
                        .advance_phase(oscis.o0.sawtooth(), 0.2, d_phase * ratio);
                },
                signal_fn: |oscis| {
                    let loudness_correction = 2.0;
                    oscis.resonance.signal() / loudness_correction
                },
            },
        },
    ]
}

#[derive(Copy, Clone, Debug)]
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

pub struct Patch {
    name: &'static str,
    envelope_type: EnvelopeType,
    waveform_type: PatchProperties,
}

impl Patch {
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn envelope_type(&self) -> EnvelopeType {
        self.envelope_type
    }

    pub fn new_waveform(
        &self,
        pitch: Pitch,
        amplitude: f64,
        envelope_type: Option<EnvelopeType>,
    ) -> Waveform {
        let state = match self.waveform_type {
            PatchProperties::Simple { signal_fn } => WaveformState::Simple {
                oscillator: Default::default(),
                signal_fn,
            },
            PatchProperties::Complex {
                phase_fn,
                signal_fn,
            } => WaveformState::Complex {
                oscillators: Default::default(),
                duration_secs: 0.0,
                phase_fn,
                signal_fn,
            },
        };
        let envelope_type = envelope_type.unwrap_or(self.envelope_type);
        Waveform {
            pitch,
            amplitude,
            amplitude_change_rate_hz: -amplitude * envelope_type.decay_rate_hz(),
            envelope_type,
            state,
        }
    }
}

enum PatchProperties {
    Simple {
        signal_fn: SignalFn<Oscillator>,
    },
    Complex {
        phase_fn: PhaseFn<ComplexState>,
        signal_fn: SignalFn<ComplexState>,
    },
}

pub struct Waveform {
    pitch: Pitch,
    amplitude: f64,
    amplitude_change_rate_hz: f64,
    envelope_type: EnvelopeType,
    state: WaveformState,
}

impl Waveform {
    pub fn advance_secs(&mut self, buffer: &mut [f32], d_secs: f64, volume: f64) {
        let d_phase = d_secs * self.pitch.as_hz();
        let change_per_sample = self.amplitude_change_rate_hz * d_secs;

        for samples in buffer.chunks_exact_mut(2) {
            self.amplitude = (self.amplitude + change_per_sample).max(0.0).min(1.0);
            match &mut self.state {
                WaveformState::Simple {
                    oscillator,
                    signal_fn,
                } => {
                    oscillator.advance_phase(d_phase);
                    let signal = (signal_fn(&oscillator) * volume * self.amplitude) as f32;
                    for sample in samples {
                        *sample += signal;
                    }
                }
                WaveformState::Complex {
                    oscillators,
                    duration_secs,
                    phase_fn,
                    signal_fn,
                } => {
                    *duration_secs += d_secs;
                    phase_fn(oscillators, d_phase, *duration_secs);
                    let signal = (signal_fn(&oscillators) * volume * self.amplitude) as f32;
                    for sample in samples {
                        *sample += signal;
                    }
                }
            }
        }
    }

    pub fn set_frequency(&mut self, pitch: Pitch) {
        self.pitch = pitch;
    }

    pub fn start_fading(&mut self) {
        self.amplitude_change_rate_hz = -self.amplitude * self.envelope_type.release_rate_hz();
    }

    pub fn amplitude(&self) -> f64 {
        self.amplitude
    }
}

enum WaveformState {
    Simple {
        signal_fn: SignalFn<Oscillator>,
        oscillator: Oscillator,
    },
    Complex {
        phase_fn: PhaseFn<ComplexState>,
        signal_fn: SignalFn<ComplexState>,
        duration_secs: f64,
        oscillators: ComplexState,
    },
}

#[derive(Clone, Default)]
pub struct Oscillator {
    phase: f64,
}

impl Oscillator {
    fn advance_phase(&mut self, d_phase: f64) {
        *self = self.shifted(d_phase);
    }

    pub fn sine(&self) -> f64 {
        (2.0 * PI * self.phase).sin()
    }

    pub fn triangle(&self) -> f64 {
        ((self.phase + 0.75) % 1.0 - 0.5).abs() * 4.0 - 1.0
    }

    pub fn square(&self) -> f64 {
        (self.phase - 0.5).signum()
    }

    pub fn sawtooth(&self) -> f64 {
        (self.phase + 0.5).fract() * 2.0 - 1.0
    }

    pub fn mul(&self, factor: i32) -> Self {
        self.map_phase(|phase| phase * f64::from(factor))
    }

    pub fn shifted(&self, d_phase: f64) -> Self {
        self.map_phase(|phase| phase + d_phase)
    }

    pub fn map_phase(&self, map_fn: impl Fn(f64) -> f64) -> Oscillator {
        Self {
            phase: map_fn(self.phase).rem_euclid(1.0),
        }
    }
}

#[derive(Default)]
pub struct ComplexState {
    pub o0: Oscillator,
    pub o1: Oscillator,
    pub o2: Oscillator,
    pub o3: Oscillator,
    pub o4: Oscillator,
    pub o5: Oscillator,
    pub o6: Oscillator,
    pub o7: Oscillator,
    pub low_pass: LowPassFilter,
    pub resonance: ResonanceFilter,
}

type PhaseFn<T> = fn(&mut T, f64, f64);
type SignalFn<T> = fn(&T) -> f64;
