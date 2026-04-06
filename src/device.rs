//! Neurofield device types and identification.

/// Known Neurofield device hardware revisions.
///
/// Byte values match the CAN-bus module-type field in the extended header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum DeviceType {
    /// Host computer (not a real device — used in protocol).
    Host       = 0x00,
    /// Neurofield 20-channel EEG Rev A.
    Eeg20RevA  = 0xA1,
    /// Neurofield 20-channel EEG Rev B.
    Eeg20RevB  = 0xA2,
    /// Neurofield 21-channel EEG.
    Eeg21      = 0xA3,
    /// Neurofield 21-channel EEG Rev A.
    Eeg21RevA  = 0xA4,
    /// FPGA-less Q21 (Rev K).
    Eeg21RevK  = 0xA5,
}

impl DeviceType {
    /// Try to convert a raw byte to a known device type.
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(Self::Host),
            0xA1 => Some(Self::Eeg20RevA),
            0xA2 => Some(Self::Eeg20RevB),
            0xA3 => Some(Self::Eeg21),
            0xA4 => Some(Self::Eeg21RevA),
            0xA5 => Some(Self::Eeg21RevK),
            _    => None,
        }
    }

    /// Returns `true` if this device type is one of the supported EEG amplifiers.
    pub fn is_eeg_device(self) -> bool {
        matches!(
            self,
            Self::Eeg20RevA | Self::Eeg20RevB | Self::Eeg21 | Self::Eeg21RevA | Self::Eeg21RevK
        )
    }
}

/// A discovered Neurofield device on the CAN bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Device {
    /// Hardware revision / type.
    pub device_type: DeviceType,
    /// Serial number (single byte).
    pub serial: u8,
}

impl Device {
    /// Two devices are considered the same if type and serial match.
    pub fn is_same(&self, other: &Device) -> bool {
        self.device_type == other.device_type && self.serial == other.serial
    }
}

impl std::fmt::Display for Device {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Dev:{}", self.serial)
    }
}
