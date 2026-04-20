//! Simple filter primitives used by the widener's mono-bass crossover.

use std::f32::consts::PI;

/// One-pole lowpass for damping / smoothing.
pub struct OnePoleLP {
    coeff: f32,
    state: f32,
}

impl OnePoleLP {
    pub fn new() -> Self {
        Self {
            coeff: 0.5,
            state: 0.0,
        }
    }

    pub fn set_freq(&mut self, freq: f32, sr: f32) {
        let w = (2.0 * PI * freq / sr).min(PI - 0.01);
        self.coeff = w.sin() / (1.0 + w.cos());
    }

    pub fn process(&mut self, input: f32) -> f32 {
        self.state += self.coeff * (input - self.state);
        self.state
    }

    pub fn reset(&mut self) {
        self.state = 0.0;
    }
}
