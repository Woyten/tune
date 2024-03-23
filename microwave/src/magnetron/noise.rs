use magnetron::{buffer::BufferIndex, creator::Creator, stage::Stage};
use rand::prelude::*;
use serde::{Deserialize, Serialize};

use super::AutomationSpec;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NoiseSpec {
    #[serde(flatten)]
    pub noise_type: NoiseType,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "noise_type")]
pub enum NoiseType {
    White,
}

impl NoiseSpec {
    pub fn use_creator<A: AutomationSpec>(
        &self,
        creator: &Creator<A>,
        out_buffer: BufferIndex,
        out_level: Option<&A>,
    ) -> Stage<A> {
        match &self.noise_type {
            NoiseType::White => {
                let mut rng = SmallRng::from_entropy();
                creator.create_stage(out_level, move |buffers, out_level| {
                    buffers.read_0_write_1(out_buffer, out_level, || rng.gen_range(-1.0..1.0))
                })
            }
        }
    }
}
