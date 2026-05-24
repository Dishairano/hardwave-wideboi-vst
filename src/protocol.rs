//! Rust → JS packet for the WideBoi webview UI.
//!
//! JS → Rust messages are parsed in `editor.rs` directly from JSON
//! (matching on the `"type"` field) so this module only needs the
//! outbound packet type.

use serde::Serialize;

/// Full state packet pushed to the webview at ~30 fps. Shape matches the
/// `WbPacket` interface in `apps/wideboi/src/hooks/useWbPacket.ts`.
#[derive(Debug, Clone, Serialize)]
pub struct WbPacket {
    pub bpm: f32,

    // ── Stereo widener controls ─────────────────────────────────────────────
    pub width: f32,            // LEGACY single width — kept for back-compat
    pub mono_bass_on: bool,    // LEGACY
    pub mono_bass_hz: f32,     // LEGACY
    pub output_gain_db: f32,   // -24..+24
    pub bypass: bool,

    // ── Multiband (v0.4) ─────────────────────────────────────────────────────
    pub width_low: f32,        // 0..4 per-band width
    pub width_mid: f32,
    pub width_high: f32,
    pub xover_lo_hz: f32,      // 40..1000
    pub xover_hi_hz: f32,      // 300..16000

    // ── Metering ────────────────────────────────────────────────────────────
    pub input_peak_l: f32,
    pub input_peak_r: f32,
    pub output_peak_l: f32,
    pub output_peak_r: f32,
    pub correlation: f32,      // overall -1..+1 (legacy meter)
    pub correlation_low: f32,  // per-band correlation
    pub correlation_mid: f32,
    pub correlation_high: f32,
    /// Goniometer scatter — flattened interleaved [l0,r0,l1,r1,...] output
    /// samples, decimated. Editor plots as a Lissajous figure.
    pub gonio: Vec<f32>,
}

impl Default for WbPacket {
    fn default() -> Self {
        Self {
            bpm: 150.0,
            width: 1.0,
            mono_bass_on: true,
            mono_bass_hz: 120.0,
            output_gain_db: 0.0,
            bypass: false,
            width_low: 1.0,
            width_mid: 1.0,
            width_high: 1.0,
            xover_lo_hz: 200.0,
            xover_hi_hz: 3000.0,
            input_peak_l: 0.0,
            input_peak_r: 0.0,
            output_peak_l: 0.0,
            output_peak_r: 0.0,
            correlation: 1.0,
            correlation_low: 1.0,
            correlation_mid: 1.0,
            correlation_high: 1.0,
            gonio: Vec::new(),
        }
    }
}
