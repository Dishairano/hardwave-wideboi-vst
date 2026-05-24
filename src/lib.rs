//! Hardwave WideBoi — stereo widener VST3/CLAP plugin.
//!
//! Signal chain:
//!   Input → (optional Linkwitz-Riley 4 mono-bass crossover)
//!         → Mid/Side width scaling on the high band
//!         → Sum back with mono-summed low band
//!         → Output gain
//!
//! Sibling of WettBoi; shares branding and webview architecture but performs
//! stereo widening rather than wet-signal generation.

use crossbeam_channel::{Receiver, Sender};
use nih_plug::prelude::*;
use parking_lot::Mutex;
use std::num::NonZeroU32;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

mod auth;
mod crash_reporter;
mod dsp;
mod editor;
mod params;
mod protocol;

use dsp::MultibandWidener;
use params::HardwaveWideBoiParams;
use protocol::WbPacket;

struct HardwaveWideBoi {
    params: Arc<HardwaveWideBoiParams>,
    widener: MultibandWidener,
    sample_rate: f32,

    /// Latest stereo correlation, packed as f32 bits in an `AtomicU32`.
    /// Written from the audio thread once per block; read by the editor
    /// (or any other host integration) without locking.
    correlation: Arc<AtomicU32>,

    // Editor communication. The audio thread pushes a snapshot ~60 fps
    // through a bounded channel; the editor JS-pump thread drains it.
    editor_packet_tx: Sender<WbPacket>,
    editor_packet_rx: Arc<Mutex<Receiver<WbPacket>>>,
    update_counter: u32,

    // Audio-thread metering — packed back into the packet snapshot just
    // before it ships to the editor.
    input_peak_l: f32,
    input_peak_r: f32,
    output_peak_l: f32,
    output_peak_r: f32,
}

impl Default for HardwaveWideBoi {
    fn default() -> Self {
        let sr = 44_100.0;
        let (pkt_tx, pkt_rx) = crossbeam_channel::bounded(4);
        Self {
            params: Arc::new(HardwaveWideBoiParams::default()),
            widener: MultibandWidener::new(sr),
            sample_rate: sr,
            correlation: Arc::new(AtomicU32::new(1.0_f32.to_bits())),
            editor_packet_tx: pkt_tx,
            editor_packet_rx: Arc::new(Mutex::new(pkt_rx)),
            update_counter: 0,
            input_peak_l: 0.0,
            input_peak_r: 0.0,
            output_peak_l: 0.0,
            output_peak_r: 0.0,
        }
    }
}

