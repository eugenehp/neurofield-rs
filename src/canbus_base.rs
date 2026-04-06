//! Low-level CAN-bus transport for Neurofield devices.
//!
//! Mirrors `NeurofieldCommunityCANBUSApiBase` from the C# API.

use std::thread;
use std::time::Duration;

use crate::device::{Device, DeviceType};
use crate::error::NeurofieldError;
use crate::message_types::{ExtendedHeader, Q21MessageType};
use crate::pcan::{
    self, PcanLib, PcanMsg, PcanTimestamp, UsbBus,
    PCAN_BAUD_500K, PCAN_CHANNEL_AVAILABLE, PCAN_ERROR_BUSLIGHT,
    PCAN_ERROR_BUSHEAVY, PCAN_ERROR_BUSOFF, PCAN_ERROR_ILLOPERATION,
    PCAN_ERROR_OK, PCAN_ERROR_QRCVEMPTY, PCAN_MESSAGE_EXTENDED,
    PCAN_TRACE_CONFIGURE, PCAN_TRACE_SIZE, TRACE_FILE_OVERWRITE,
    TRACE_FILE_SINGLE,
};

/// Low-level CAN-bus transport — owns the channel handle and the list of
/// discovered devices.
pub struct CanBusBase {
    /// The PCAN channel handle (0 = released).
    handle: u16,
    /// Whether the channel is currently initialised.
    initialised: bool,
    /// All Neurofield devices discovered during construction.
    pub(crate) connected_devices: Vec<Device>,
}

impl CanBusBase {
    // ── Construction ─────────────────────────────────────────────────────

    /// Open the PCAN-USB interface, discover Neurofield devices (~1 s).
    ///
    /// Equivalent to `NeurofieldCommunityCANBUSApiBase(PcanChannel)`.
    pub fn new(bus: UsbBus) -> Result<Self, NeurofieldError> {
        let lib = pcan::pcan_lib()?;

        // Check channel availability (bitwise AND, matching C# behavior)
        let condition = pcan::channel_condition(bus)?;
        if (condition & PCAN_CHANNEL_AVAILABLE) == 0 {
            return Err(NeurofieldError::InterfaceNotAvailable {
                reason: format!("Channel {:?} is NOT available (condition=0x{:X})", bus, condition),
            });
        }

        let handle = bus.handle();

        // Initialise at 500 kbit/s
        let status = lib.initialize(handle, PCAN_BAUD_500K);
        if status != PCAN_ERROR_OK {
            return Err(NeurofieldError::InterfaceNotAvailable {
                reason: format!("CAN_Initialize failed: 0x{:08X}", status),
            });
        }

        // From this point, the channel is initialised.  If anything fails
        // we must uninitialise before returning — mirrors the C# try/catch
        // that calls Release() on any exception after Initialize.
        let mut base = CanBusBase {
            handle,
            initialised: true,
            connected_devices: Vec::new(),
        };

        // Configure trace file (5 MB, single file + overwrite)
        if let Err(e) = configure_trace(lib, handle) {
            base.release();
            return Err(e);
        }

        // Send query broadcast
        base.send_message_raw(None, Q21MessageType::CANBusQuery as u8, &[0u8; 8])?;

        // Wait 1 s for responses
        thread::sleep(Duration::from_secs(1));

        // Drain receive queue and collect device responses
        loop {
            let mut msg = PcanMsg::default();
            let mut ts = PcanTimestamp::default();
            let status = lib.read(handle, &mut msg, &mut ts);

            if status == PCAN_ERROR_ILLOPERATION || status == PCAN_ERROR_QRCVEMPTY {
                break;
            }
            if status == PCAN_ERROR_BUSLIGHT {
                base.release();
                return Err(NeurofieldError::BusError(
                    "Bus light error during discovery — is the device on?".into(),
                ));
            }
            // C# falls through to _processQueryAnswer on all other statuses
            // (including OK, BusHeavy, Overrun, etc.)

            match process_query_answer(&msg) {
                Ok(Some(dev)) => base.connected_devices.push(dev),
                Ok(None) => {} // stream message or unknown header — skip
                Err(e) => {
                    base.release();
                    return Err(e);
                }
            }
        }

        Ok(base)
    }

    // ── Sending ──────────────────────────────────────────────────────────

    /// Send a CAN message to a specific device (or broadcast if `device` is `None`).
    pub(crate) fn send_message(
        &mut self,
        device: &Device,
        msg_type: u8,
        data: &[u8; 8],
    ) -> Result<(), NeurofieldError> {
        self.send_message_raw(Some(device), msg_type, data)
    }

