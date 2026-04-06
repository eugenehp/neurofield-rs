//! Top-level Q21 API — scaled EEG samples in microvolts and impedance in ohms.
//!
//! Mirrors `NeurofieldCommunityQ21API` from the C# API.

use std::sync::atomic::{AtomicBool, Ordering};

use crate::canbus_base::CanBusBase;
use crate::device::{Device, DeviceType};
use crate::eeg_api::{EegApi, IMPEDANCE_DATA_RX_SEQUENCE};
use crate::error::NeurofieldError;
use crate::pcan::UsbBus;

/// Number of EEG channels on the Q21.
pub const NUM_CHANNELS: usize = 20;

/// Sampling rate in Hz.
pub const SAMPLING_RATE: f64 = 256.0;

/// Injected current for impedance measurement (6 µA).
pub const INJECTED_CURRENT_FOR_IMPEDANCE: f64 = 6e-6;

/// On-board resistance on the impedance measurement line (12 kΩ).
pub const RESISTOR_LINE: f64 = 12_000.0;

/// EEG channel names in electrode-index order.
///
/// ```text
/// F7=0, T3=1, T4=2, T5=3, T6=4, Cz=5, Fz=6, Pz=7, F3=8, C4=9,
/// C3=10, P4=11, P3=12, O2=13, O1=14, F8=15, F4=16, Fp1=17, Fp2=18, HR=19
/// ```
pub const EEG_CHANNEL_NAMES: [&str; NUM_CHANNELS] = [
    "F7", "T3", "T4", "T5", "T6", "Cz", "Fz", "Pz", "F3", "C4",
    "C3", "P4", "P3", "O2", "O1", "F8", "F4", "Fp1", "Fp2", "HR",
];

/// A single scaled EEG sample.
#[derive(Debug, Clone)]
pub struct EegSample {
    /// 20 channels of EEG data in **microvolts**.
    pub data: [f64; NUM_CHANNELS],
    /// Reception timestamp of the first message in the sample sequence (µs).
    pub timestamp_us: u64,
}

/// A single impedance measurement for all 20 channels.
#[derive(Debug, Clone)]
pub struct ImpedanceSample {
    /// Impedance per channel in **ohms**.  Clamped to a minimum of 1000 Ω.
    pub data: [f64; NUM_CHANNELS],
}

/// Top-level API for the Neurofield Q21 EEG amplifier.
///
/// Provides scaled EEG data in microvolts and impedance readings in ohms.
///
/// # Example
///
/// ```rust,ignore
/// use neurofield::{Q21Api, UsbBus};
///
/// let mut api = Q21Api::new(UsbBus::USB1)?;
/// api.start_receiving_eeg()?;
///
/// for _ in 0..(256 * 4) {
///     let sample = api.get_single_sample()?;
///     println!("{:?}", sample.data);
/// }
///
/// api.abort_receiving_eeg()?;
/// ```
pub struct Q21Api {
    /// Mid-level EEG protocol layer.
    eeg: EegApi,
    /// ADC-to-microvolt conversion factor (includes polarity inversion).
    scale_factor: f64,
}

impl Q21Api {
    // ── Construction ─────────────────────────────────────────────────────

    /// Connect to a Q21 on the given PCAN-USB bus (~1 s discovery).
    ///
    /// Discovers all Neurofield devices, selects the first EEG amplifier,
    /// and computes the hardware-specific scale factor.
    pub fn new(bus: UsbBus) -> Result<Self, NeurofieldError> {
        let base = CanBusBase::new(bus)?;

        // Filter to EEG devices only
        let eeg_devices: Vec<Device> = base
            .connected_devices
            .iter()
            .filter(|d| d.device_type.is_eeg_device())
            .copied()
            .collect();

        if eeg_devices.is_empty() {
            // Release before returning error (mirrors C# behavior)
            let mut base = base;
            base.release();
            return Err(NeurofieldError::NoEegDevice);
        }

        let selected = eeg_devices[0];

        let scale_factor = match selected.device_type {
            DeviceType::Eeg21RevK => {
                // Q21 Rev-K: 4.5V / 2^24 / ADC gain(12), inverted polarity
                // 4500000 / 8388608 / 12 = 0.044703483581543
                -0.044703483581543
            }
            DeviceType::Eeg21RevA => {
                // Rev-A: 4.5V / 2^24 / ext_gain(6.6667) / ADC gain(2), inverted
                // 4500000 / 8388608 / 6.6667 / 2 = 0.040233115106831
                -0.040233115106831
            }
            _ => {
                // Other revisions: 4.5V / 2^24 / ext_gain(12.85) / ADC gain(2), inverted
                // 4500000 / 8388608 / 12.85 / 2 = 0.020873221905779
                -0.020873221905779
            }
        };

        Ok(Q21Api {
            eeg: EegApi {
                base,
                selected_device: selected,
            },
            scale_factor,
        })
    }

    // ── Device info ──────────────────────────────────────────────────────

    /// Returns `true` if the selected device supports impedance measurement (Rev-K).
    pub fn impedance_enabled(&self) -> bool {
        self.eeg.selected_device.device_type == DeviceType::Eeg21RevK
    }

    /// All EEG devices discovered on this bus.
    pub fn connected_eeg_devices(&self) -> Vec<Device> {
        self.eeg
            .base
            .connected_devices
            .iter()
            .filter(|d| d.device_type.is_eeg_device())
            .copied()
            .collect()
    }

