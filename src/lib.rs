//! # neurofield
//!
//! Rust library and terminal UI for streaming real-time EEG data from
//! **Neurofield Q21** 20-channel EEG amplifiers over PCAN-USB.
//!
//! The Q21 is a USA FDA approved, high-resolution, DC-coupled 20-channel
//! simultaneous-sampling amplifier with very low input noise and high CMRR.
//! Communication uses CAN bus via a PEAK PCAN-USB adapter.
//!
//! ## Cross-platform
//!
//! Works on **Windows**, **Linux**, and **macOS**.  The PCANBasic shared
//! library is loaded at runtime via `libloading` — no build-time C
//! dependencies.
//!
//! ## Quick start
//!
//! ```rust,ignore
//! use neurofield::prelude::*;
//!
//! let mut api = Q21Api::new(UsbBus::USB1)?;
//!
//! println!("Device: {:?}, serial: {}", api.eeg_device_type(), api.eeg_device_serial());
//! println!("Impedance capable: {}", api.impedance_enabled());
//!
//! api.start_receiving_eeg()?;
//!
//! for _ in 0..(SAMPLING_RATE as usize * 4) {
//!     let sample = api.get_single_sample()?;
//!     // sample.data: [f64; 20] in µV
//! }
//!
//! api.abort_receiving_eeg()?;
//! ```
//!
//! ## Using as a library dependency
//!
//! ```toml
//! [dependencies]
//! # Full build (includes the ratatui TUI feature):
//! neurofield = "0.0.1"
//!
//! # Library only — skips ratatui / crossterm compilation:
//! neurofield = { version = "0.0.1", default-features = false }
//! ```
//!
//! ## Module overview
//!
//! | Module | Purpose |
//! |---|---|
//! | [`prelude`] | One-line glob import of the most commonly needed types |
//! | [`pcan`] | Cross-platform FFI bindings for PCANBasic (runtime-loaded) |
//! | [`device`] | Device types and identification |
//! | [`message_types`] | CAN-bus message type enums and header decoding |
//! | [`canbus_base`] | Low-level CAN transport, device discovery, send/receive |
//! | [`eeg_api`] | Mid-level EEG protocol: streaming, raw ADC, mode switching |
//! | [`q21_api`] | Top-level API: scaled µV samples and impedance in Ω |
//! | [`error`] | Error types |

pub mod pcan;
pub mod device;
pub mod error;
pub mod message_types;
pub mod canbus_base;
pub mod eeg_api;
pub mod q21_api;

// ── Prelude ────────────────���─────────────────────────────────���────────────────

/// Convenience re-exports for downstream crates.
///
/// A single glob import covers the entire surface area needed to discover,
/// connect, and stream data from a Q21 amplifier:
///
/// ```rust,ignore
/// use neurofield::prelude::*;
///
/// let mut api = Q21Api::new(UsbBus::USB1)?;
/// api.start_receiving_eeg()?;
/// let sample = api.get_single_sample()?;
/// api.abort_receiving_eeg()?;
/// ```
pub mod prelude {
    // ── Bus ───────────────────────────────────────────────────────────────────
    pub use crate::pcan::UsbBus;

    // ── Device identification ─────────��───────────────────────────────────────
    pub use crate::device::{DeviceType, Device};

    // ── Error ─────────���───────────────────────────────────────────────────────
    pub use crate::error::NeurofieldError;

    // ── Protocol ───────────────────────────────────────────��──────────────────
    pub use crate::message_types::{Q21MessageType, ExtendedHeader};

    // ── API layers ──────────────────��───────────────────────────���─────────────
    pub use crate::canbus_base::CanBusBase;
    pub use crate::eeg_api::EegApi;
    pub use crate::q21_api::{Q21Api, EegSample, ImpedanceSample};

    // ── Constants ──────────���────────────────────────────��─────────────────────
    pub use crate::q21_api::{
        NUM_CHANNELS, SAMPLING_RATE,
        INJECTED_CURRENT_FOR_IMPEDANCE, RESISTOR_LINE,
    };

    // ── Channel names ───────��────────────────────────���────────────────────────
    pub use crate::q21_api::EEG_CHANNEL_NAMES;
}
