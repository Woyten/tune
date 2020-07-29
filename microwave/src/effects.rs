use std::f64::consts::PI;

/// Filter as described in https://en.wikipedia.org/wiki/Low-pass_filter#Discrete-time_realization.
#[derive(Default)]
pub struct LowPassFilter {
    out_buffer: f64,
}

impl LowPassFilter {
    pub fn advance_phase(&mut self, input: f64, d_phase: f64) {
        let alpha = 1.0 / (1.0 + (2.0 * PI * d_phase).recip());
        self.out_buffer += alpha * (input - self.out_buffer);
    }

    pub fn signal(&self) -> f64 {
        self.out_buffer
    }
}

/// Filter based on the differential equation d2out_dt2 = omega^2*input - out - omega*damping*dout_dt.
#[derive(Default)]
pub struct ResonanceFilter {
    out: f64,
    dout_dt: f64,
}

impl ResonanceFilter {
    pub fn advance_phase(&mut self, input: f64, damping: f64, mut d_phase: f64) {
        // Filter is unstable when d_phase is larger than a quarter period
        d_phase = d_phase.min(0.25);
        let d2out_dt2 = input - self.out - damping * self.dout_dt;
        self.dout_dt += d2out_dt2 * 2.0 * PI * d_phase;
        self.out += self.dout_dt * 2.0 * PI * d_phase;
    }

    pub fn signal(&self) -> f64 {
        self.out
    }
}

pub struct Delay {
    feedback: f32,
    feedback_rotation_radians: f32,
    buffer: Vec<(f32, f32)>,
    position: usize,
}

impl Delay {
    pub fn new(samples_per_buffer: usize, feedback: f32, feedback_rotation_radians: f32) -> Self {
        Self {
            feedback,
            feedback_rotation_radians,
            buffer: vec![(0.0, 0.0); samples_per_buffer],
            position: 0,
        }
    }

    pub fn mute(&mut self) {
        self.buffer.iter_mut().for_each(|e| *e = (0.0, 0.0))
    }

    pub fn process(&mut self, signal: &mut [f32]) {
        // A channel rotation of alpha degrees is perceived as a rotation of 2*alpha
        let (sin, cos) = (self.feedback_rotation_radians / 2.0).sin_cos();

        // A mathematically positive rotation around the l x r axis is perceived as a clockwise rotation
        let rot_l_l = cos * self.feedback;
        let rot_r_l = sin * self.feedback;
        let rot_l_r = -rot_r_l;
        let rot_r_r = rot_l_l;

        for signal_sample in signal.chunks_mut(2) {
            let delayed_sample = self.buffer.get_mut(self.position);

            if let ([signal_l, signal_r], Some((delayed_l, delayed_r))) =
                (signal_sample, delayed_sample)
            {
                *signal_l += rot_l_l * *delayed_l + rot_l_r * *delayed_r;
                *signal_r += rot_r_l * *delayed_l + rot_r_r * *delayed_r;

                *delayed_l = *signal_l;
                *delayed_r = *signal_r;
            }

            self.position += 1;
            self.position %= self.buffer.len();
        }
    }
}
