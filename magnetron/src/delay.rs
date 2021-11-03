use crate::interpolate::Interpolate;

pub struct DelayLine<T = f64> {
    buffer: Vec<T>,
    position: usize,
}

impl<T: Copy + Default> DelayLine<T> {
    pub fn new(num_skip_back_samples: usize) -> Self {
        Self {
            buffer: vec![Default::default(); num_skip_back_samples + 1],
            position: 0,
        }
    }

    pub fn buffer_len(&self) -> usize {
        self.buffer.len()
    }

    pub fn mute(&mut self) {
        self.buffer.iter_mut().for_each(|e| *e = Default::default());
    }

    pub fn write(&mut self, sample: T) {
        if let Some(value) = self.buffer.get_mut(self.position) {
            *value = sample
        }
    }

    pub fn advance(&mut self) {
        self.position = (self.position + 1) % self.buffer.len();
    }

    pub fn get_delayed(&self) -> T {
        self.get(self.position + 1)
    }

    pub fn get_delayed_fract(&self, fract_offset: f64) -> T
    where
        T: Interpolate,
    {
        let offset = (self.buffer.len() - 1) as f64 * fract_offset;
        let interpolation = offset.ceil() - offset;

        let position = self.position + self.buffer.len() - offset.ceil() as usize;
        let delayed_1 = self.get(position);
        let delayed_2 = self.get(position + 1);

        T::interpolate(delayed_1, delayed_2, interpolation)
    }

    fn get(&self, position: usize) -> T {
        self.buffer
            .get(position % self.buffer.len())
            .copied()
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use assert_approx_eq::assert_approx_eq;

    use super::*;

    #[test]
    fn delay_line() {
        let mut delay_line = DelayLine::new(5);
        delay_line.write(1.0);
        delay_line.advance();
        delay_line.write(2.0);
        delay_line.advance();
        delay_line.write(4.0);
        delay_line.advance();
        delay_line.write(8.0);
        delay_line.advance();
        delay_line.write(16.0);
        delay_line.advance();
        delay_line.write(32.0);

        assert_approx_eq!(delay_line.get_delayed_fract(1.0), 1.0);
        assert_approx_eq!(delay_line.get_delayed_fract(0.9), 1.5);
        assert_approx_eq!(delay_line.get_delayed_fract(0.8), 2.0);
        assert_approx_eq!(delay_line.get_delayed_fract(0.7), 3.0);
        assert_approx_eq!(delay_line.get_delayed_fract(0.6), 4.0);
        assert_approx_eq!(delay_line.get_delayed_fract(0.5), 6.0);
        assert_approx_eq!(delay_line.get_delayed_fract(0.4), 8.0);
        assert_approx_eq!(delay_line.get_delayed_fract(0.3), 12.0);
        assert_approx_eq!(delay_line.get_delayed_fract(0.2), 16.0);
        assert_approx_eq!(delay_line.get_delayed_fract(0.1), 24.0);
        assert_approx_eq!(delay_line.get_delayed_fract(0.0), 32.0);
        assert_approx_eq!(delay_line.get_delayed(), 1.0);
    }
}
