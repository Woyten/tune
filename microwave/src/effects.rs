use std::f64::consts::PI;

#[derive(Default)]
pub struct DifferentialFilter {
    in_buffer: (f64, f64),
    out_buffer: f64,
}

impl DifferentialFilter {
    pub fn advance_low_pass_phase(&mut self, d_phase: f64) {
        let alpha = 1.0 / (1.0 + 1.0 / 2.0 / PI / d_phase);
        self.out_buffer = self.out_buffer + alpha * (self.in_buffer.1 - self.out_buffer);
    }

    pub fn write_input(&mut self, input: f64) {
        self.in_buffer = (self.in_buffer.1, input);
    }

    pub fn signal(&self) -> f64 {
        self.out_buffer
    }
}

pub struct Delay {
    buffer: Vec<f32>,
    feedback: f32,
    position: usize,
}

impl Delay {
    pub fn new(buffer_size: usize, feedback: f32) -> Self {
        Self {
            buffer: vec![0.0; buffer_size],
            feedback,
            position: 0,
        }
    }

    pub fn process(&mut self, signal: &mut [f32]) {
        for sample in signal {
            self.position += 1;
            self.position %= self.buffer.len();
            let delayed_sample = &mut self.buffer[self.position];
            *sample += *delayed_sample * self.feedback;
            *delayed_sample = *sample;
        }
    }
}
