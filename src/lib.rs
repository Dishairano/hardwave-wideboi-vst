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

use nih_plug::prelude::*;
use std::num::NonZeroU32;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

mod crash_reporter;
mod dsp;
mod params;

use dsp::StereoWidener;
use params::HardwaveWideBoiParams;

struct HardwaveWideBoi {
    params: Arc<HardwaveWideBoiParams>,
    widener: StereoWidener,
    sample_rate: f32,

    /// Latest stereo correlation, packed as f32 bits in an `AtomicU32`.
    /// Written from the audio thread once per block; read by the editor
    /// (or any other host integration) without locking.
    correlation: Arc<AtomicU32>,
}

impl Default for HardwaveWideBoi {
    fn default() -> Self {
        let sr = 44_100.0;
        Self {
            params: Arc::new(HardwaveWideBoiParams::default()),
            widener: StereoWidener::new(sr),
            sample_rate: sr,
            correlation: Arc::new(AtomicU32::new(1.0_f32.to_bits())),
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
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        if self.params.bypass.value() {
            return ProcessStatus::Normal;
        }

        let mono_on = self.params.mono_bass_on.value();

        for mut frame in buffer.iter_samples() {
            if frame.len() < 2 {
                continue;
            }

            // Pull the next smoothed sample for each automatable param so
            // user automation doesn't introduce zipper noise.
            let width = self.params.width.smoothed.next();
            let mono_hz = self.params.mono_bass_hz.smoothed.next();
            let out_gain = util::db_to_gain(self.params.output_gain_db.smoothed.next());

            self.widener.set_width(width);
            self.widener.set_mono_bass(mono_hz, mono_on);

            let l_ptr = frame.get_mut(0).unwrap() as *mut f32;
            let r_ptr = frame.get_mut(1).unwrap() as *mut f32;
            unsafe {
                self.widener.process(&mut *l_ptr, &mut *r_ptr);
                *l_ptr *= out_gain;
                *r_ptr *= out_gain;
            }
        }

        // Update the correlation read-out once per block — cheap and gives
        // the editor (when it lands in v0.3) a smooth, lock-free meter.
        self.widener.finalize_correlation();
        self.correlation.store(
            self.widener.correlation().to_bits(),
            Ordering::Relaxed,
        );

        ProcessStatus::Normal
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        // Webview editor lands in v0.3. The DAW generic UI is used until
        // then so the plugin is usable end-to-end.
        None
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
