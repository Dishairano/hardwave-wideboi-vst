//! DSP processing modules for WideBoi.

pub mod filters;
pub mod multiband_widener;
pub mod widener;

pub use multiband_widener::MultibandWidener;
// widener::StereoWidener (v0.3 single-band engine) is kept as a module for
// reference/fallback but no longer re-exported — MultibandWidener supersedes it.

#[cfg(test)]
mod multiband_widener_test;
