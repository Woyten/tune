use magnetron::buffer::BufferIndex;
use magnetron::buffer::BufferWriter;
use ringbuf::HeapProd;
use ringbuf::traits::Observer;
use ringbuf::traits::Producer;

// A read-only observer that runs after all pipeline stages have processed a render window.
// Unlike a stage, a tap cannot write to pipeline buffers and is not part of the
// profile-controlled stage ordering. Taps are intended for sinks such as visualization or
// metering.
pub(crate) trait Tap: Send {
    fn process(&mut self, context: &TapContext<'_>);
}

#[derive(Default)]
pub(crate) struct TapSet {
    taps: Vec<Box<dyn Tap>>,
}

impl TapSet {
    pub(crate) fn push(&mut self, tap: impl Tap + 'static) {
        self.taps.push(Box::new(tap));
    }

    pub(crate) fn process(&mut self, context: &TapContext<'_>) {
        for tap in &mut self.taps {
            tap.process(context);
        }
    }
}

pub(crate) struct TapContext<'a> {
    buffers: BufferWriter<'a>,
}

impl<'a> TapContext<'a> {
    pub(crate) fn new(buffers: BufferWriter<'a>) -> Self {
        Self { buffers }
    }

    pub(crate) fn read(&self, buffer: BufferIndex) -> &[f64] {
        self.buffers.read(buffer)
    }
}

// Messages used by lossy audio capture rings. Start and End are only used by recorders.
pub(crate) enum AudioRingMessage {
    Frame { left: f32, right: f32 },
    Start,
    End,
    Overflow,
}

pub(crate) struct AudioRingProducer {
    producer: HeapProd<AudioRingMessage>,
    pending_overflow: bool,
}

impl AudioRingProducer {
    pub(crate) fn new(producer: HeapProd<AudioRingMessage>) -> Self {
        Self {
            producer,
            pending_overflow: false,
        }
    }

    pub(crate) fn push_frame(&mut self, left: f32, right: f32) -> bool {
        if !self.flush_overflow() {
            return false;
        }

        // Keep one slot available for the overflow marker.
        if self.producer.vacant_len() <= 1 {
            self.pending_overflow = true;
            let _ = self.flush_overflow();
            return false;
        }

        if self
            .producer
            .try_push(AudioRingMessage::Frame { left, right })
            .is_err()
        {
            self.pending_overflow = true;
            let _ = self.flush_overflow();
            return false;
        }

        true
    }

    pub(crate) fn try_push_control(&mut self, message: AudioRingMessage) -> bool {
        self.producer.try_push(message).is_ok()
    }

    pub(crate) fn flush_overflow(&mut self) -> bool {
        if !self.pending_overflow {
            return true;
        }

        if self.producer.try_push(AudioRingMessage::Overflow).is_err() {
            return false;
        }

        self.pending_overflow = false;
        true
    }
}
