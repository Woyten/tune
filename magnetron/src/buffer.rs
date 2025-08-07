use std::iter;
use std::mem;

use crate::automation::QueryInfo;
use crate::stage::Stage;
use crate::stage::StageActivity;
use crate::Magnetron;

pub struct BufferWriter<'a> {
    sample_width_secs: f64,
    render_window_secs: f64,
    buffers: Buffers<'a>,
    zeros: &'a [f64],
    reset: bool,
}

impl<'a> BufferWriter<'a> {
    pub(crate) fn new(
        magnetron: &'a mut Magnetron,
        num_samples: usize,
        external_buffers: &'a mut [WaveformBuffer],
        reset: bool,
    ) -> Self {
        Self {
            sample_width_secs: magnetron.sample_width_secs,
            render_window_secs: magnetron.sample_width_secs * num_samples as f64,
            buffers: Buffers {
                external_buffers,
                internal_buffers: &mut magnetron.buffers,
            },
            zeros: &magnetron.zeros[..num_samples],
            reset,
        }
    }

    pub fn buffer_len(&self) -> usize {
        self.zeros.len()
    }

    pub fn sample_width_secs(&self) -> f64 {
        self.sample_width_secs
    }

    pub fn render_window_secs(&self) -> f64 {
        self.render_window_secs
    }

    pub fn reset(&self) -> bool {
        self.reset
    }

    pub fn set_reset(&mut self) {
        self.reset = true;
    }

    pub fn process<'ctx, Q: QueryInfo + 'ctx>(
        &mut self,
        context: Q::Context<'_>,
        stages: impl IntoIterator<Item = &'ctx mut Stage<Q>>,
    ) -> StageActivity {
        stages
            .into_iter()
            .map(|stage| stage.process(self, context))
            .max()
            .unwrap_or_default()
    }

    pub fn read(&self, in_buffer: BufferIndex) -> &[f64] {
        self.buffers.get(in_buffer, self.zeros)
    }

    pub fn read_0_write_1(
        &mut self,
        out_buffer: BufferIndex,
        out_level: Option<f64>,
        mut f: impl FnMut() -> f64,
    ) -> StageActivity {
        let out_level = out_level.unwrap_or(1.0);
        self.buffers.read_n_write_1(out_buffer, |_, out_buffer| {
            write_1(
                out_buffer,
                self.zeros,
                iter::repeat_with(|| f() * out_level),
            )
        })
    }

    pub fn read_1_write_1(
        &mut self,
        in_buffer: BufferIndex,
        out_buffer: BufferIndex,
        out_level: Option<f64>,
        mut f: impl FnMut(f64) -> f64,
    ) -> StageActivity {
        let out_level = out_level.unwrap_or(1.0);
        self.buffers
            .read_n_write_1(out_buffer, |buffers, out_buffer| {
                write_1(
                    out_buffer,
                    self.zeros,
                    buffers
                        .get(in_buffer, self.zeros)
                        .iter()
                        .map(|&src| f(src) * out_level),
                )
            })
    }

    pub fn read_2_write_1(
        &mut self,
        in_buffers: (BufferIndex, BufferIndex),
        out_buffer: BufferIndex,
        out_level: Option<f64>,
        mut f: impl FnMut(f64, f64) -> f64,
    ) -> StageActivity {
        let out_level = out_level.unwrap_or(1.0);
        self.buffers
            .read_n_write_1(out_buffer, |buffers, out_buffer| {
                write_1(
                    out_buffer,
                    self.zeros,
                    buffers
                        .get(in_buffers.0, self.zeros)
                        .iter()
                        .zip(buffers.get(in_buffers.1, self.zeros))
                        .map(|(&src_0, &src_1)| f(src_0, src_1) * out_level),
                )
            })
    }

    pub fn read_0_write_2(
        &mut self,
        out_buffers: (BufferIndex, BufferIndex),
        out_levels: Option<(f64, f64)>,
        mut f: impl FnMut() -> (f64, f64),
    ) -> StageActivity {
        let out_levels = out_levels.unwrap_or((1.0, 1.0));
        self.buffers.read_n_write_2(out_buffers, |_, out_buffers| {
            write_2(
                out_buffers,
                iter::repeat_with(|| {
                    let sample = f();
                    (sample.0 * out_levels.0, sample.1 * out_levels.1)
                }),
                self.zeros,
            )
        })
    }

    pub fn read_1_write_2(
        &mut self,
        in_buffer: BufferIndex,
        out_buffers: (BufferIndex, BufferIndex),
        out_levels: Option<(f64, f64)>,
        mut f: impl FnMut(f64) -> (f64, f64),
    ) -> StageActivity {
        let out_levels = out_levels.unwrap_or((1.0, 1.0));
        self.buffers
            .read_n_write_2(out_buffers, |buffers, out_buffers| {
                write_2(
                    out_buffers,
                    buffers.get(in_buffer, self.zeros).iter().map(|&src| {
                        let sample = f(src);
                        (sample.0 * out_levels.0, sample.1 * out_levels.1)
                    }),
                    self.zeros,
                )
            })
    }

