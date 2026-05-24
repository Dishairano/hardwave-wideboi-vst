//! Multiband stereo widener — independent M/S width on three frequency
//! bands (low / mid / high), with phase-coherent Linkwitz-Riley crossovers
//! so the bands recombine without magnitude notches.
//!
//! ## Band split topology
//!
//! A naive cascaded 3-way LR4 has a phase problem: the low band passes
//! through ONE crossover while mid+high pass through TWO, so when you sum
//! them back the low band is phase-misaligned with the rest and you get a
//! dip at the upper crossover. The fix used here is the standard
//! allpass-compensation trick:
//!
//!   (low_raw, rest) = xover_lo.process(x)        // split at f_lo
//!   (mid,     high) = xover_hi.process(rest)     // split rest at f_hi
//!   low             = allpass_hi(low_raw)         // realign low to xover_hi
//!
//! An LR4's `low + high` output is itself an allpass of the input, so we
//! get the f_hi allpass for free by running `low_raw` through a third LR4
//! tuned to f_hi and summing its two outputs. low + mid + high then
//! reconstructs the input flat (within the LR4's allpass phase response).
//!
//! ## Width
//!
//! Per band: M = (L+R)/2, S = (L-R)/2 * width, then L' = M+S', R' = M-S'.
//! width 0 = mono, 1 = unity, up to 4 = very wide. Applied independently
//! to each band, so you can mono the sub (width_low = 0) while pushing the
//! hats wide (width_high = 2.5) — the headline feature.
//!
//! ## Metering
//!
//! Per-band sliding-window correlation (so the UI can show which band is
//! going mono-incompatible), plus a downsampled L/R scatter buffer for the
//! goniometer/vectorscope in the editor.

use crate::dsp::filters::LinkwitzRiley4;

const CORR_WINDOW_SAMPLES: usize = 2400; // ~50 ms @ 48k
/// Number of L/R points held for the goniometer. The editor reads this
/// ring each frame and draws the Lissajous figure. 256 points at the
/// decimation rate below traces a readable figure without flooding the
/// packet.
pub const GONIO_POINTS: usize = 256;
/// Take one goniometer sample every N output frames. At 48k, /32 ≈ 1.5k
/// points/sec — plenty for a smooth scope, tiny in the packet.
const GONIO_DECIMATE: usize = 32;

/// Sliding-window Pearson correlation over the output of one band.
struct CorrTracker {
    lr: f32,
    ll: f32,
    rr: f32,
    ring_lr: Box<[f32]>,
    ring_ll: Box<[f32]>,
    ring_rr: Box<[f32]>,
    idx: usize,
    last: f32,
}

impl CorrTracker {
    fn new() -> Self {
        Self {
            lr: 0.0, ll: 0.0, rr: 0.0,
            ring_lr: vec![0.0; CORR_WINDOW_SAMPLES].into_boxed_slice(),
            ring_ll: vec![0.0; CORR_WINDOW_SAMPLES].into_boxed_slice(),
            ring_rr: vec![0.0; CORR_WINDOW_SAMPLES].into_boxed_slice(),
            idx: 0,
            last: 1.0,
        }
    }

    #[inline]
    fn push(&mut self, l: f32, r: f32) {
        let i = self.idx;
        let nlr = l * r;
        let nll = l * l;
        let nrr = r * r;
        self.lr += nlr - self.ring_lr[i];
        self.ll += nll - self.ring_ll[i];
        self.rr += nrr - self.ring_rr[i];
        self.ring_lr[i] = nlr;
        self.ring_ll[i] = nll;
        self.ring_rr[i] = nrr;
        self.idx = (i + 1) % CORR_WINDOW_SAMPLES;
    }

    fn finalize(&mut self) {
        let denom = (self.ll * self.rr).sqrt();
        self.last = if denom > 1e-12 {
            (self.lr / denom).clamp(-1.0, 1.0)
        } else {
            1.0
        };
    }

    fn reset(&mut self) {
        self.lr = 0.0; self.ll = 0.0; self.rr = 0.0;
        for s in self.ring_lr.iter_mut() { *s = 0.0; }
        for s in self.ring_ll.iter_mut() { *s = 0.0; }
        for s in self.ring_rr.iter_mut() { *s = 0.0; }
        self.idx = 0;
        self.last = 1.0;
    }
}

pub struct MultibandWidener {
    sample_rate: f32,

    // Crossover frequencies. f_lo splits low|mid, f_hi splits mid|high.
    f_lo: f32,
    f_hi: f32,

    // Per-band width (0 = mono, 1 = unity, up to 4).
    width_low: f32,
    width_mid: f32,
    width_high: f32,

    // Crossovers, per channel. xover_lo @ f_lo, xover_hi @ f_hi, plus an
    // allpass-compensation LR4 @ f_hi applied to the low band.
    xlo_l: LinkwitzRiley4,
    xlo_r: LinkwitzRiley4,
    xhi_l: LinkwitzRiley4,
    xhi_r: LinkwitzRiley4,
    ap_l: LinkwitzRiley4,
    ap_r: LinkwitzRiley4,

    corr_low: CorrTracker,
    corr_mid: CorrTracker,
    corr_high: CorrTracker,

    // Goniometer scatter ring (interleaved L,R pairs).
    gonio: Box<[(f32, f32)]>,
    gonio_idx: usize,
    gonio_decimate_ctr: usize,
}

