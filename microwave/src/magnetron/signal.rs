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
    pub fn create_stage(&self) -> Stage<C::Storage> {
        let mut output = self.out_spec.create_output();

        match self.kind {
            SignalKind::Noise => {
                let mut rng = SmallRng::from_entropy();
                Box::new(move |buffers, control| {
                    buffers.read_0_and_write(&mut output, control, || rng.gen_range(-1.0..1.0))
                })
            }
        }
    }
}
