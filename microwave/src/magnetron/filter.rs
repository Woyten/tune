use std::f64::consts::TAU;

use serde::{Deserialize, Serialize};

use super::{
    control::Controller,
    source::LfSource,
    waveform::{Source, Stage},
    Destination,
};

#[derive(Deserialize, Serialize)]
pub struct Filter<C> {
    #[serde(flatten)]
    pub kind: FilterKind<C>,
    pub source: Source,
    pub destination: Destination<C>,
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "kind")]
pub enum FilterKind<C> {
    Copy,
    Pow3,
    Clip {
        limit: LfSource<C>,
    },
    /// Filter as described in https://en.wikipedia.org/wiki/Low-pass_filter#Discrete-time_realization.
    LowPass {
        cutoff: LfSource<C>,
    },
    /// LPF implementation as described in http://shepazu.github.io/Audio-EQ-Cookbook/audio-eq-cookbook.html.
    LowPass2 {
        resonance: LfSource<C>,
        quality: LfSource<C>,
    },
    /// Filter as described in https://en.wikipedia.org/wiki/High-pass_filter#Discrete-time_realization.
    HighPass {
        cutoff: LfSource<C>,
    },
    /// HPF implementation as described in http://shepazu.github.io/Audio-EQ-Cookbook/audio-eq-cookbook.html.
    HighPass2 {
        resonance: LfSource<C>,
        quality: LfSource<C>,
    },
    // BPF (with peak gain) implementation as described in http://shepazu.github.io/Audio-EQ-Cookbook/audio-eq-cookbook.html.
    BandPass {
        center: LfSource<C>,
        quality: LfSource<C>,
    },
    // Notch filter implementation as described in http://shepazu.github.io/Audio-EQ-Cookbook/audio-eq-cookbook.html.
    Notch {
        center: LfSource<C>,
        quality: LfSource<C>,
    },
    // APF implementation as described in http://shepazu.github.io/Audio-EQ-Cookbook/audio-eq-cookbook.html.
    AllPass {
        corner: LfSource<C>,
        quality: LfSource<C>,
    },
}

impl<C: Controller> Filter<C> {
    pub fn create_stage(&self) -> Stage<C::Storage> {
        let source = self.source.clone();
        let mut destination = self.destination.clone();
        match &self.kind {
            FilterKind::Copy => Box::new(move |buffers, control| {
                buffers.write_1_read_1(&mut destination, &source, &control, |s| s)
            }),
            FilterKind::Pow3 => Box::new(move |buffers, control| {
                buffers.write_1_read_1(&mut destination, &source, &control, |s| s * s * s)
            }),
            FilterKind::Clip { limit } => {
                let mut limit = limit.clone();
                Box::new(move |buffers, control| {
                    let limit = limit.next(control);
                    buffers.write_1_read_1(&mut destination, &source, &control, |s| {
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
            FilterKind::HighPass2 { resonance, quality } => {
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

                    let b1 = -(1.0 + cos);
                    let b0 = -b1 / 2.0;
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
            FilterKind::BandPass { center, quality } => {
                let mut center = center.clone();
                let mut quality = quality.clone();

                let (mut y1, mut y2, mut x1, mut x2) = Default::default();
                Box::new(move |buffers, control| {
                    let center = center.next(control);
                    let quality = quality.next(control).max(1e-10);

                    // Restrict f0 for stability
                    let f0 = (center * control.sample_secs).max(0.0).min(0.5);
                    let (sin, cos) = (TAU * f0).sin_cos();
                    let alpha = sin / 2.0 / quality;

                    let b0 = quality * alpha;
                    let b1 = 0.0;
                    let b2 = -b0;
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
            FilterKind::Notch { center, quality } => {
                let mut center = center.clone();
                let mut quality = quality.clone();

                let (mut y1, mut y2, mut x1, mut x2) = Default::default();
                Box::new(move |buffers, control| {
                    let center = center.next(control);
                    let quality = quality.next(control).max(1e-10);

                    // Restrict f0 for stability
                    let f0 = (center * control.sample_secs).max(0.0).min(0.5);
                    let (sin, cos) = (TAU * f0).sin_cos();
                    let alpha = sin / 2.0 / quality;

                    let b0 = 1.0;
                    let b1 = -2.0 * cos;
                    let b2 = 1.0;
                    let a0 = 1.0 + alpha;
                    let a1 = b1;
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
            FilterKind::AllPass { corner, quality } => {
                let mut corner = corner.clone();
                let mut quality = quality.clone();

                let (mut y1, mut y2, mut x1, mut x2) = Default::default();
                Box::new(move |buffers, control| {
                    let corner = corner.next(control);
                    let quality = quality.next(control).max(1e-10);

                    // Restrict f0 for stability
                    let f0 = (corner * control.sample_secs).max(0.0).min(0.5);
                    let (sin, cos) = (TAU * f0).sin_cos();
                    let alpha = sin / 2.0 / quality;

                    let b0 = 1.0 - alpha;
                    let b1 = -2.0 * cos;
                    let b2 = 1.0 + alpha;
                    let a0 = b2;
                    let a1 = b1;
                    let a2 = b0;

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
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct RingModulator<K> {
    pub sources: (Source, Source),
    pub destination: Destination<K>,
}

impl<C: Controller> RingModulator<C> {
    pub fn create_stage(&self) -> Stage<C::Storage> {
        let sources = self.sources.clone();
        let mut destination = self.destination.clone();
        Box::new(move |buffers, control| {
            buffers.write_1_read_2(&mut destination, &sources, control, |source_1, source_2| {
                source_1 * source_2
            })
        })
    }
}
