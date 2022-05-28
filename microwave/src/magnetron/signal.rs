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

impl<C: Controller> Spec for &SignalSpec<C> {
    type Created = Stage<C::Storage>;

    fn use_creator(self, creator: &Creator) -> Self::Created {
        let mut output = creator.create(&self.out_spec);

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
