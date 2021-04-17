use nannou::rand::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    control::Controller,
    waveform::{OutSpec, Stage},
};

#[derive(Serialize, Deserialize)]
pub struct SignalSpec<C> {
    pub kind: SignalKind,
    #[serde(flatten)]
    pub out_spec: OutSpec<C>,
}

#[derive(Serialize, Deserialize)]
pub enum SignalKind {
    Noise,
}

impl<C: Controller> SignalSpec<C> {
    /// A typical Perlin noise implementation like in https://www.arendpeter.com/Perlin_Noise.html.
    pub fn create_stage(&self) -> Stage<C::Storage> {
        let mut out_spec = self.out_spec.clone();

        match self.kind {
            SignalKind::Noise => {
                let mut rng = SmallRng::from_entropy();
                Box::new(move |buffers, control| {
                    buffers.read_0_and_write(&mut out_spec, control, || rng.gen_range(-1.0, 1.0))
                })
            }
        }
    }
}
