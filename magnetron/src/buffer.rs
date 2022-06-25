use std::{iter, mem, sync::Arc};

pub struct BufferWriter {
    pub(crate) sample_width_secs: f64,
    pub(crate) readable: ReadableBuffers,
    pub(crate) writeable: WaveformBuffer,
}

impl BufferWriter {
    pub fn sample_width_secs(&self) -> f64 {
        self.sample_width_secs
    }

    pub fn read_0_and_write(
        &mut self,
        out_buffer: OutBuffer,
        out_level: f64,
        mut f: impl FnMut() -> f64,
    ) {
        self.read_n_and_write(out_buffer, |_, write_access| {
            write_access.write(iter::repeat_with(|| f() * out_level))
        });
    }

    pub fn read_1_and_write(
        &mut self,
        in_buffer: InBuffer,
        out_buffer: OutBuffer,
        out_level: f64,
        mut f: impl FnMut(f64) -> f64,
    ) {
        self.read_n_and_write(out_buffer, |read_access, write_access| {
            write_access.write(
                read_access
                    .read(in_buffer)
                    .iter()
                    .map(|&src| f(src) * out_level),
            )
        });
    }

    pub fn read_2_and_write(
        &mut self,
        in_buffers: (InBuffer, InBuffer),
        out_buffer: OutBuffer,
        out_level: f64,
        mut f: impl FnMut(f64, f64) -> f64,
    ) {
        self.read_n_and_write(out_buffer, |read_access, write_access| {
            write_access.write(
                read_access
                    .read(in_buffers.0)
                    .iter()
                    .zip(read_access.read(in_buffers.1))
                    .map(|(&src_0, &src_1)| f(src_0, src_1) * out_level),
            )
        });
    }

    fn read_n_and_write(
        &mut self,
        out_buffer: OutBuffer,
        mut rw_access_fn: impl FnMut(&ReadableBuffers, &mut WaveformBuffer),
    ) {
        self.readable.swap(out_buffer, &mut self.writeable);
        rw_access_fn(&self.readable, &mut self.writeable);
        self.readable.swap(out_buffer, &mut self.writeable);
    }
}

#[derive(Copy, Clone, Debug)]
pub enum InBuffer {
    Buffer(usize),
    AudioIn,
}

#[derive(Copy, Clone, Debug)]
pub enum OutBuffer {
    Buffer(usize),
    AudioOut,
}

pub(crate) struct ReadableBuffers {
    pub audio_in: WaveformBuffer,
    pub intermediate: Vec<WaveformBuffer>,
    pub audio_out: WaveformBuffer,
    pub mix: WaveformBuffer,
}

impl ReadableBuffers {
    fn swap(&mut self, buffer_a: OutBuffer, buffer_b: &mut WaveformBuffer) {
        let buffer_a = match buffer_a {
            OutBuffer::Buffer(index) => self.intermediate.get_mut(index).unwrap_or_else(|| {
                panic!(
                    "Index {} out of range. Please allocate more waveform buffers.",
                    index
                )
            }),
            OutBuffer::AudioOut => &mut self.audio_out,
        };
        mem::swap(buffer_a, buffer_b);
    }

    fn read(&self, in_buffer: InBuffer) -> &[f64] {
        match in_buffer {
            InBuffer::Buffer(index) => &self.intermediate[index],
            InBuffer::AudioIn => &self.audio_in,
        }
        .read()
    }
}

#[derive(Clone)]
pub(crate) struct WaveformBuffer {
    pub storage: Vec<f64>,
    pub len: usize,
    pub dirty: bool,
    pub zeros: Arc<[f64]>,
}

impl WaveformBuffer {
    pub fn new(zeros: Arc<[f64]>) -> Self {
        Self {
            storage: vec![0.0; zeros.len()],
            len: 0,
            dirty: false,
            zeros,
        }
    }

    pub fn clear(&mut self, len: usize) {
        self.len = len;
        self.dirty = true;
    }

    pub fn read(&self) -> &[f64] {
        match self.dirty {
            true => &self.zeros[..self.len],
            false => &self.storage[..self.len],
        }
    }

    pub fn write(&mut self, items: impl Iterator<Item = f64>) {
        match self.dirty {
            true => {
                for (dest, src) in self.storage[..self.len].iter_mut().zip(items) {
                    *dest = src
                }
                self.dirty = false;
            }
            false => {
                for (dest, src) in self.storage[..self.len].iter_mut().zip(items) {
                    *dest += src
                }
            }
        }
    }
}
