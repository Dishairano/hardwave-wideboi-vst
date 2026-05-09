//! DAW-exposed parameters for Hardwave WideBoi — a stereo widener.
//!
//! All numeric params are smoothed (logarithmic for gain/freq, linear for
//! width) so user automation doesn't introduce zipper noise on the audio.

use nih_plug::prelude::*;
use std::sync::Arc;

#[derive(Params)]
pub struct HardwaveWideBoiParams {
    /// Stereo width amount. 0% = mono, 100% = unity, up to 400%.
    #[id = "width"]
    pub width: FloatParam,

    /// If enabled, frequencies below `mono_bass_hz` are summed to mono
    /// regardless of the width setting.
    #[id = "mono_bass_on"]
    pub mono_bass_on: BoolParam,

    /// Mono-bass crossover frequency.
    #[id = "mono_bass_hz"]
    pub mono_bass_hz: FloatParam,

    /// Output trim.
    #[id = "output_gain_db"]
    pub output_gain_db: FloatParam,

    /// Global bypass for A/B.
    #[id = "bypass"]
    pub bypass: BoolParam,
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
