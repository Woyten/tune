use nannou::rand::prelude::*;
use serde::{Deserialize, Serialize};

use super::{
    control::Controller,
    waveform::{Creator, OutSpec, Spec, Stage},
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

impl<C: Controller> Spec for SignalSpec<C> {
    type Created = Stage<C>;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        let out_buffer = self.out_spec.out_buffer.clone();

        match self.kind {
            SignalKind::Noise => {
                let mut rng = SmallRng::from_entropy();
                creator.create_stage(&self.out_spec.out_level, move |buffers, out_level| {
                    buffers.read_0_and_write(&out_buffer, out_level, || rng.gen_range(-1.0..1.0))
                })
            }
        }
    }
}