    /// Device type of the selected EEG amplifier.
    pub fn eeg_device_type(&self) -> DeviceType {
        self.eeg.selected_device.device_type
    }

    /// Serial number of the selected EEG amplifier.
    pub fn eeg_device_serial(&self) -> u8 {
        self.eeg.selected_device.serial
    }

    /// Human-readable device info string.
    pub fn eeg_device_info(&self) -> String {
        format!(
            "Type: {:?}, Serial: {}",
            self.eeg.selected_device.device_type, self.eeg.selected_device.serial
        )
    }

    // ── EEG streaming ──────���─────────────────────────────────────────────

    /// Start EEG data streaming (up to 8 hours).
    pub fn start_receiving_eeg(&mut self) -> Result<(), NeurofieldError> {
        self.eeg.start_receiving_eeg()
    }

    /// Stop EEG data streaming and reset CAN buffers.
    pub fn abort_receiving_eeg(&mut self) -> Result<(), NeurofieldError> {
        self.eeg.abort_receiving_eeg()
    }

    /// Blink the front LED three times.
    pub fn blink(&mut self) -> Result<(), NeurofieldError> {
        self.eeg.blink()
    }

    // ── EEG data ─────────────────────────────────────────────────────────

    /// Receive a single 20-channel EEG sample scaled to **microvolts**.
    ///
    /// Blocks until one full 10-message sequence is received (~4 ms at 256 Hz).
    /// Times out after ~1.4 s if no data arrives.
    pub fn get_single_sample(&self) -> Result<EegSample, NeurofieldError> {
        let (raw, time) = self.eeg.receive_single_eeg_data_sample()?;

        let mut data = [0.0f64; NUM_CHANNELS];
        for ch in 0..NUM_CHANNELS {
            data[ch] = raw[ch] as f64 * self.scale_factor;
        }

        Ok(EegSample {
            data,
            timestamp_us: time,
        })
    }

    /// Receive a single raw (unscaled, 24-bit signed) 20-channel EEG sample.
    pub fn get_single_raw_sample(&self) -> Result<([i32; 20], u64), NeurofieldError> {
        self.eeg.receive_single_eeg_data_sample()
    }

    // ── Impedance ────────────────────────────────────────────────────────

    /// Switch to impedance measurement mode (Rev-K only).
    pub fn switch_to_impedance_mode(&mut self) -> Result<(), NeurofieldError> {
        self.eeg.switch_to_impedance_mode()
    }

    /// Switch to EEG measurement mode (Rev-K only).
    pub fn switch_to_eeg_mode(&mut self) -> Result<(), NeurofieldError> {
        self.eeg.switch_to_eeg_mode()
    }

    /// Receive a single impedance measurement for all 20 channels.
    ///
    /// `cancelled` can be used to abort early (mirrors `CancellationTokenSource` in C#).
    /// Pass `None` if cancellation is not needed.
    pub fn receive_single_impedance_sample(
        &self,
        cancelled: Option<&AtomicBool>,
    ) -> Result<ImpedanceSample, NeurofieldError> {
        let mut stage = 0usize;
        let mut raw_data = [(0i32, 0i32); NUM_CHANNELS];
        let mut impedance = [0.0f64; NUM_CHANNELS];

        loop {
            // Check cancellation
            if let Some(flag) = cancelled {
                if flag.load(Ordering::Relaxed) {
                    return Ok(ImpedanceSample { data: impedance });
                }
            }

            let (msg, header, _ts) = self
                .eeg
                .base
                .receive_single_message_from_device(&self.eeg.selected_device)?;

            if header.message_type == IMPEDANCE_DATA_RX_SEQUENCE[stage] {
                EegApi::extract_impedance_data_from_message(&msg, stage, &mut raw_data)?;
                stage += 1;
            } else {
                log::debug!(
                    "ADC data sequence error. Expected: {:?}. Received: {:?}",
                    IMPEDANCE_DATA_RX_SEQUENCE[stage],
                    header.message_type
                );
                stage = 0;
            }

            if stage >= 20 {
                break;
            }
        }

        const NUM_SAMPLES: f64 = 15.0;

        for ch in 0..NUM_CHANNELS {
            let offset_voltage = raw_data[ch].0 as f64 * 4.5 / NUM_SAMPLES / 8_388_608.0;
            let impedance_voltage = raw_data[ch].1 as f64 * 4.5 / NUM_SAMPLES / 8_388_608.0;

            impedance[ch] =
                (impedance_voltage - offset_voltage) / INJECTED_CURRENT_FOR_IMPEDANCE
                    - RESISTOR_LINE;

            // Prevent negative / very low impedance due to measurement errors.
            if impedance[ch] <= 1000.0 {
                impedance[ch] = 1000.0;
            }
        }

        Ok(ImpedanceSample { data: impedance })
    }

    // ── Misc ──────────────────────────��──────────────────────────────────

    /// Release the CAN-USB interface.
    pub fn release(&mut self) {
        self.eeg.release();
    }

    /// Returns a list of PCAN USB interfaces that are currently available.
    pub fn get_online_pcan_interfaces() -> Vec<UsbBus> {
        CanBusBase::get_online_pcan_interfaces()
    }
}

impl Drop for Q21Api {
    fn drop(&mut self) {
        self.release();
    }
}

impl std::fmt::Display for Q21Api {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Dev:{}", self.eeg.selected_device.serial)
    }
}
