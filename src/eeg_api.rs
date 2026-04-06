//! Mid-level EEG protocol layer.
//!
//! Mirrors `NeurofieldCommunityEEGAPI` from the C# API.
//! Handles start/stop streaming, raw ADC data extraction, impedance data
//! extraction, and mode switching.

use std::thread;
use std::time::Duration;

use crate::canbus_base::CanBusBase;
use crate::device::{Device, DeviceType};
use crate::error::NeurofieldError;
use crate::message_types::Q21MessageType;
use crate::pcan::PcanMsg;

/// The 10-message sequence for one ADC sample (20 channels, 2 per message).
pub(crate) const AD_DATA_RX_SEQUENCE: [Q21MessageType; 10] = [
    Q21MessageType::SendAtoDData,
    Q21MessageType::SendAtoDDataMsg2,
    Q21MessageType::SendAtoDDataMsg3,
    Q21MessageType::SendAtoDDataMsg4,
    Q21MessageType::SendAtoDDataMsg5,
    Q21MessageType::SendAtoDDataMsg6,
    Q21MessageType::SendAtoDDataMsg7,
    Q21MessageType::SendAtoDDataMsg8,
    Q21MessageType::SendAtoDDataMsg9,
    Q21MessageType::SendAtoDDataMsg10,
];

/// The 20-message sequence for one impedance measurement (1 channel per message).
pub(crate) const IMPEDANCE_DATA_RX_SEQUENCE: [Q21MessageType; 20] = [
    Q21MessageType::ImpedanceCh1,
    Q21MessageType::ImpedanceCh2,
    Q21MessageType::ImpedanceCh3,
    Q21MessageType::ImpedanceCh4,
    Q21MessageType::ImpedanceCh5,
    Q21MessageType::ImpedanceCh6,
    Q21MessageType::ImpedanceCh7,
    Q21MessageType::ImpedanceCh8,
    Q21MessageType::ImpedanceCh9,
    Q21MessageType::ImpedanceCh10,
    Q21MessageType::ImpedanceCh11,
    Q21MessageType::ImpedanceCh12,
    Q21MessageType::ImpedanceCh13,
    Q21MessageType::ImpedanceCh14,
    Q21MessageType::ImpedanceCh15,
    Q21MessageType::ImpedanceCh16,
    Q21MessageType::ImpedanceCh17,
    Q21MessageType::ImpedanceCh18,
    Q21MessageType::ImpedanceCh19,
    Q21MessageType::ImpedanceCh20,
];

/// Mid-level EEG API — wraps [`CanBusBase`] and adds EEG-specific protocol logic.
pub struct EegApi {
    /// Low-level CAN transport.
    pub(crate) base: CanBusBase,
    /// The currently selected EEG device.
    pub(crate) selected_device: Device,
}

impl EegApi {
    // ── Data extraction (static helpers) ─────────────────────────────────

    /// Extract two 24-bit signed ADC samples from a 6-byte CAN message.
    ///
    /// Each message carries 2 channels × 3 bytes = 6 bytes.
    /// `stage` (0..9) determines the channel indices: `stage*2` and `stage*2+1`.
    pub fn extract_ad_data_from_message(
        msg: &PcanMsg,
        stage: usize,
        data: &mut [i32; 20],
    ) -> Result<(), NeurofieldError> {
        if msg.len != 6 {
            return Err(NeurofieldError::UnexpectedDataLength {
                expected: 6,
                got: msg.len as usize,
            });
        }
        let d = &msg.data;
        let i = stage * 2;

        // Sign-extend from 24-bit to 32-bit (cast first byte as i8 for sign).
        data[i] = ((d[0] as i8 as i32) << 16) | ((d[1] as i32) << 8) | (d[2] as i32);
        data[i + 1] = ((d[3] as i8 as i32) << 16) | ((d[4] as i32) << 8) | (d[5] as i32);

        Ok(())
    }

    /// Extract one impedance measurement (offset, impedance) from an 8-byte message.
    ///
    /// Each message carries 4 bytes offset voltage + 4 bytes impedance voltage
    /// as 32-bit big-endian signed integers.
    pub fn extract_impedance_data_from_message(
        msg: &PcanMsg,
        stage: usize,
        data: &mut [(i32, i32); 20],
    ) -> Result<(), NeurofieldError> {
        if msg.len != 8 {
            return Err(NeurofieldError::UnexpectedDataLength {
                expected: 8,
                got: msg.len as usize,
            });
        }
        let d = &msg.data;

        // Sign-extend from bytes (big-endian) — first byte cast as i8 for sign.
        let offset = ((d[0] as i8 as i32) << 24)
            | ((d[1] as i32) << 16)
            | ((d[2] as i32) << 8)
            | (d[3] as i32);
        let voltage = ((d[4] as i8 as i32) << 24)
            | ((d[5] as i32) << 16)
            | ((d[6] as i32) << 8)
            | (d[7] as i32);

        data[stage] = (offset, voltage);
        Ok(())
    }

