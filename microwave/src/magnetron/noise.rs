use magnetron::{
    automation::{AutomatableParam, Automated, AutomationFactory},
    buffer::BufferIndex,
    stage::Stage,
};
use rand::prelude::*;
use serde::{Deserialize, Serialize};

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
    pub fn create<A: AutomatableParam>(
        &self,
        factory: &mut AutomationFactory<A>,
        out_buffer: BufferIndex,
        out_level: Option<&A>,
    ) -> Stage<A> {
        match &self.noise_type {
            NoiseType::White => {
                let mut rng = SmallRng::seed_from_u64(0);
                factory
                    .automate(out_level)
                    .into_stage(move |buffers, out_level| {
                        buffers
                            .read_0_write_1(out_buffer, out_level, || rng.random_range(-1.0..1.0))
                    })
            }
        }
    }
}
