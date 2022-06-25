use std::f64::consts::TAU;

use magnetron::{
    automation::AutomationSpec,
    spec::{Creator, Spec},
    waveform::Stage,
};
use serde::{Deserialize, Serialize};

use super::{InBufferSpec, OutSpec};

#[derive(Deserialize, Serialize)]
pub struct Filter<A> {
    #[serde(flatten)]
    pub kind: FilterKind<A>,
    pub in_buffer: InBufferSpec,
    #[serde(flatten)]
    pub out_spec: OutSpec<A>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum FilterKind<A> {
    Copy,
    Pow3,
    Clip {
        limit: A,
    },
    /// Filter as described in https://en.wikipedia.org/wiki/Low-pass_filter#Discrete-time_realization.
    LowPass {
        cutoff: A,
    },
    /// LPF implementation as described in http://shepazu.github.io/Audio-EQ-Cookbook/audio-eq-cookbook.html.
    LowPass2 {
        resonance: A,
        quality: A,
    },
    /// Filter as described in https://en.wikipedia.org/wiki/High-pass_filter#Discrete-time_realization.
    HighPass {
        cutoff: A,
    },
    /// HPF implementation as described in http://shepazu.github.io/Audio-EQ-Cookbook/audio-eq-cookbook.html.
    HighPass2 {
        resonance: A,
        quality: A,
    },
    // BPF (with peak gain) implementation as described in http://shepazu.github.io/Audio-EQ-Cookbook/audio-eq-cookbook.html.
    BandPass {
        center: A,
        quality: A,
    },
    // Notch filter implementation as described in http://shepazu.github.io/Audio-EQ-Cookbook/audio-eq-cookbook.html.
    Notch {
        center: A,
        quality: A,
    },
    // APF implementation as described in http://shepazu.github.io/Audio-EQ-Cookbook/audio-eq-cookbook.html.
    AllPass {
        corner: A,
        quality: A,
    },
}

impl<A: AutomationSpec> Spec for Filter<A> {
    type Created = Stage<A>;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        let in_buffer = self.in_buffer.buffer();
        let out_buffer = self.out_spec.out_buffer.buffer();