    // ── Streaming control ────────────────────────────────────────────────

    /// Tell the device to start sending EEG samples.
    ///
    /// Requests up to 8 hours of continuous data (256 Hz × 60 × 60 × 8).
    pub fn start_receiving_eeg(&mut self) -> Result<(), NeurofieldError> {
        let n_samples: i32 = 256 * 60 * 60 * 8; // 8 hours

        let mut data = [0u8; 8];
        data[0] = (n_samples >> 24) as u8;
        data[1] = (n_samples >> 16) as u8;
        data[2] = (n_samples >> 8) as u8;
        data[3] = n_samples as u8;

        self.base.send_message(&self.selected_device, 0x03, &data)
    }

    /// Blink the front LED three times (requests 3 × 100 samples with 400 ms pauses).
    pub fn blink(&mut self) -> Result<(), NeurofieldError> {
        let n_samples: i32 = 100;

        let mut data = [0u8; 8];
        data[0] = (n_samples >> 24) as u8;
        data[1] = (n_samples >> 16) as u8;
        data[2] = (n_samples >> 8) as u8;
        data[3] = n_samples as u8;

        // C# sends 3 bursts of 100 samples with 400ms pauses between them
        // (no pause after the third burst).
        for i in 0..3 {
            self.base
                .send_message(&self.selected_device, 0x03, &data)?;
            for _ in 0..n_samples {
                let _ = self.receive_single_eeg_data_sample()?;
            }
            if i < 2 {
                thread::sleep(Duration::from_millis(400));
            }
        }
        Ok(())
    }

    /// Stop the device from sending EEG data and reset the CAN buffers.
    pub fn abort_receiving_eeg(&mut self) -> Result<(), NeurofieldError> {
        for _ in 0..50 {
            self.base
                .send_message(&self.selected_device, 0xFF, &[0u8; 8])?;
        }
        thread::sleep(Duration::from_millis(100));
        self.base.reset_buffers()
    }

    // ── Mode switching (Rev-K only) ──────────────────────────────────────

    /// Switch to impedance measurement mode (Rev-K only).
    pub fn switch_to_impedance_mode(&mut self) -> Result<(), NeurofieldError> {
        if self.selected_device.device_type != DeviceType::Eeg21RevK {
            return Err(NeurofieldError::NotSupported(
                "Only Q21 Rev-K supports switching between EEG / impedance measurement modes."
                    .into(),
            ));
        }
        let mut data = [0u8; 8];
        data[0] = 1;
        data[1] = 1;
        self.base.send_message(&self.selected_device, 0x20, &data)
    }

    /// Switch to EEG measurement mode (Rev-K only).
    pub fn switch_to_eeg_mode(&mut self) -> Result<(), NeurofieldError> {
        if self.selected_device.device_type != DeviceType::Eeg21RevK {
            return Err(NeurofieldError::NotSupported(
                "Only Q21 Rev-K supports switching between EEG / impedance measurement modes."
                    .into(),
            ));
        }
        let data = [0u8; 8];
        self.base.send_message(&self.selected_device, 0x20, &data)
    }

    // ── Raw EEG sample reception ─────────────────────────────────────────

    /// Receive a single raw (unscaled) 20-channel EEG sample.
    ///
    /// Returns 24-bit signed integers and the timestamp of the first message
    /// in the 10-message sequence.
    ///
    /// Channel order:
    /// `F7=0, T3=1, T4=2, T5=3, T6=4, Cz=5, Fz=6, Pz=7, F3=8, C4=9,
    ///  C3=10, P4=11, P3=12, O2=13, O1=14, F8=15, F4=16, Fp1=17, Fp2=18, HR=19`
    pub fn receive_single_eeg_data_sample(
        &self,
    ) -> Result<([i32; 20], u64), NeurofieldError> {
        let mut stage = 0usize;
        let mut data = [0i32; 20];
        let mut time: u64 = 0;

        loop {
            let (msg, header, timestamp) = self
                .base
                .receive_single_message_from_device(&self.selected_device)?;

            if header.message_type == AD_DATA_RX_SEQUENCE[stage] {
                Self::extract_ad_data_from_message(&msg, stage, &mut data)?;
                if stage == 0 {
                    time = timestamp;
                }
                stage += 1;
            } else {
                log::debug!(
                    "ADC data sequence error. Expected: {:?}. Received: {:?}",
                    AD_DATA_RX_SEQUENCE[stage],
                    header.message_type
                );
                stage = 0;
            }

            if stage >= 10 {
                return Ok((data, time));
            }
        }
    }

    /// Get the selected EEG device type.
    pub fn eeg_device_type(&self) -> DeviceType {
        self.selected_device.device_type
    }

    /// Release the CAN-USB interface.
    pub fn release(&mut self) {
        self.base.release();
    }
}