impl Plugin for HardwaveWideBoi {
    const NAME: &'static str = "Hardwave WideBoi";
    const VENDOR: &'static str = "Hardwave Studios";
    const URL: &'static str = "https://hardwavestudios.com";
    const EMAIL: &'static str = "support@hardwavestudios.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(2),
        main_output_channels: NonZeroU32::new(2),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        crash_reporter::install("wideboi");
        self.sample_rate = buffer_config.sample_rate;
        self.widener.set_sample_rate(self.sample_rate);
        true
    }

    fn reset(&mut self) {
        self.widener.reset();
        self.correlation.store(1.0_f32.to_bits(), Ordering::Relaxed);
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut peak_in_l = 0.0_f32;
        let mut peak_in_r = 0.0_f32;
        let mut peak_out_l = 0.0_f32;
        let mut peak_out_r = 0.0_f32;

        let bypass = self.params.bypass.value();

        // Crossover frequencies set ONCE per block (not per sample) — changing
        // them recomputes biquad coefficients (expensive) and crossover moves
        // don't need audio-rate smoothing. set_crossovers no-ops if unchanged.
        self.widener.set_crossovers(
            self.params.xover_lo_hz.value(),
            self.params.xover_hi_hz.value(),
        );

        for mut frame in buffer.iter_samples() {
            if frame.len() < 2 {
                continue;
            }

            let l_ptr = frame.get_mut(0).unwrap() as *mut f32;
            let r_ptr = frame.get_mut(1).unwrap() as *mut f32;
            unsafe {
                peak_in_l = peak_in_l.max((*l_ptr).abs());
                peak_in_r = peak_in_r.max((*r_ptr).abs());

                if !bypass {
                    // Per-band widths pulled smoothed per-sample (cheap: three
                    // float stores) so width automation stays zipper-free.
                    let wl = self.params.width_low.smoothed.next();
                    let wm = self.params.width_mid.smoothed.next();
                    let wh = self.params.width_high.smoothed.next();
                    let out_gain = util::db_to_gain(self.params.output_gain_db.smoothed.next());

                    self.widener.set_widths(wl, wm, wh);
                    self.widener.process(&mut *l_ptr, &mut *r_ptr);
                    *l_ptr *= out_gain;
                    *r_ptr *= out_gain;
                }

                peak_out_l = peak_out_l.max((*l_ptr).abs());
                peak_out_r = peak_out_r.max((*r_ptr).abs());
            }
        }

        // Finalize per-band correlations once per block — cheap, lock-free.
        self.widener.finalize();
        let corr_low = self.widener.correlation_low();
        let corr_mid = self.widener.correlation_mid();
        let corr_high = self.widener.correlation_high();
        // Legacy overall meter: mean of the three bands (a glance, not exact).
        let correlation = (corr_low + corr_mid + corr_high) / 3.0;
        self.correlation.store(correlation.to_bits(), Ordering::Relaxed);

        // Decay the held peaks slightly each block so meters fall back rather
        // than locking at max forever.
        const PEAK_DECAY: f32 = 0.85;
        self.input_peak_l  = (self.input_peak_l  * PEAK_DECAY).max(peak_in_l);
        self.input_peak_r  = (self.input_peak_r  * PEAK_DECAY).max(peak_in_r);
        self.output_peak_l = (self.output_peak_l * PEAK_DECAY).max(peak_out_l);
        self.output_peak_r = (self.output_peak_r * PEAK_DECAY).max(peak_out_r);

        // Ship a packet to the editor at ~30 fps. We rely on the host's
        // transport state for BPM; if it's unavailable we hold the last value.
        self.update_counter = self.update_counter.wrapping_add(1);
        if self.update_counter >= 2 {
            self.update_counter = 0;
            let bpm = context
                .transport()
                .tempo
                .map(|t| t as f32)
                .unwrap_or(150.0);

            let mut packet = editor::snapshot_params(&self.params, bpm, correlation);
            packet.input_peak_l  = self.input_peak_l;
            packet.input_peak_r  = self.input_peak_r;
            packet.output_peak_l = self.output_peak_l;
            packet.output_peak_r = self.output_peak_r;
            packet.correlation_low  = corr_low;
            packet.correlation_mid  = corr_mid;
            packet.correlation_high = corr_high;
            // Flatten the goniometer scatter into [l0,r0,l1,r1,...].
            let mut pts = [(0.0f32, 0.0f32); dsp::multiband_widener::GONIO_POINTS];
            self.widener.copy_goniometer(&mut pts);
            let mut gonio = Vec::with_capacity(pts.len() * 2);
            for (gl, gr) in pts.iter() {
                gonio.push(*gl);
                gonio.push(*gr);
            }
            packet.gonio = gonio;

            // try_send drops the packet if the editor isn't draining fast
            // enough — preferable to blocking the audio thread.
            let _ = self.editor_packet_tx.try_send(packet);
        }

        ProcessStatus::Normal
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        eprintln!("[HardwaveWideBoi] editor() called — creating WideBoiEditor");
        let token = auth::load_token();
        eprintln!(
            "[HardwaveWideBoi] auth token: {}",
            if token.is_some() { "present" } else { "none" }
        );
        Some(Box::new(editor::WideBoiEditor::new(
            Arc::clone(&self.params),
            Arc::clone(&self.editor_packet_rx),
            token,
        )))
    }
}

impl ClapPlugin for HardwaveWideBoi {
    const CLAP_ID: &'static str = "com.hardwavestudios.wideboi";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("Stereo widener with mono-bass preservation");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Utility,
    ];
}

impl Vst3Plugin for HardwaveWideBoi {
    const VST3_CLASS_ID: [u8; 16] = *b"HwaveWideBoi0001";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Stereo];
}

nih_export_clap!(HardwaveWideBoi);
nih_export_vst3!(HardwaveWideBoi);
