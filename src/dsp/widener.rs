//! Stereo widener via Mid/Side processing with optional mono-bass below a
//! cutoff. Classic approach:
//!
//!   M = (L + R) / 2
//!   S = (L - R) / 2
//!   S' = S * width
//!   L' = M + S'
//!   R' = M - S'
//!
//! `width = 0` collapses to mono, `width = 1` is unity, `width > 1` pushes
//! the sides out further. To avoid bass mud, a first-order crossover keeps
//! low frequencies centred regardless of the width setting.

use crate::dsp::filters::OnePoleLP;

pub struct StereoWidener {
    sample_rate: f32,
    width: f32,
    mono_bass_hz: f32,
    mono_bass_enabled: bool,

    lp_l: OnePoleLP,
    lp_r: OnePoleLP,
}

impl StereoWidener {
    pub fn new(sample_rate: f32) -> Self {
        let mut w = Self {
            sample_rate,
            width: 1.0,
            mono_bass_hz: 120.0,
            mono_bass_enabled: true,
            lp_l: OnePoleLP::new(),
            lp_r: OnePoleLP::new(),
        };
        w.lp_l.set_freq(w.mono_bass_hz, sample_rate);
        w.lp_r.set_freq(w.mono_bass_hz, sample_rate);
        w
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sample_rate = sr;
        self.lp_l.set_freq(self.mono_bass_hz, sr);
        self.lp_r.set_freq(self.mono_bass_hz, sr);
    }

    pub fn reset(&mut self) {
        self.lp_l.reset();
        self.lp_r.reset();
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = width.clamp(0.0, 4.0);
    }

    pub fn set_mono_bass(&mut self, hz: f32, enabled: bool) {
        self.mono_bass_hz = hz.clamp(20.0, 500.0);
        self.mono_bass_enabled = enabled;
        self.lp_l.set_freq(self.mono_bass_hz, self.sample_rate);
        self.lp_r.set_freq(self.mono_bass_hz, self.sample_rate);
    }

    /// Process one stereo sample pair in-place.
    #[inline]
    pub fn process(&mut self, l: &mut f32, r: &mut f32) {
        let (lo_l, lo_r, hi_l, hi_r) = if self.mono_bass_enabled {
            let lo_l = self.lp_l.process(*l);
            let lo_r = self.lp_r.process(*r);
            (lo_l, lo_r, *l - lo_l, *r - lo_r)
        } else {
            (0.0, 0.0, *l, *r)
        };

        let mid = 0.5 * (hi_l + hi_r);
        let side = 0.5 * (hi_l - hi_r) * self.width;
        let wide_l = mid + side;
        let wide_r = mid - side;

        let mono_low = 0.5 * (lo_l + lo_r);

        *l = wide_l + mono_low;
        *r = wide_r + mono_low;
    }
}
