//! Filter primitives used by WideBoi's mono-bass crossover.
//!
//! v0.2 replaces the v0.1 one-pole (6 dB/oct) with a Linkwitz-Riley 4th-order
//! pair (24 dB/oct, phase-coherent low + high outputs). LR4 = two cascaded
//! Butterworth 2nd-order biquads per leg. The low and high outputs are
//! defined so that low + high reconstructs the input with only an allpass
//! phase shift — perfect for split-then-recombine processing where you don't
//! want notches at the crossover frequency.

use std::f32::consts::PI;

/// One-pole lowpass — kept for any future slow smoothing needs (e.g. UI
/// metering decay). No longer used for the audio crossover.
#[allow(dead_code)]
pub struct OnePoleLP {
    coeff: f32,
    state: f32,
}

#[allow(dead_code)]
impl OnePoleLP {
    pub fn new() -> Self {
        Self { coeff: 0.5, state: 0.0 }
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

/// Single Butterworth 2nd-order biquad (Direct Form I). Two of these cascaded
/// at the same cutoff give a Linkwitz-Riley 4th-order leg.
#[derive(Clone, Default)]
struct Biquad {
    b0: f32, b1: f32, b2: f32,
    a1: f32, a2: f32,
    x1: f32, x2: f32,
    y1: f32, y2: f32,
}

impl Biquad {
    fn reset(&mut self) {
        self.x1 = 0.0; self.x2 = 0.0;
        self.y1 = 0.0; self.y2 = 0.0;
    }

    #[inline]
    fn process(&mut self, x: f32) -> f32 {
        let y = self.b0 * x + self.b1 * self.x1 + self.b2 * self.x2
              - self.a1 * self.y1 - self.a2 * self.y2;
        self.x2 = self.x1; self.x1 = x;
        self.y2 = self.y1; self.y1 = y;
        y
    }
}

/// Linkwitz-Riley 4th-order crossover. Single instance produces both the
/// lowpass and highpass output of one channel. Two cascaded Butterworth
/// 2nd-orders per leg → 24 dB/octave, -6 dB at cutoff, low + high recombines
/// to allpass.
pub struct LinkwitzRiley4 {
    lp_a: Biquad,
    lp_b: Biquad,
    hp_a: Biquad,
    hp_b: Biquad,
}

impl LinkwitzRiley4 {
    pub fn new() -> Self {
        Self {
            lp_a: Biquad::default(),
            lp_b: Biquad::default(),
            hp_a: Biquad::default(),
            hp_b: Biquad::default(),
        }
    }

    pub fn reset(&mut self) {
        self.lp_a.reset();
        self.lp_b.reset();
        self.hp_a.reset();
        self.hp_b.reset();
    }

    /// Update the cutoff frequency (Hz) at the given sample rate. Recomputes
    /// the biquad coefficients but does NOT zero the delay lines — for slow
    /// param updates that's the desired behavior; for big jumps the caller
    /// can also call `reset()`.
    pub fn set_freq(&mut self, freq_hz: f32, sample_rate: f32) {
        // Standard Butterworth lowpass / highpass biquad coefficients.
        // Q = 1/sqrt(2) for Butterworth response.
        let nyq = sample_rate * 0.5;
        let f = freq_hz.clamp(20.0, nyq - 100.0);
        let w0 = 2.0 * PI * f / sample_rate;
        let cos_w = w0.cos();
        let sin_w = w0.sin();
        let q = std::f32::consts::FRAC_1_SQRT_2; // Butterworth Q
        let alpha = sin_w / (2.0 * q);

        let a0 = 1.0 + alpha;
        let inv_a0 = 1.0 / a0;
        let a1 = -2.0 * cos_w * inv_a0;
        let a2 = (1.0 - alpha) * inv_a0;

        // Lowpass coefficients
        let lp_b0 = ((1.0 - cos_w) * 0.5) * inv_a0;
        let lp_b1 = (1.0 - cos_w) * inv_a0;
        let lp_b2 = lp_b0;

        // Highpass coefficients
        let hp_b0 = ((1.0 + cos_w) * 0.5) * inv_a0;
        let hp_b1 = -(1.0 + cos_w) * inv_a0;
        let hp_b2 = hp_b0;

        for bq in [&mut self.lp_a, &mut self.lp_b] {
            bq.b0 = lp_b0; bq.b1 = lp_b1; bq.b2 = lp_b2;
            bq.a1 = a1; bq.a2 = a2;
        }
        for bq in [&mut self.hp_a, &mut self.hp_b] {
            bq.b0 = hp_b0; bq.b1 = hp_b1; bq.b2 = hp_b2;
            bq.a1 = a1; bq.a2 = a2;
        }
    }

    /// Process one input sample and return `(low, high)`. By LR4 design,
    /// `low + high` is allpass — i.e. recombines back to the original input
    /// with no magnitude notch at the crossover.
    #[inline]
    pub fn process(&mut self, x: f32) -> (f32, f32) {
        let low = self.lp_b.process(self.lp_a.process(x));
        let high = self.hp_b.process(self.hp_a.process(x));
        (low, high)
    }
}