    pub fn read_2_write_2(
        &mut self,
        in_buffers: (BufferIndex, BufferIndex),
        out_buffers: (BufferIndex, BufferIndex),
        out_levels: Option<(f64, f64)>,
        mut f: impl FnMut(f64, f64) -> (f64, f64),
    ) -> StageActivity {
        let out_levels = out_levels.unwrap_or((1.0, 1.0));
        self.buffers
            .read_n_write_2(out_buffers, |buffers, out_buffers| {
                write_2(
                    out_buffers,
                    buffers
                        .get(in_buffers.0, self.zeros)
                        .iter()
                        .zip(buffers.get(in_buffers.1, self.zeros))
                        .map(|(&src_0, &src_1)| {
                            let sample = f(src_0, src_1);
                            (sample.0 * out_levels.0, sample.1 * out_levels.1)
                        }),
                    self.zeros,
                )
            })
    }

    pub(crate) fn internal_buffers(&mut self) -> &mut [WaveformBuffer] {
        self.buffers.internal_buffers
    }
}

#[derive(Copy, Clone, Debug)]
pub enum BufferIndex {
    External(usize),
    Internal(usize),
}

impl BufferIndex {
    fn stage_activity(self) -> StageActivity {
        match self {
            BufferIndex::External(_) => StageActivity::External,
            BufferIndex::Internal(_) => StageActivity::Internal,
        }
    }
}

struct Buffers<'a> {
    external_buffers: &'a mut [WaveformBuffer],
    internal_buffers: &'a mut [WaveformBuffer],
}

impl Buffers<'_> {
    fn read_n_write_1(
        &mut self,
        out_buffer: BufferIndex,
        mut rw_access_fn: impl FnMut(&Buffers, &mut WaveformBuffer),
    ) -> StageActivity {
        let mut writeable = self.get_mut(out_buffer).take();
        rw_access_fn(self, &mut writeable);
        *self.get_mut(out_buffer) = writeable;

        out_buffer.stage_activity()
    }

    fn read_n_write_2(
        &mut self,
        out_buffers: (BufferIndex, BufferIndex),
        mut rw_access_fn: impl FnMut(&Buffers, &mut (WaveformBuffer, WaveformBuffer)),
    ) -> StageActivity {
        let mut writeable = (
            self.get_mut(out_buffers.0).take(),
            self.get_mut(out_buffers.1).take(),
        );
        rw_access_fn(self, &mut writeable);
        (*self.get_mut(out_buffers.0), *self.get_mut(out_buffers.1)) = writeable;

        out_buffers
            .0
            .stage_activity()
            .max(out_buffers.1.stage_activity())
    }

    fn get<'a>(&'a self, buffer: BufferIndex, zeros: &'a [f64]) -> &'a [f64] {
        match buffer {
            BufferIndex::Internal(index) => &self.internal_buffers[index],
            BufferIndex::External(index) => &self.external_buffers[index],
        }
        .read(zeros)
    }

    fn get_mut(&mut self, buffer: BufferIndex) -> &mut WaveformBuffer {
        match buffer {
            BufferIndex::Internal(index) => &mut self.internal_buffers[index],
            BufferIndex::External(index) => &mut self.external_buffers[index],
        }
    }
}

#[derive(Clone)]
pub(crate) struct WaveformBuffer {
    dirty: bool,
    storage: Vec<f64>,
}

impl WaveformBuffer {
    pub(crate) fn new(buffer_size: usize) -> Self {
        Self {
            dirty: false,
            storage: vec![0.0; buffer_size],
        }
    }

    pub(crate) fn set_dirty(&mut self) {
        self.dirty = true;
    }

    fn take(&mut self) -> Self {
        Self {
            storage: mem::take(&mut self.storage),
            ..*self
        }
    }

    pub(crate) fn read<'a>(&'a self, zeros: &'a [f64]) -> &'a [f64] {
        match self.dirty {
            true => zeros,
            false => &self.storage[..zeros.len()],
        }
    }
}

fn write_1(out_buffer: &mut WaveformBuffer, zeros: &[f64], items: impl Iterator<Item = f64>) {
    let iterator = out_buffer.storage[..zeros.len()].iter_mut().zip(items);
    match out_buffer.dirty {
        true => {
            for (dest, src) in iterator {
                *dest = src
            }
        }
        false => {
            for (dest, src) in iterator {
                *dest += src
            }
        }
    }
    out_buffer.dirty = false;
}

fn write_2(
    out_buffers: &mut (WaveformBuffer, WaveformBuffer),
    items: impl Iterator<Item = (f64, f64)>,
    zeros: &[f64],
) {
    let iterator = iter::zip(
        &mut out_buffers.0.storage[..zeros.len()],
        &mut out_buffers.1.storage[..zeros.len()],
    )
    .zip(items);

    match (out_buffers.0.dirty, out_buffers.1.dirty) {
        (true, true) => {
            for (dest, src) in iterator {
                *dest.0 = src.0;
                *dest.1 = src.1;
            }
        }
        (true, false) => {
            for (dest, src) in iterator {
                *dest.0 = src.0;
                *dest.1 += src.1;
            }
        }
        (false, true) => {
            for (dest, src) in iterator {
                *dest.0 += src.0;
                *dest.1 = src.1;
            }
        }
        (false, false) => {
            for (dest, src) in iterator {
                *dest.0 += src.0;
                *dest.1 += src.1;
            }
        }
    }
    (out_buffers.0.dirty, out_buffers.1.dirty) = (false, false)
}
