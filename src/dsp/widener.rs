//! Stereo widener via Mid/Side processing with optional mono-bass below a
//! cutoff. Classic approach, but split through a phase-coherent
//! Linkwitz-Riley 4th-order crossover so the low/high recombine cleanly.
//!
//!   M = (L + R) * 0.5
//!   S = (L - R) * 0.5
//!   S' = S * width
//!   L' = M + S'
//!   R' = M - S'
//!
//! `width = 0` collapses to mono, `width = 1` is unity, `width > 1` pushes
//! the sides further out. To avoid bass mud (and avoid phase issues caused
//! by widening sub-bass), the LR4 crossover keeps low frequencies centred
//! regardless of the width setting.

use crate::dsp::filters::LinkwitzRiley4;

/// Sliding window length in samples for the correlation estimate. ~50 ms at
/// 48 kHz — short enough that the meter feels live, long enough to filter
/// out per-sample noise.
const CORR_WINDOW_SAMPLES: usize = 2400;

pub struct StereoWidener {
    sample_rate: f32,
    width: f32,
    mono_bass_hz: f32,
    mono_bass_enabled: bool,

    xover_l: LinkwitzRiley4,
    xover_r: LinkwitzRiley4,

    // Sliding-window stereo correlation accumulators.
    // We keep running sums of L*R, L*L, R*R over a fixed-length window
    // by storing the per-sample products in a ring buffer.
    corr_lr: f32,
    corr_ll: f32,
    corr_rr: f32,
    corr_ring_lr: Box<[f32]>,
    corr_ring_ll: Box<[f32]>,
    corr_ring_rr: Box<[f32]>,
    corr_idx: usize,
    last_correlation: f32,
}

impl StereoWidener {
    pub fn new(sample_rate: f32) -> Self {
        let mut w = Self {
            sample_rate,
            width: 1.0,
            mono_bass_hz: 120.0,
            mono_bass_enabled: true,
            xover_l: LinkwitzRiley4::new(),
            xover_r: LinkwitzRiley4::new(),
            corr_lr: 0.0,
            corr_ll: 0.0,
            corr_rr: 0.0,
            corr_ring_lr: vec![0.0; CORR_WINDOW_SAMPLES].into_boxed_slice(),
            corr_ring_ll: vec![0.0; CORR_WINDOW_SAMPLES].into_boxed_slice(),
            corr_ring_rr: vec![0.0; CORR_WINDOW_SAMPLES].into_boxed_slice(),
            corr_idx: 0,
            last_correlation: 1.0,
        };
        w.xover_l.set_freq(w.mono_bass_hz, sample_rate);
        w.xover_r.set_freq(w.mono_bass_hz, sample_rate);
        w
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sample_rate = sr;
        self.xover_l.set_freq(self.mono_bass_hz, sr);
        self.xover_r.set_freq(self.mono_bass_hz, sr);
    }

    pub fn reset(&mut self) {
        self.xover_l.reset();
        self.xover_r.reset();
        self.corr_lr = 0.0;
        self.corr_ll = 0.0;
        self.corr_rr = 0.0;
        for s in self.corr_ring_lr.iter_mut() { *s = 0.0; }
        for s in self.corr_ring_ll.iter_mut() { *s = 0.0; }
        for s in self.corr_ring_rr.iter_mut() { *s = 0.0; }
        self.corr_idx = 0;
        self.last_correlation = 1.0;
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = width.clamp(0.0, 4.0);
    }

    pub fn set_mono_bass(&mut self, hz: f32, enabled: bool) {
        let new_hz = hz.clamp(20.0, 500.0);
        if (new_hz - self.mono_bass_hz).abs() > 0.01 {
            self.mono_bass_hz = new_hz;
            self.xover_l.set_freq(new_hz, self.sample_rate);
            self.xover_r.set_freq(new_hz, self.sample_rate);
        }
        self.mono_bass_enabled = enabled;
    }

    /// Latest sliding-window stereo correlation, range [-1, +1].
    /// `+1` = identical channels (mono), `0` = uncorrelated,
    /// `-1` = inverted polarity — the one to watch for.
    #[inline]
    pub fn correlation(&self) -> f32 {
        self.last_correlation
    }

    /// Process one stereo sample pair in-place.
    #[inline]
    pub fn process(&mut self, l: &mut f32, r: &mut f32) {
        let in_l = *l;
        let in_r = *r;

        // Phase-coherent split via LR4. lo + hi == in (allpass), so we can
        // process them independently and sum back without notches.
        let (lo_l, hi_l, lo_r, hi_r) = if self.mono_bass_enabled {
            let (ll, hl) = self.xover_l.process(in_l);
            let (lr, hr) = self.xover_r.process(in_r);
            (ll, hl, lr, hr)
        } else {
            (0.0, in_l, 0.0, in_r)
        };

        // M/S widening on the HIGH band only.
        let mid = 0.5 * (hi_l + hi_r);
        let side = 0.5 * (hi_l - hi_r) * self.width;
        let wide_l = mid + side;
        let wide_r = mid - side;

        // Mono sum on the LOW band.
        let mono_low = 0.5 * (lo_l + lo_r);

        let out_l = wide_l + mono_low;
        let out_r = wide_r + mono_low;

        *l = out_l;
        *r = out_r;

        // Sliding-window correlation update.
        let idx = self.corr_idx;
        let new_lr = out_l * out_r;
        let new_ll = out_l * out_l;
        let new_rr = out_r * out_r;
        self.corr_lr += new_lr - self.corr_ring_lr[idx];
        self.corr_ll += new_ll - self.corr_ring_ll[idx];
        self.corr_rr += new_rr - self.corr_ring_rr[idx];
        self.corr_ring_lr[idx] = new_lr;
        self.corr_ring_ll[idx] = new_ll;
        self.corr_ring_rr[idx] = new_rr;
        self.corr_idx = (idx + 1) % CORR_WINDOW_SAMPLES;
    }

    /// Recompute the correlation estimate from the sliding-window sums.
    /// Cheap; safe to call once per audio block (not per sample).
    pub fn finalize_correlation(&mut self) {
        let denom = (self.corr_ll * self.corr_rr).sqrt();
        self.last_correlation = if denom > 1e-12 {
            (self.corr_lr / denom).clamp(-1.0, 1.0)
        } else {
            // Below the noise floor — treat as fully correlated (mono).
            1.0
        };
    }
}
