//! Hardwave WideBoi — stereo widener VST3/CLAP plugin.
//!
//! Signal chain:
//!   Input → (optional mono-bass crossover) → Mid/Side width scaling → Output
//!
//! Sibling of WettBoi; shares branding and webview architecture but performs
//! stereo widening rather than wet-signal generation.

use nih_plug::prelude::*;
use std::sync::Arc;

mod dsp;
mod params;

use dsp::StereoWidener;
use params::HardwaveWideBoiParams;
use std::num::NonZeroU32;

struct HardwaveWideBoi {
    params: Arc<HardwaveWideBoiParams>,
    widener: StereoWidener,
    sample_rate: f32,
}

impl Default for HardwaveWideBoi {
    fn default() -> Self {
        let sr = 44_100.0;
        Self {
            params: Arc::new(HardwaveWideBoiParams::default()),
            widener: StereoWidener::new(sr),
            sample_rate: sr,
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
        self.sample_rate = buffer_config.sample_rate;
        self.widener.set_sample_rate(self.sample_rate);
        true
    }

    fn reset(&mut self) {
        self.widener.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let p = &self.params;

        if p.bypass.value() {
            return ProcessStatus::Normal;
        }

        let width = p.width.value();
        let mono_on = p.mono_bass_on.value();
        let mono_hz = p.mono_bass_hz.value();
        let out_gain = util::db_to_gain(p.output_gain_db.value());

        self.widener.set_width(width);
        self.widener.set_mono_bass(mono_hz, mono_on);

        for mut frame in buffer.iter_samples() {
            if frame.len() < 2 {
                continue;
            }
            let l_ptr = frame.get_mut(0).unwrap() as *mut f32;
            let r_ptr = frame.get_mut(1).unwrap() as *mut f32;
            unsafe {
                self.widener.process(&mut *l_ptr, &mut *r_ptr);
                *l_ptr *= out_gain;
                *r_ptr *= out_gain;
            }
        }

        ProcessStatus::Normal
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        // Editor/webview UI comes in a later release; DAW generic UI is used
        // for now so the plugin is usable end-to-end.
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