    fn send_message_raw(
        &mut self,
        device: Option<&Device>,
        msg_type: u8,
        data: &[u8; 8],
    ) -> Result<(), NeurofieldError> {
        let lib = pcan::pcan_lib()?;

        let id: u32 = match device {
            Some(dev) => {
                ((dev.device_type as u32) << 16)
                    | ((dev.serial as u32) << 8)
                    | (msg_type as u32)
            }
            None => 0,
        };

        let mut msg = PcanMsg {
            id,
            msg_type: PCAN_MESSAGE_EXTENDED,
            len: 8,
            data: *data,
        };

        let status = lib.write(self.handle, &mut msg);

        if status == PCAN_ERROR_OK {
            return Ok(());
        }

        if status == PCAN_ERROR_BUSOFF {
            return self.recover_bus_off(lib, &mut msg);
        }

        Err(NeurofieldError::SendFailed(format!(
            "Could not send message over PCAN-USB: 0x{:08X}",
            status
        )))
    }

    /// Bus-off recovery loop — mirrors the C# `BusOff` retry logic exactly.
    ///
    /// C# checks each step and does `tryCount++; continue` on failure,
    /// skipping subsequent steps in that iteration.
    fn recover_bus_off(
        &mut self,
        lib: &PcanLib,
        msg: &mut PcanMsg,
    ) -> Result<(), NeurofieldError> {
        for _ in 0..30 {
            let status = lib.uninitialize(self.handle);
            if status != PCAN_ERROR_OK {
                continue;
            }

            thread::sleep(Duration::from_millis(10));

            let status = lib.initialize(self.handle, PCAN_BAUD_500K);
            if status != PCAN_ERROR_OK {
                continue;
            }

            thread::sleep(Duration::from_millis(10));

            // C# _configureTraceFile() can throw here, which escapes the
            // while loop and propagates up.  We match that: propagate error.
            configure_trace(lib, self.handle)?;

            let status = lib.write(self.handle, msg);
            if status == PCAN_ERROR_OK {
                self.initialised = true;
                return Ok(());
            }
        }
        Err(NeurofieldError::SendFailed(
            "Bus-off recovery failed after 30 attempts".into(),
        ))
    }

    // ── Receiving ──────────────���─────────────────────────────────────────

    /// Receive a single CAN message from a specific device, discarding messages
    /// from other devices.  Blocks up to ~1.4 s, then returns `Err(Timeout)`.
    pub(crate) fn receive_single_message_from_device(
        &self,
        device: &Device,
    ) -> Result<(PcanMsg, ExtendedHeader, u64), NeurofieldError> {
        loop {
            let (msg, ts) = self.receive_single_canbus_message()?;

            // C# uses exact equality: `msgRx.MsgType != MessageType.Extended`
            if msg.msg_type != PCAN_MESSAGE_EXTENDED {
                continue;
            }

            let header = match decode_extended_header(msg.id) {
                Some(h) => h,
                None => continue,
            };

            if !header.slave_to_host
                || header.serial != device.serial
                || header.module_type != device.device_type
            {
                continue;
            }

            return Ok((msg, header, ts.to_micros()));
        }
    }

    /// Receive one raw CAN message with a ~1.4 s timeout.
    ///
    /// Mirrors `_receiveSingleCANBUSMessage` — on ReceiveQueueEmpty, waits 1 ms
    /// and retries (up to 1400 times ≈ 1.4 s).  On any other non-error status
    /// the message is returned as-is (matching C# `else { break; }` behavior).
    fn receive_single_canbus_message(&self) -> Result<(PcanMsg, PcanTimestamp), NeurofieldError> {
        let lib = pcan::pcan_lib()?;
        let mut timeout_counter = 0u32;

        loop {
            let mut msg = PcanMsg::default();
            let mut ts = PcanTimestamp::default();
            let status = lib.read(self.handle, &mut msg, &mut ts);

            if status == PCAN_ERROR_ILLOPERATION {
                return Err(NeurofieldError::SendFailed(format!(
                    "CAN_Read: invalid operation (0x{:08X})",
                    status
                )));
            }

            if status == PCAN_ERROR_BUSLIGHT || status == PCAN_ERROR_BUSHEAVY {
                return Err(NeurofieldError::BusError(
                    "Bus error — is the device still on?".into(),
                ));
            }

            if status == PCAN_ERROR_QRCVEMPTY {
                thread::sleep(Duration::from_millis(1));
                timeout_counter += 1;
                if timeout_counter >= 1400 {
                    return Err(NeurofieldError::Timeout);
                }
            } else {
                // PCAN_ERROR_OK or any other status: return the message
                // (matches C# `else { timestamp = timestamp1; break; }`)
                return Ok((msg, ts));
            }
        }
    }

