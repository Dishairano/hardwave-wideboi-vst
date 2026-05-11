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
    pub width: f32,            // 0..4 — 0 = mono, 1 = unity, up to 4x widening
    pub mono_bass_on: bool,
    pub mono_bass_hz: f32,     // 20..500 — crossover cutoff
    pub output_gain_db: f32,   // -24..+24
    pub bypass: bool,

    // ── Metering ────────────────────────────────────────────────────────────
    pub input_peak_l: f32,
    pub input_peak_r: f32,
    pub output_peak_l: f32,
    pub output_peak_r: f32,
    pub correlation: f32,      // -1..+1 — stereo correlation read-out
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
            input_peak_l: 0.0,
            input_peak_r: 0.0,
            output_peak_l: 0.0,
            output_peak_r: 0.0,
            correlation: 1.0,
        }
    }
}
