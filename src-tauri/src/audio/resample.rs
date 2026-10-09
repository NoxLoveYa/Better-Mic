/// Streaming linear-interpolation resampler. `step` is input samples consumed per output sample
/// and can be nudged at runtime to absorb clock drift between devices.
pub struct Resampler {
    pub step: f64,
    pos: f64,
    prev: f32,
}

impl Resampler {
    pub fn new(in_rate: f64, out_rate: f64) -> Self {
        Self { step: in_rate / out_rate, pos: 0.0, prev: 0.0 }
    }

    pub fn push(&mut self, input: &[f32], mut emit: impl FnMut(f32)) {
        for &x in input {
            while self.pos < 1.0 {
                emit(self.prev + (x - self.prev) * self.pos as f32);
                self.pos += self.step;
            }
            self.pos -= 1.0;
            self.prev = x;
        }
    }
}
