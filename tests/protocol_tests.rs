//! Unit tests for protocol-level logic (no hardware required).

use neurofield::canbus_base::decode_extended_header;
use neurofield::device::DeviceType;
use neurofield::eeg_api::EegApi;
use neurofield::message_types::Q21MessageType;
use neurofield::pcan::{PcanMsg, PcanTimestamp, PCAN_MESSAGE_EXTENDED};

#[test]
fn test_decode_extended_header_eeg21_revk() {
    // slave_to_host=1, module=0xA5 (Eeg21RevK), serial=0x42, msg=0x03
    let id: u32 = 0x01_A5_42_03;
    let header = decode_extended_header(id).expect("should decode");
    assert!(header.slave_to_host);
    assert_eq!(header.module_type, DeviceType::Eeg21RevK);
    assert_eq!(header.serial, 0x42);
    assert_eq!(header.message_type, Q21MessageType::SendAtoDData);
}

#[test]
fn test_decode_extended_header_host_to_slave() {
    // slave_to_host=0
    let id: u32 = 0x00_A5_42_03;
    let header = decode_extended_header(id).expect("should decode");
    assert!(!header.slave_to_host);
}

#[test]
fn test_decode_extended_header_unknown_module() {
    // module=0xFF (unknown)
    let id: u32 = 0x01_FF_42_03;
    assert!(decode_extended_header(id).is_none());
}

#[test]
fn test_decode_extended_header_unknown_msg_type() {
    // msg_type=0x50 (unknown)
    let id: u32 = 0x01_A5_42_50;
    assert!(decode_extended_header(id).is_none());
}

#[test]
fn test_extract_ad_data_positive() {
    let msg = PcanMsg {
        id: 0,
        msg_type: PCAN_MESSAGE_EXTENDED,
        len: 6,
        data: [0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC, 0, 0],
    };
    let mut data = [0i32; 20];
    EegApi::extract_ad_data_from_message(&msg, 0, &mut data).unwrap();

    // 0x12 as i8 = 18, so: (18 << 16) | (0x34 << 8) | 0x56
    assert_eq!(data[0], (0x12_i8 as i32) << 16 | 0x34 << 8 | 0x56);
    assert_eq!(data[1], (0x78_i8 as i32) << 16 | 0x9A << 8 | 0xBC);
}

#[test]
fn test_extract_ad_data_negative() {
    // First byte 0xFF → i8 = -1, so sign-extends to all 1s in upper bits
    let msg = PcanMsg {
        id: 0,
        msg_type: PCAN_MESSAGE_EXTENDED,
        len: 6,
        data: [0xFF, 0xFF, 0xFF, 0x80, 0x00, 0x00, 0, 0],
    };
    let mut data = [0i32; 20];
    EegApi::extract_ad_data_from_message(&msg, 3, &mut data).unwrap();

    assert_eq!(data[6], -1); // 0xFFFFFF sign-extended to i32
    assert_eq!(data[7], -8_388_608); // 0x800000 sign-extended = -2^23
}

#[test]
fn test_extract_ad_data_wrong_length() {
    let msg = PcanMsg {
        id: 0,
        msg_type: PCAN_MESSAGE_EXTENDED,
        len: 8,
        data: [0; 8],
    };
    let mut data = [0i32; 20];
    assert!(EegApi::extract_ad_data_from_message(&msg, 0, &mut data).is_err());
}

#[test]
fn test_extract_impedance_data() {
    let msg = PcanMsg {
        id: 0,
        msg_type: PCAN_MESSAGE_EXTENDED,
        len: 8,
        data: [0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00],
    };
    let mut data = [(0i32, 0i32); 20];
    EegApi::extract_impedance_data_from_message(&msg, 5, &mut data).unwrap();

    assert_eq!(data[5].0, 0x00_01_00_00); // offset
    assert_eq!(data[5].1, 0x00_02_00_00); // voltage
}

#[test]
fn test_extract_impedance_data_negative() {
    let msg = PcanMsg {
        id: 0,
        msg_type: PCAN_MESSAGE_EXTENDED,
        len: 8,
        data: [0xFF, 0xFF, 0xFF, 0xFF, 0x80, 0x00, 0x00, 0x00],
    };
    let mut data = [(0i32, 0i32); 20];
    EegApi::extract_impedance_data_from_message(&msg, 0, &mut data).unwrap();

    assert_eq!(data[0].0, -1);
    assert_eq!(data[0].1, i32::MIN); // 0x80000000
}

#[test]
fn test_message_type_is_stream() {
    assert!(Q21MessageType::SendAtoDData.is_stream_message());
    assert!(Q21MessageType::SendAtoDDataMsg5.is_stream_message());
    assert!(Q21MessageType::SendAtoDDataMsg10.is_stream_message());
    assert!(Q21MessageType::ImpedanceCh1.is_stream_message());
    assert!(Q21MessageType::ImpedanceCh20.is_stream_message());
    assert!(!Q21MessageType::CANBusQuery.is_stream_message());
    assert!(!Q21MessageType::Q20Abort.is_stream_message());
}

#[test]
fn test_device_type_is_eeg() {
    assert!(DeviceType::Eeg20RevA.is_eeg_device());
    assert!(DeviceType::Eeg20RevB.is_eeg_device());
    assert!(DeviceType::Eeg21.is_eeg_device());
    assert!(DeviceType::Eeg21RevA.is_eeg_device());
    assert!(DeviceType::Eeg21RevK.is_eeg_device());
    assert!(!DeviceType::Host.is_eeg_device());
}

#[test]
fn test_timestamp_to_micros() {
    let ts = PcanTimestamp {
        millis: 1000,
        millis_overflow: 0,
        micros: 500,
    };
    assert_eq!(ts.to_micros(), 1_000_500);

    let ts2 = PcanTimestamp {
        millis: 0,
        millis_overflow: 1,
        micros: 0,
    };
    // overflow=1 means 2^32 ms have elapsed
    assert_eq!(ts2.to_micros(), (u32::MAX as u64 + 1) * 1000);
}

#[test]
fn test_device_type_from_byte_roundtrip() {
    for &b in &[0x00, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5] {
        let dt = DeviceType::from_byte(b).unwrap();
        assert_eq!(dt as u8, b);
    }
    assert!(DeviceType::from_byte(0xFF).is_none());
    assert!(DeviceType::from_byte(0x01).is_none());
}

#[test]
fn test_message_type_from_byte_roundtrip() {
    // Spot-check a few
    assert_eq!(Q21MessageType::from_byte(0x00), Some(Q21MessageType::CANBusQuery));
    assert_eq!(Q21MessageType::from_byte(0x03), Some(Q21MessageType::SendAtoDData));
    assert_eq!(Q21MessageType::from_byte(0x0D), Some(Q21MessageType::SendAtoDDataMsg10));
    assert_eq!(Q21MessageType::from_byte(0xA0), Some(Q21MessageType::ImpedanceCh1));
    assert_eq!(Q21MessageType::from_byte(0xB3), Some(Q21MessageType::ImpedanceCh20));
    assert_eq!(Q21MessageType::from_byte(0xFF), Some(Q21MessageType::Q20Abort));
    assert_eq!(Q21MessageType::from_byte(0x01), None);
    assert_eq!(Q21MessageType::from_byte(0x04), None);
}
