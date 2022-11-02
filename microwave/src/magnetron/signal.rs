use magnetron::{
    spec::{Creator, Spec},
    waveform::Stage,
};
use nannou::rand::prelude::*;
use serde::{Deserialize, Serialize};

use super::{AutomationSpec, OutSpec};

#[derive(Serialize, Deserialize)]
pub struct SignalSpec<A> {
    pub kind: SignalKind,
    #[serde(flatten)]
    pub out_spec: OutSpec<A>,
}

#[derive(Serialize, Deserialize)]
pub enum SignalKind {
    Noise,
}

impl<A: AutomationSpec> Spec for SignalSpec<A> {
    type Created = Stage<A::Context>;

    fn use_creator(&self, creator: &Creator) -> Self::Created {
        let out_buffer = self.out_spec.out_buffer.buffer();

        match self.kind {
            SignalKind::Noise => {
                let mut rng = SmallRng::from_entropy();
                creator.create_stage(&self.out_spec.out_level, move |buffers, out_level| {
                    buffers.read_0_and_write(out_buffer, out_level, || rng.gen_range(-1.0..1.0))
                })
            }
        }
    }
}