    // ── Misc ─────────────────────────────────��───────────────────────────

    /// Reset the CAN transmit/receive buffers.
    pub(crate) fn reset_buffers(&mut self) -> Result<(), NeurofieldError> {
        let lib = pcan::pcan_lib()?;
        lib.reset(self.handle);
        Ok(())
    }

    /// Release the CAN-USB interface.
    pub fn release(&mut self) {
        if self.initialised {
            if let Ok(lib) = pcan::pcan_lib() {
                lib.reset(self.handle);
                lib.uninitialize(self.handle);
            }
            self.initialised = false;
        }
    }

    /// Returns a list of PCAN USB interfaces that are currently available.
    ///
    /// Mirrors `GetOnlinePcanInterfaces()` — uses bitwise AND to check the
    /// `ChannelAvailable` flag, matching the C# `(condition & Available) != 0`.
    pub fn get_online_pcan_interfaces() -> Vec<UsbBus> {
        let mut interfaces = Vec::new();
        for &bus in &UsbBus::ALL {
            if let Ok(cond) = pcan::channel_condition(bus) {
                if (cond & PCAN_CHANNEL_AVAILABLE) != 0 {
                    interfaces.push(bus);
                }
            }
        }
        interfaces
    }
}

impl Drop for CanBusBase {
    fn drop(&mut self) {
        self.release();
    }
}

// ─�� Helpers ───��──────────────────────────────────────────────────────────────

/// Configure trace: 5 MB, single file + overwrite.
fn configure_trace(lib: &PcanLib, handle: u16) -> Result<(), NeurofieldError> {
    // Set trace size to 5 MB
    let mut size_buf = 5u32.to_le_bytes();
    let status = lib.set_value(handle, PCAN_TRACE_SIZE, &mut size_buf);
    if status != PCAN_ERROR_OK {
        return Err(NeurofieldError::InterfaceNotAvailable {
            reason: format!("CAN_SetValue(TRACE_SIZE) failed: 0x{:08X}", status),
        });
    }

    // Configure: single file + overwrite
    let mut cfg_buf = (TRACE_FILE_SINGLE | TRACE_FILE_OVERWRITE).to_le_bytes();
    let status = lib.set_value(handle, PCAN_TRACE_CONFIGURE, &mut cfg_buf);
    if status != PCAN_ERROR_OK {
        return Err(NeurofieldError::InterfaceNotAvailable {
            reason: format!("CAN_SetValue(TRACE_CONFIGURE) failed: 0x{:08X}", status),
        });
    }

    Ok(())
}

/// Decode a 29-bit CAN extended ID into a Neurofield header.
pub fn decode_extended_header(id: u32) -> Option<ExtendedHeader> {
    let module_byte = ((id & 0x00FF_0000) >> 16) as u8;
    let module_type = DeviceType::from_byte(module_byte)?;

    let msg_byte = (id & 0xFF) as u8;
    let message_type = Q21MessageType::from_byte(msg_byte)?;

    Some(ExtendedHeader {
        slave_to_host: (id & 0x0100_0000) != 0,
        message_type,
        module_type,
        serial: ((id & 0x0000_FF00) >> 8) as u8,
    })
}

/// Process a query answer frame, returning a [`Device`] if valid.
///
/// Mirrors `_processQueryAnswer` — returns `Err` for invalid message type,
/// non-slave messages, and non-query answers (matching C# exception behavior).
/// Returns `Ok(None)` for stream messages (silently discarded) and unknown
/// headers. Returns `Ok(Some(device))` for valid query responses.
fn process_query_answer(msg: &PcanMsg) -> Result<Option<Device>, NeurofieldError> {
    // C# uses exact equality: `msgRx.MsgType != MessageType.Extended`
    if msg.msg_type != PCAN_MESSAGE_EXTENDED {
        return Err(NeurofieldError::BusError(
            "Invalid Message Type received. Is the EEG AMP powered on?".into(),
        ));
    }

    let header = match decode_extended_header(msg.id) {
        Some(h) => h,
        None => return Ok(None),
    };

    if !header.slave_to_host {
        return Err(NeurofieldError::BusError("Not a Slave message.".into()));
    }

    // Discard stream messages silently
    if header.message_type.is_stream_message() {
        return Ok(None);
    }

    if header.message_type != Q21MessageType::CANBusQuery {
        return Err(NeurofieldError::BusError("Not a Query Answer.".into()));
    }

    Ok(Some(Device {
        device_type: header.module_type,
        serial: header.serial,
    }))
}
