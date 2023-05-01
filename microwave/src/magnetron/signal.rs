use magnetron::{buffer::BufferIndex, creator::Creator, stage::Stage};
use rand::prelude::*;
use serde::{Deserialize, Serialize};

use super::{AutomationSpec, OutSpec};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SignalSpec<A> {
    pub kind: SignalKind,
    #[serde(flatten)]
    pub out_spec: OutSpec<A>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum SignalKind {
    Noise,
}

impl<A: AutomationSpec> SignalSpec<A> {
    pub fn use_creator(&self, creator: &Creator<A>) -> Stage<A::Context> {
        let out_buffer = BufferIndex::Internal(self.out_spec.out_buffer);

        match self.kind {
            SignalKind::Noise => {
                let mut rng = SmallRng::from_entropy();
                creator.create_stage(&self.out_spec.out_level, move |buffers, out_level| {
                    buffers.read_0_write_1(out_buffer, out_level, || rng.gen_range(-1.0..1.0))
                })
            }
        }
    }
}
