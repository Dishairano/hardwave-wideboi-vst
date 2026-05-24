//! DAW-exposed parameters for Hardwave WideBoi — a stereo widener.
//!
//! All numeric params are smoothed (logarithmic for gain/freq, linear for
//! width) so user automation doesn't introduce zipper noise on the audio.

use nih_plug::prelude::*;
use std::sync::Arc;

/// Short alias for use across modules (editor, protocol). The full
/// `HardwaveWideBoiParams` name stays for the public-API shape.
pub type WideBoiParams = HardwaveWideBoiParams;

#[derive(Params)]
pub struct HardwaveWideBoiParams {
    /// LEGACY single-band width (v0.3.x). Kept so pre-v0.4 saved projects
    /// still deserialize, but the multiband engine ignores it — the three
    /// per-band widths below drive the audio now. A v0.3 project loads with
    /// unity multiband (safe) rather than silently re-applying old width.
    #[id = "width"]
    pub width: FloatParam,

    // ── Multiband width (v0.4 — the headline feature) ────────────────────
    /// Width of the LOW band (below the low crossover). 0 = mono kick/sub.
    #[id = "width_low"]
    pub width_low: FloatParam,
    /// Width of the MID band.
    #[id = "width_mid"]
    pub width_mid: FloatParam,
    /// Width of the HIGH band.
    #[id = "width_high"]
    pub width_high: FloatParam,
    /// Low|mid crossover frequency.
    #[id = "xover_lo_hz"]
    pub xover_lo_hz: FloatParam,
    /// Mid|high crossover frequency.
    #[id = "xover_hi_hz"]
    pub xover_hi_hz: FloatParam,

    /// LEGACY mono-bass toggle (v0.3.x). Superseded by LOW band width = 0%.
    /// Kept for state-load compat; inert in the multiband engine.
    #[id = "mono_bass_on"]
    pub mono_bass_on: BoolParam,

    /// LEGACY mono-bass crossover frequency (v0.3.x). Inert; see above.
    #[id = "mono_bass_hz"]
    pub mono_bass_hz: FloatParam,

    /// Output trim.
    #[id = "output_gain_db"]
    pub output_gain_db: FloatParam,

    /// Global bypass for A/B.
    #[id = "bypass"]
    pub bypass: BoolParam,
}

fn width_param(name: &str) -> FloatParam {
    FloatParam::new(name, 1.0, FloatRange::Linear { min: 0.0, max: 4.0 })
        .with_unit(" x")
        .with_smoother(SmoothingStyle::Linear(20.0))
        .with_value_to_string(Arc::new(|v| format!("{:.0}%", v * 100.0)))
        .with_string_to_value(Arc::new(|s| {
            s.trim_end_matches('%').trim().parse::<f32>().ok().map(|p| p / 100.0)
        }))
}

impl Default for HardwaveWideBoiParams {
    fn default() -> Self {
        Self {
            width: FloatParam::new(
                "Width",
                1.0,
                FloatRange::Linear { min: 0.0, max: 4.0 },
            )
            .with_unit(" x")
            .with_smoother(SmoothingStyle::Linear(20.0))
            .with_value_to_string(Arc::new(|v| format!("{:.0}%", v * 100.0)))
            .with_string_to_value(Arc::new(|s| {
                s.trim_end_matches('%')
                    .trim()
                    .parse::<f32>()
                    .ok()
                    .map(|p| p / 100.0)
            })),

            // Multiband: all bands unity by default (transparent until the
            // user moves something). Low defaults to unity, NOT mono — users
            // who want mono lows set it to 0%.
            width_low: width_param("Low Width"),
            width_mid: width_param("Mid Width"),
            width_high: width_param("High Width"),
            xover_lo_hz: FloatParam::new(
                "Low/Mid Crossover",
                200.0,
                FloatRange::Skewed { min: 40.0, max: 1000.0, factor: FloatRange::skew_factor(-1.0) },
            )
            .with_unit(" Hz")
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_value_to_string(Arc::new(|v| format!("{:.0}", v))),
            xover_hi_hz: FloatParam::new(
                "Mid/High Crossover",
                3000.0,
                FloatRange::Skewed { min: 300.0, max: 16000.0, factor: FloatRange::skew_factor(-1.0) },
            )
            .with_unit(" Hz")
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_value_to_string(Arc::new(|v| format!("{:.0}", v))),

            mono_bass_on: BoolParam::new("Mono Bass", true),

            mono_bass_hz: FloatParam::new(
                "Mono Bass Freq",
                120.0,
                FloatRange::Skewed {
                    min: 20.0,
                    max: 500.0,
                    factor: FloatRange::skew_factor(-1.0),
                },
            )
            .with_unit(" Hz")
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_value_to_string(Arc::new(|v| format!("{:.0}", v))),

            output_gain_db: FloatParam::new(
                "Output",
                0.0,
                FloatRange::Linear { min: -24.0, max: 24.0 },
            )
            .with_unit(" dB")
            .with_smoother(SmoothingStyle::Linear(15.0))
            .with_value_to_string(Arc::new(|v| format!("{:+.1}", v))),

            bypass: BoolParam::new("Bypass", false),
        }
    }
}