        match &self.kind {
            FilterKind::Copy => {
                creator.create_stage(&self.out_spec.out_level, move |buffers, out_level| {
                    buffers.read_1_and_write(in_buffer, out_buffer, out_level, |s| s)
                })
            }
            FilterKind::Pow3 => {
                creator.create_stage(&self.out_spec.out_level, move |buffers, out_level| {
                    buffers.read_1_and_write(in_buffer, out_buffer, out_level, |s| s * s * s)
                })
            }
            FilterKind::Clip { limit } => creator.create_stage(
                (&self.out_spec.out_level, limit),
                move |buffers, (out_level, limit)| {
                    buffers.read_1_and_write(in_buffer, out_buffer, out_level, |s| {
                        s.max(-limit).min(limit)
                    })
                },
            ),
            FilterKind::LowPass { cutoff } => {
                let mut out = Default::default();
                creator.create_stage(
                    (&self.out_spec.out_level, cutoff),
                    move |buffers, (out_level, cutoff)| {
                        let omega_0 = TAU * cutoff * buffers.sample_width_secs();
                        let alpha = (1.0 + omega_0.recip()).recip();
                        buffers.read_1_and_write(in_buffer, out_buffer, out_level, |input| {
                            out += alpha * (input - out);
                            out
                        });
                    },
                )
            }
            FilterKind::LowPass2 { resonance, quality } => {
                let (mut y1, mut y2, mut x1, mut x2) = Default::default();
                creator.create_stage(
                    (&self.out_spec.out_level, resonance, quality),
                    move |buffers, (out_level, resonance, quality)| {
                        let quality = quality.max(1e-10);

                        // Restrict f0 for stability
                        let f0 = (resonance * buffers.sample_width_secs()).max(0.0).min(0.25);
                        let (sin, cos) = (TAU * f0).sin_cos();
                        let alpha = sin / 2.0 / quality;

                        let b1 = 1.0 - cos;
                        let b0 = b1 / 2.0;
                        let b2 = b0;
                        let a0 = 1.0 + alpha;
                        let a1 = -2.0 * cos;
                        let a2 = 1.0 - alpha;

                        buffers.read_1_and_write(in_buffer, out_buffer, out_level, |x0| {
                            let y0 = (b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2) / a0;
                            x2 = x1;
                            x1 = x0;
                            y2 = y1;
                            y1 = y0;
                            y0
                        });
                    },
                )
            }
            FilterKind::HighPass { cutoff } => {
                let (mut out, mut last_input) = Default::default();
                creator.create_stage(
                    (&self.out_spec.out_level, cutoff),
                    move |buffers, (out_level, cutoff)| {
                        let alpha = 1.0 / (1.0 + TAU * buffers.sample_width_secs() * cutoff);

                        buffers.read_1_and_write(in_buffer, out_buffer, out_level, |input| {
                            out = alpha * (out + input - last_input);
                            last_input = input;
                            out
                        });
                    },
                )
            }
            FilterKind::HighPass2 { resonance, quality } => {
                let (mut y1, mut y2, mut x1, mut x2) = Default::default();
                creator.create_stage(
                    (&self.out_spec.out_level, resonance, quality),
                    move |buffers, (out_level, resonance, quality)| {
                        let quality = quality.max(1e-10);

                        // Restrict f0 for stability
                        let f0 = (resonance * buffers.sample_width_secs()).max(0.0).min(0.25);
                        let (sin, cos) = (TAU * f0).sin_cos();
                        let alpha = sin / 2.0 / quality;

                        let b1 = -(1.0 + cos);
                        let b0 = -b1 / 2.0;
                        let b2 = b0;
                        let a0 = 1.0 + alpha;
                        let a1 = -2.0 * cos;
                        let a2 = 1.0 - alpha;

                        buffers.read_1_and_write(in_buffer, out_buffer, out_level, |x0| {
                            let y0 = (b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2) / a0;
                            x2 = x1;
                            x1 = x0;
                            y2 = y1;
                            y1 = y0;
                            y0
                        });
                    },
                )
            }
            FilterKind::BandPass { center, quality } => {
                let (mut y1, mut y2, mut x1, mut x2) = Default::default();
                creator.create_stage(
                    (&self.out_spec.out_level, center, quality),
                    move |buffers, (out_level, center, quality)| {
                        let quality = quality.max(1e-10);

                        // Restrict f0 for stability
                        let f0 = (center * buffers.sample_width_secs()).max(0.0).min(0.5);
                        let (sin, cos) = (TAU * f0).sin_cos();
                        let alpha = sin / 2.0 / quality;

                        let b0 = quality * alpha;
                        let b1 = 0.0;
                        let b2 = -b0;
                        let a0 = 1.0 + alpha;
                        let a1 = -2.0 * cos;
                        let a2 = 1.0 - alpha;

                        buffers.read_1_and_write(in_buffer, out_buffer, out_level, |x0| {
                            let y0 = (b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2) / a0;
                            x2 = x1;
                            x1 = x0;
                            y2 = y1;
                            y1 = y0;
                            y0
                        });
                    },
                )
            }
            FilterKind::Notch { center, quality } => {
                let (mut y1, mut y2, mut x1, mut x2) = Default::default();
                creator.create_stage(
                    (&self.out_spec.out_level, center, quality),
                    move |buffers, (out_level, center, quality)| {
                        let quality = quality.max(1e-10);

                        // Restrict f0 for stability
                        let f0 = (center * buffers.sample_width_secs()).max(0.0).min(0.5);
                        let (sin, cos) = (TAU * f0).sin_cos();
                        let alpha = sin / 2.0 / quality;

                        let b0 = 1.0;
                        let b1 = -2.0 * cos;
                        let b2 = 1.0;
                        let a0 = 1.0 + alpha;
                        let a1 = b1;
                        let a2 = 1.0 - alpha;

                        buffers.read_1_and_write(in_buffer, out_buffer, out_level, |x0| {
                            let y0 = (b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2) / a0;
                            x2 = x1;
                            x1 = x0;
                            y2 = y1;
                            y1 = y0;
                            y0
                        });
                    },
                )
            }
            FilterKind::AllPass { corner, quality } => {
                let (mut y1, mut y2, mut x1, mut x2) = Default::default();
                creator.create_stage(
                    (&self.out_spec.out_level, corner, quality),
                    move |buffers, (out_level, corner, quality)| {
                        let quality = quality.max(1e-10);

                        // Restrict f0 for stability
                        let f0 = (corner * buffers.sample_width_secs()).max(0.0).min(0.5);
                        let (sin, cos) = (TAU * f0).sin_cos();
                        let alpha = sin / 2.0 / quality;

                        let b0 = 1.0 - alpha;
                        let b1 = -2.0 * cos;
                        let b2 = 1.0 + alpha;
                        let a0 = b2;
                        let a1 = b1;
                        let a2 = b0;

                        buffers.read_1_and_write(in_buffer, out_buffer, out_level, |x0| {
                            let y0 = (b0 * x0 + b1 * x1 + b2 * x2 - a1 * y1 - a2 * y2) / a0;
                            x2 = x1;
                            x1 = x0;
                            y2 = y1;
                            y1 = y0;
                            y0
                        });
                    },
                )
            }
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct RingModulator<A> {
    pub in_buffers: (InBufferSpec, InBufferSpec),
    #[serde(flatten)]
    pub out_spec: OutSpec<A>,
}

impl<A: AutomationSpec> Spec for RingModulator<A> {
    type Created = Stage<A>;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        let in_buffers = (self.in_buffers.0.buffer(), self.in_buffers.1.buffer());
        let out_buffer = self.out_spec.out_buffer.buffer();

        creator.create_stage(&self.out_spec.out_level, move |buffers, out_level| {
            buffers.read_2_and_write(in_buffers, out_buffer, out_level, |source_1, source_2| {
                source_1 * source_2
            })
        })
    }
}
