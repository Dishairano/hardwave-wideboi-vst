//! Sanity/null tests for the multiband widener. These can't judge "sounds
//! good" but they catch the catastrophic bugs that would make a release
//! audibly broken: non-transparent unity, lost mono compatibility, NaN/Inf,
//! or runaway gain.

#[cfg(test)]
mod tests {
    use crate::dsp::MultibandWidener;

    fn run(w: &mut MultibandWidener, samples: &[(f32, f32)]) -> Vec<(f32, f32)> {
        samples.iter().map(|&(l, r)| {
            let mut a = l; let mut b = r;
            w.process(&mut a, &mut b);
            (a, b)
        }).collect()
    }

    /// A test signal: mix of sines across the spectrum, slightly different
    /// L/R so there IS a side component to widen.
    fn test_signal(n: usize, sr: f32) -> Vec<(f32, f32)> {
        (0..n).map(|i| {
            let t = i as f32 / sr;
            let l = 0.3 * (2.0 * std::f32::consts::PI * 80.0 * t).sin()
                  + 0.2 * (2.0 * std::f32::consts::PI * 800.0 * t).sin()
                  + 0.15 * (2.0 * std::f32::consts::PI * 6000.0 * t).sin();
            let r = 0.3 * (2.0 * std::f32::consts::PI * 80.0 * t).sin()
                  + 0.2 * (2.0 * std::f32::consts::PI * 800.0 * t + 0.4).sin()
                  + 0.15 * (2.0 * std::f32::consts::PI * 6000.0 * t + 0.8).sin();
            (l, r)
        }).collect()
    }

    #[test]
    fn unity_is_transparent() {
        // At unity (all widths 1.0) the widener must be TONALLY transparent
        // and preserve the stereo image. It is NOT sample-identical to the
        // input — the LR4 crossovers introduce an allpass phase rotation
        // (~360° across the spectrum). That phase shift is identical on L and
        // R, so it's inaudible: tone is unchanged (magnitude-flat) and the
        // stereo relationship is unchanged. We verify the two things that ARE
        // audible:
        //   1. Per-channel energy (RMS) preserved → no magnitude coloration /
        //      no crossover notch.
        //   2. Stereo correlation preserved → image not smeared.
        // (A naive sample-difference test wrongly flags the benign allpass
        // phase — it reads ~106% "error" for a ~64° rotation. See git log.)
        let sr = 48000.0;
        let mut w = MultibandWidener::new(sr);
        w.set_widths(1.0, 1.0, 1.0);
        let sig = test_signal(8192, sr);
        let out = run(&mut w, &sig);

        let warm = 2048;
        let mut in_l = 0.0f64; let mut in_r = 0.0f64;
        let mut out_l = 0.0f64; let mut out_r = 0.0f64;
        let mut in_lr = 0.0f64; let mut out_lr = 0.0f64;
        for i in warm..sig.len() {
            let (il, ir) = sig[i];
            let (ol, or) = out[i];
            in_l  += (il as f64).powi(2);  in_r  += (ir as f64).powi(2);
            out_l += (ol as f64).powi(2);  out_r += (or as f64).powi(2);
            in_lr += (il * ir) as f64;     out_lr += (ol * or) as f64;
        }
        // 1. Energy preservation per channel (magnitude flatness).
        let el = (out_l / in_l).sqrt();
        let er = (out_r / in_r).sqrt();
        assert!((el - 1.0).abs() < 0.05, "L energy not preserved: ratio {el:.4} (>5% off)");
        assert!((er - 1.0).abs() < 0.05, "R energy not preserved: ratio {er:.4} (>5% off)");
        // 2. Stereo correlation preservation (image intact).
        let in_corr  = in_lr  / (in_l * in_r).sqrt();
        let out_corr = out_lr / (out_l * out_r).sqrt();
        assert!((in_corr - out_corr).abs() < 0.05,
            "stereo image changed at unity: corr {in_corr:.3} -> {out_corr:.3}");
    }

    #[test]
    fn mono_input_stays_mono() {
        // Identical L/R in → identical L/R out regardless of widths (you
        // can't widen what has no side signal).
        let sr = 48000.0;
        let mut w = MultibandWidener::new(sr);
        w.set_widths(0.0, 2.0, 4.0);
        let mono: Vec<(f32, f32)> = (0..4096).map(|i| {
            let v = 0.4 * (2.0 * std::f32::consts::PI * 200.0 * i as f32 / sr).sin();
            (v, v)
        }).collect();
        let out = run(&mut w, &mono);
        for i in 1024..out.len() {
            let (l, r) = out[i];
            assert!((l - r).abs() < 1e-4, "mono diverged at {i}: L={l} R={r}");
        }
    }

    #[test]
    fn no_nan_or_runaway() {
        // Extreme widths + crossovers + a hot signal must not produce NaN,
        // Inf, or gain blow-up.
        let sr = 44100.0;
        let mut w = MultibandWidener::new(sr);
        w.set_widths(4.0, 4.0, 4.0);
        w.set_crossovers(40.0, 16000.0);
        let sig = test_signal(8192, sr);
        let out = run(&mut w, &sig);
        for (l, r) in out.iter().skip(2048) {
            assert!(l.is_finite() && r.is_finite(), "non-finite output");
            assert!(l.abs() < 16.0 && r.abs() < 16.0, "runaway gain: {l}, {r}");
        }
    }

    #[test]
    fn mono_low_band_centers_bass() {
        // width_low = 0 should mono the low band: a wide bass tone collapses
        // to center while a wide high tone stays wide.
        let sr = 48000.0;
        let mut w = MultibandWidener::new(sr);
        w.set_crossovers(200.0, 3000.0);
        w.set_widths(0.0, 1.0, 1.0);
        // Wide 60 Hz (well below the 200 Hz low crossover): hard-panned-ish.
        let sig: Vec<(f32, f32)> = (0..8192).map(|i| {
            let t = i as f32 / sr;
            let l = 0.4 * (2.0 * std::f32::consts::PI * 60.0 * t).sin();
            let r = 0.4 * (2.0 * std::f32::consts::PI * 60.0 * t + 1.0).sin(); // phase-offset = side energy
            (l, r)
        }).collect();
        let out = run(&mut w, &sig);
        // After mono-ing the low band, L and R should be much closer than input.
        let warm = 4096;
        let mut in_diff = 0.0f64; let mut out_diff = 0.0f64;
        for i in warm..sig.len() {
            in_diff += (sig[i].0 - sig[i].1).abs() as f64;
            out_diff += (out[i].0 - out[i].1).abs() as f64;
        }
        assert!(out_diff < in_diff * 0.25,
            "low band not monoed: in L-R sum {in_diff:.1}, out {out_diff:.1}");
    }
}
