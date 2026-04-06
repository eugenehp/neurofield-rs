//! CAN-bus message types used by the Q21 protocol.

use crate::device::DeviceType;

/// All known Q21 CAN-bus message types.
///
/// Byte values match the least-significant byte of the CAN extended ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Q21MessageType {
    CANBusQuery       = 0x00,

    SendAtoDData      = 0x03,
    SendAtoDDataMsg2  = 0x05,
    SendAtoDDataMsg3  = 0x06,
    SendAtoDDataMsg4  = 0x07,
    SendAtoDDataMsg5  = 0x08,
    SendAtoDDataMsg6  = 0x09,
    SendAtoDDataMsg7  = 0x0A,
    SendAtoDDataMsg8  = 0x0B,
    SendAtoDDataMsg9  = 0x0C,
    SendAtoDDataMsg10 = 0x0D,

    ImpedanceCh1  = 0xA0,
    ImpedanceCh2  = 0xA1,
    ImpedanceCh3  = 0xA2,
    ImpedanceCh4  = 0xA3,
    ImpedanceCh5  = 0xA4,
    ImpedanceCh6  = 0xA5,
    ImpedanceCh7  = 0xA6,
    ImpedanceCh8  = 0xA7,
    ImpedanceCh9  = 0xA8,
    ImpedanceCh10 = 0xA9,
    ImpedanceCh11 = 0xAA,
    ImpedanceCh12 = 0xAB,
    ImpedanceCh13 = 0xAC,
    ImpedanceCh14 = 0xAD,
    ImpedanceCh15 = 0xAE,
    ImpedanceCh16 = 0xAF,
    ImpedanceCh17 = 0xB0,
    ImpedanceCh18 = 0xB1,
    ImpedanceCh19 = 0xB2,
    ImpedanceCh20 = 0xB3,

    Q20Abort = 0xFF,
}

impl Q21MessageType {
    /// Try to convert a raw byte to a known message type.
    pub fn from_byte(b: u8) -> Option<Self> {
        match b {
            0x00 => Some(Self::CANBusQuery),

            0x03 => Some(Self::SendAtoDData),
            0x05 => Some(Self::SendAtoDDataMsg2),
            0x06 => Some(Self::SendAtoDDataMsg3),
            0x07 => Some(Self::SendAtoDDataMsg4),
            0x08 => Some(Self::SendAtoDDataMsg5),
            0x09 => Some(Self::SendAtoDDataMsg6),
            0x0A => Some(Self::SendAtoDDataMsg7),
            0x0B => Some(Self::SendAtoDDataMsg8),
            0x0C => Some(Self::SendAtoDDataMsg9),
            0x0D => Some(Self::SendAtoDDataMsg10),

            0xA0 => Some(Self::ImpedanceCh1),
            0xA1 => Some(Self::ImpedanceCh2),
            0xA2 => Some(Self::ImpedanceCh3),
            0xA3 => Some(Self::ImpedanceCh4),
            0xA4 => Some(Self::ImpedanceCh5),
            0xA5 => Some(Self::ImpedanceCh6),
            0xA6 => Some(Self::ImpedanceCh7),
            0xA7 => Some(Self::ImpedanceCh8),
            0xA8 => Some(Self::ImpedanceCh9),
            0xA9 => Some(Self::ImpedanceCh10),
            0xAA => Some(Self::ImpedanceCh11),
            0xAB => Some(Self::ImpedanceCh12),
            0xAC => Some(Self::ImpedanceCh13),
            0xAD => Some(Self::ImpedanceCh14),
            0xAE => Some(Self::ImpedanceCh15),
            0xAF => Some(Self::ImpedanceCh16),
            0xB0 => Some(Self::ImpedanceCh17),
            0xB1 => Some(Self::ImpedanceCh18),
            0xB2 => Some(Self::ImpedanceCh19),
            0xB3 => Some(Self::ImpedanceCh20),

            0xFF => Some(Self::Q20Abort),

            _ => None,
        }
    }

    /// Returns `true` if this message type belongs to the ADC or impedance data stream.
    pub fn is_stream_message(self) -> bool {
        if self == Self::SendAtoDData {
            return true;
        }
        let b = self as u8;
        // SendAtoDDataMsg2 (0x05) ..= SendAtoDDataMsg10 (0x0D)
        if b >= Self::SendAtoDDataMsg2 as u8 && b <= Self::SendAtoDDataMsg10 as u8 {
            return true;
        }
        // ImpedanceCh1 (0xA0) ..= ImpedanceCh20 (0xB3)
        if b >= Self::ImpedanceCh1 as u8 && b <= Self::ImpedanceCh20 as u8 {
            return true;
        }
        false
    }
}

/// Decoded Neurofield extended CAN header (29-bit ID).
///
/// Layout: `[bit 24: slave→host] [bits 23..16: module type] [bits 15..8: serial] [bits 7..0: msg type]`
#[derive(Debug, Clone, Copy)]
pub struct ExtendedHeader {
    /// `true` if this message originated from a device (slave → host).
    pub slave_to_host: bool,
    /// The message type.
    pub message_type: Q21MessageType,
    /// The hardware type of the originating module.
    pub module_type: DeviceType,
    /// Serial number of the originating module.
    pub serial: u8,
}