impl MultibandWidener {
    pub fn new(sample_rate: f32) -> Self {
        let mut w = Self {
            sample_rate,
            f_lo: 200.0,
            f_hi: 3000.0,
            width_low: 1.0,
            width_mid: 1.0,
            width_high: 1.0,
            xlo_l: LinkwitzRiley4::new(),
            xlo_r: LinkwitzRiley4::new(),
            xhi_l: LinkwitzRiley4::new(),
            xhi_r: LinkwitzRiley4::new(),
            ap_l: LinkwitzRiley4::new(),
            ap_r: LinkwitzRiley4::new(),
            corr_low: CorrTracker::new(),
            corr_mid: CorrTracker::new(),
            corr_high: CorrTracker::new(),
            gonio: vec![(0.0, 0.0); GONIO_POINTS].into_boxed_slice(),
            gonio_idx: 0,
            gonio_decimate_ctr: 0,
        };
        w.update_filters();
        w
    }

    fn update_filters(&mut self) {
        for f in [&mut self.xlo_l, &mut self.xlo_r] {
            f.set_freq(self.f_lo, self.sample_rate);
        }
        for f in [&mut self.xhi_l, &mut self.xhi_r, &mut self.ap_l, &mut self.ap_r] {
            f.set_freq(self.f_hi, self.sample_rate);
        }
    }

    pub fn set_sample_rate(&mut self, sr: f32) {
        self.sample_rate = sr;
        self.update_filters();
    }

    pub fn reset(&mut self) {
        for f in [
            &mut self.xlo_l, &mut self.xlo_r, &mut self.xhi_l,
            &mut self.xhi_r, &mut self.ap_l, &mut self.ap_r,
        ] {
            f.reset();
        }
        self.corr_low.reset();
        self.corr_mid.reset();
        self.corr_high.reset();
        for p in self.gonio.iter_mut() { *p = (0.0, 0.0); }
        self.gonio_idx = 0;
        self.gonio_decimate_ctr = 0;
    }

    pub fn set_widths(&mut self, low: f32, mid: f32, high: f32) {
        self.width_low = low.clamp(0.0, 4.0);
        self.width_mid = mid.clamp(0.0, 4.0);
        self.width_high = high.clamp(0.0, 4.0);
    }

    /// Set crossover points. Enforces f_lo < f_hi with a margin so the mid
    /// band never collapses to nothing.
    pub fn set_crossovers(&mut self, f_lo: f32, f_hi: f32) {
        let lo = f_lo.clamp(40.0, 1000.0);
        let hi = f_hi.clamp(lo * 1.5, 16000.0);
        if (lo - self.f_lo).abs() > 0.01 || (hi - self.f_hi).abs() > 0.01 {
            self.f_lo = lo;
            self.f_hi = hi;
            self.update_filters();
        }
    }

    #[inline]
    pub fn correlation_low(&self) -> f32 { self.corr_low.last }
    #[inline]
    pub fn correlation_mid(&self) -> f32 { self.corr_mid.last }
    #[inline]
    pub fn correlation_high(&self) -> f32 { self.corr_high.last }

    /// Copy the goniometer scatter into the provided slice (oldest→newest).
    /// `out` should be GONIO_POINTS long; extra is ignored, short is filled
    /// up to its length.
    pub fn copy_goniometer(&self, out: &mut [(f32, f32)]) {
        let n = out.len().min(GONIO_POINTS);
        for k in 0..n {
            // Read in chronological order starting just after the write head.
            let src = (self.gonio_idx + k) % GONIO_POINTS;
            out[k] = self.gonio[src];
        }
    }

    #[inline]
    fn widen_band(l: f32, r: f32, width: f32) -> (f32, f32) {
        let mid = 0.5 * (l + r);
        let side = 0.5 * (l - r) * width;
        (mid + side, mid - side)
    }

    /// Process one stereo pair in place.
    #[inline]
    pub fn process(&mut self, l: &mut f32, r: &mut f32) {
        let in_l = *l;
        let in_r = *r;

        // 3-band split, per channel.
        let (low_raw_l, rest_l) = self.xlo_l.process(in_l);
        let (low_raw_r, rest_r) = self.xlo_r.process(in_r);
        let (mid_l, high_l) = self.xhi_l.process(rest_l);
        let (mid_r, high_r) = self.xhi_r.process(rest_r);

        // Allpass-align the low band to the f_hi crossover's phase: an LR4's
        // (lo + hi) == allpass(input).
        let (a_l, b_l) = self.ap_l.process(low_raw_l);
        let (a_r, b_r) = self.ap_r.process(low_raw_r);
        let low_l = a_l + b_l;
        let low_r = a_r + b_r;

        // Per-band M/S widening.
        let (wl_low, wr_low) = Self::widen_band(low_l, low_r, self.width_low);
        let (wl_mid, wr_mid) = Self::widen_band(mid_l, mid_r, self.width_mid);
        let (wl_high, wr_high) = Self::widen_band(high_l, high_r, self.width_high);

        // Per-band correlation on the widened output (what the user hears).
        self.corr_low.push(wl_low, wr_low);
        self.corr_mid.push(wl_mid, wr_mid);
        self.corr_high.push(wl_high, wr_high);

        let out_l = wl_low + wl_mid + wl_high;
        let out_r = wr_low + wr_mid + wr_high;
        *l = out_l;
        *r = out_r;

        // Goniometer decimated capture.
        self.gonio_decimate_ctr += 1;
        if self.gonio_decimate_ctr >= GONIO_DECIMATE {
            self.gonio_decimate_ctr = 0;
            self.gonio[self.gonio_idx] = (out_l, out_r);
            self.gonio_idx = (self.gonio_idx + 1) % GONIO_POINTS;
        }
    }

    /// Recompute all three band correlations. Call once per audio block.
    pub fn finalize(&mut self) {
        self.corr_low.finalize();
        self.corr_mid.finalize();
        self.corr_high.finalize();
    }
}
