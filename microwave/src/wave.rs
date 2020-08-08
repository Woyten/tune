use crate::effects::DifferentialFilter;
use std::f64::consts::PI;
use tune::pitch::Pitch;

pub fn all_waveforms() -> Vec<Patch> {
    vec![
        Patch {
            name: "Sine",
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| o.sine(),
            },
        },
        Patch {
            name: "Clipped Sine",
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| {
                    let loudness_correction = 2.0;
                    (2.0 * o.sine()).max(-1.0).min(1.0) / loudness_correction
                },
            },
        },
        Patch {
            name: "Triangle",
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| o.triangle(),
            },
        },
        Patch {
            name: "Triangle³",
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| (o.triangle()).powi(3),
            },
        },
        Patch {
            name: "Square",
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| {
                    let loudness_correction = 4.0;
                    o.square() / loudness_correction
                },
            },
        },
        Patch {
            name: "Sawtooth",
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| {
                    let loudness_correction = 2.0;
                    o.sawtooth() / loudness_correction
                },
            },
        },
        Patch {
            name: "Fat Sawtooth 1",
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
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| {
                    let loudness_correction = 2.0;
                    o.sine() * (1.0 + o.mul(6).sine()) / loudness_correction
                },
            },
        },
        Patch {
            name: "Ring Modulation 2",
            waveform_type: PatchProperties::Simple {
                signal_fn: |o| {
                    let loudness_correction = 1.125;
                    o.sine() * (1.0 + o.mul(6).sine() / 8.0) / loudness_correction
                },
            },
        },
        Patch {
            name: "Bright Pad",
            waveform_type: PatchProperties::Complex {
                phase_fn: |oscis, d_phase, duration_secs| {
                    oscis.o0.advance_phase(d_phase);
                    let ratio = 10.0 * (1.0 - (-1.0 * duration_secs).exp2());
                    oscis.filter.advance_low_pass_phase(d_phase * ratio);
                    oscis.filter.write_input(oscis.o0.sawtooth())
                },
                signal_fn: |oscis| {
                    let loudness_correction = 2.0;
                    oscis.filter.signal() / loudness_correction
                },
            },
        },
    ]
}

pub struct Patch {
    name: &'static str,
    waveform_type: PatchProperties,
}

impl Patch {
    pub fn name(&self) -> &'static str {
        self.name
    }

    pub fn new_waveform(&self, pitch: Pitch, amplitude: f64, decay_time_secs: f64) -> Waveform {
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
        Waveform {
            pitch,
            decay_time_secs,
            amplitude,
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
    decay_time_secs: f64,
    amplitude: f64,
    state: WaveformState,
}

impl Waveform {
    pub fn advance_secs(&mut self, d_secs: f64) {
        let d_phase = d_secs * self.pitch.as_hz();
        match &mut self.state {
            WaveformState::Simple {
                signal_fn: _,
                oscillator,
            } => oscillator.advance_phase(d_phase),
            WaveformState::Complex {
                signal_fn: _,
                phase_fn,
                oscillators,
                duration_secs,
            } => {
                *duration_secs += d_secs;
                phase_fn(oscillators, d_phase, *duration_secs);
            }
        }
    }

    pub fn advance_fade_secs(&mut self, d_secs: f64) {
        self.amplitude = (self.amplitude - d_secs / self.decay_time_secs).max(0.0);
    }

    pub fn set_frequency(&mut self, pitch: Pitch) {
        self.pitch = pitch;
    }

    pub fn signal(&self) -> f64 {
        match &self.state {
            WaveformState::Simple {
                oscillator,
                signal_fn,
            } => signal_fn(oscillator),
            WaveformState::Complex {
                phase_fn: _,
                signal_fn,
                duration_secs: _,
                oscillators,
            } => signal_fn(oscillators),
        }
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
    pub filter: DifferentialFilter,
}

type PhaseFn<T> = fn(&mut T, f64, f64);
type SignalFn<T> = fn(&T) -> f64;
