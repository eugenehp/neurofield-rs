//! Minimal cross-platform FFI bindings for the PCAN-Basic API.
//!
//! Loads `PCANBasic.dll` / `libPCANBasic.so` / `libPCANBasic.dylib` at runtime
//! via `libloading`.  All integer types are explicit (`u32`, `u16`, `u8`) so
//! the same code compiles identically on Windows, Linux, and macOS.

use std::ffi::c_void;
use std::sync::OnceLock;

use crate::error::NeurofieldError;

// ── Constants ────────────────────────────────────────────────────────────────

// USB channel handles (PCAN_USBBUS1 .. PCAN_USBBUS8)
pub const PCAN_USBBUS1: u16 = 0x51;
pub const PCAN_USBBUS2: u16 = 0x52;
pub const PCAN_USBBUS3: u16 = 0x53;
pub const PCAN_USBBUS4: u16 = 0x54;
pub const PCAN_USBBUS5: u16 = 0x55;
pub const PCAN_USBBUS6: u16 = 0x56;
pub const PCAN_USBBUS7: u16 = 0x57;
pub const PCAN_USBBUS8: u16 = 0x58;

// Baud rate
pub const PCAN_BAUD_500K: u16 = 0x001C;

// Status codes (PCAN-Basic v4 values — must match the C# Peak.Can.Basic NuGet)
pub const PCAN_ERROR_OK:           u32 = 0x00000;
pub const PCAN_ERROR_XMTFULL:      u32 = 0x00001;
pub const PCAN_ERROR_OVERRUN:      u32 = 0x00002;
pub const PCAN_ERROR_BUSLIGHT:     u32 = 0x00004;
pub const PCAN_ERROR_BUSHEAVY:     u32 = 0x00008;
pub const PCAN_ERROR_BUSOFF:       u32 = 0x00010;
pub const PCAN_ERROR_QRCVEMPTY:    u32 = 0x00020;
pub const PCAN_ERROR_QOVERRUN:     u32 = 0x00040;
pub const PCAN_ERROR_QXMTFULL:     u32 = 0x00080;
pub const PCAN_ERROR_BUSPASSIVE:   u32 = 0x40000;
pub const PCAN_ERROR_ILLOPERATION: u32 = 0x8000000;

// Parameter IDs for CAN_GetValue / CAN_SetValue (v4 values)
pub const PCAN_CHANNEL_CONDITION: u8 = 0x0D;
pub const PCAN_TRACE_SIZE:        u8 = 0x13;
pub const PCAN_TRACE_CONFIGURE:   u8 = 0x14;

// Channel condition values
pub const PCAN_CHANNEL_UNAVAILABLE: u32 = 0x00;
pub const PCAN_CHANNEL_AVAILABLE:   u32 = 0x01;
pub const PCAN_CHANNEL_OCCUPIED:    u32 = 0x02;

// Trace configuration flags
pub const TRACE_FILE_SINGLE:    u32 = 0x00;
pub const TRACE_FILE_OVERWRITE: u32 = 0x80;

// Message types
pub const PCAN_MESSAGE_STANDARD: u8 = 0x00;
pub const PCAN_MESSAGE_EXTENDED: u8 = 0x02;

// ── FFI structures ───────────────────────────────────────────────────────────

/// A CAN 2.0 message (matches TPCANMsg on all platforms).
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct PcanMsg {
    pub id:       u32,
    pub msg_type: u8,
    pub len:      u8,
    pub data:     [u8; 8],
}

impl Default for PcanMsg {
    fn default() -> Self {
        Self {
            id: 0,
            msg_type: PCAN_MESSAGE_STANDARD,
            len: 0,
            data: [0u8; 8],
        }
    }
}

/// Timestamp returned by CAN_Read (matches TPCANTimestamp on all platforms).
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct PcanTimestamp {
    /// Milliseconds counter.
    pub millis: u32,
    /// Number of times the millis counter has overflowed.
    pub millis_overflow: u16,
    /// Microseconds within the current millisecond (0–999).
    pub micros: u16,
}

impl PcanTimestamp {
    /// Convert to a total microsecond value.
    pub fn to_micros(&self) -> u64 {
        let total_millis =
            (self.millis_overflow as u64) * (u32::MAX as u64 + 1) + (self.millis as u64);
        total_millis * 1000 + self.micros as u64
    }
}

// ── USB bus enum ─────────────────────────────────────────────────────────────

/// PCAN USB bus identifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UsbBus {
    USB1,
    USB2,
    USB3,
    USB4,
    USB5,
    USB6,
    USB7,
    USB8,
}

impl UsbBus {
    pub fn handle(self) -> u16 {
        match self {
            Self::USB1 => PCAN_USBBUS1,
            Self::USB2 => PCAN_USBBUS2,
            Self::USB3 => PCAN_USBBUS3,
            Self::USB4 => PCAN_USBBUS4,
            Self::USB5 => PCAN_USBBUS5,
            Self::USB6 => PCAN_USBBUS6,
            Self::USB7 => PCAN_USBBUS7,
            Self::USB8 => PCAN_USBBUS8,
        }
    }

    /// All eight USB buses.
    pub const ALL: [UsbBus; 8] = [
        Self::USB1, Self::USB2, Self::USB3, Self::USB4,
        Self::USB5, Self::USB6, Self::USB7, Self::USB8,
    ];
}

// ── Dynamic library wrapper ──────────────────────────────────────────────────

/// Dynamically-loaded PCAN-Basic library handle.
pub struct PcanLib {
    _lib: libloading::Library,

    // Function pointers
    fn_initialize:   unsafe extern "C" fn(u16, u16, u8, u32, u32) -> u32,
    fn_uninitialize: unsafe extern "C" fn(u16) -> u32,
    fn_reset:        unsafe extern "C" fn(u16) -> u32,
    fn_read:         unsafe extern "C" fn(u16, *mut PcanMsg, *mut PcanTimestamp) -> u32,
    fn_write:        unsafe extern "C" fn(u16, *mut PcanMsg) -> u32,
    fn_get_value:    unsafe extern "C" fn(u16, u8, *mut c_void, u32) -> u32,
    fn_set_value:    unsafe extern "C" fn(u16, u8, *mut c_void, u32) -> u32,
}

// SAFETY: The PCAN-Basic library is designed to be called from any thread
// (it manages its own internal synchronisation).
unsafe impl Send for PcanLib {}
unsafe impl Sync for PcanLib {}

impl PcanLib {
    /// Load the PCAN-Basic shared library.
    fn load() -> Result<Self, NeurofieldError> {
        let lib_name = libloading::library_filename("PCANBasic");
        let lib = unsafe { libloading::Library::new(&lib_name) }.map_err(|e| {
            NeurofieldError::InterfaceNotAvailable {
                reason: format!("Could not load PCANBasic library ({:?}): {}", lib_name, e),
            }
        })?;

        unsafe {
            let fn_initialize = *lib
                .get::<unsafe extern "C" fn(u16, u16, u8, u32, u32) -> u32>(b"CAN_Initialize\0")
                .map_err(|e| NeurofieldError::InterfaceNotAvailable {
                    reason: format!("CAN_Initialize not found: {}", e),
                })?;
            let fn_uninitialize = *lib
                .get::<unsafe extern "C" fn(u16) -> u32>(b"CAN_Uninitialize\0")
                .map_err(|e| NeurofieldError::InterfaceNotAvailable {
                    reason: format!("CAN_Uninitialize not found: {}", e),
                })?;
            let fn_reset = *lib
                .get::<unsafe extern "C" fn(u16) -> u32>(b"CAN_Reset\0")
                .map_err(|e| NeurofieldError::InterfaceNotAvailable {
                    reason: format!("CAN_Reset not found: {}", e),
                })?;
            let fn_read = *lib
                .get::<unsafe extern "C" fn(u16, *mut PcanMsg, *mut PcanTimestamp) -> u32>(
                    b"CAN_Read\0",
                )
                .map_err(|e| NeurofieldError::InterfaceNotAvailable {
                    reason: format!("CAN_Read not found: {}", e),
                })?;
            let fn_write = *lib
                .get::<unsafe extern "C" fn(u16, *mut PcanMsg) -> u32>(b"CAN_Write\0")
                .map_err(|e| NeurofieldError::InterfaceNotAvailable {
                    reason: format!("CAN_Write not found: {}", e),
                })?;
            let fn_get_value = *lib
                .get::<unsafe extern "C" fn(u16, u8, *mut c_void, u32) -> u32>(
                    b"CAN_GetValue\0",
                )
                .map_err(|e| NeurofieldError::InterfaceNotAvailable {
                    reason: format!("CAN_GetValue not found: {}", e),
                })?;
            let fn_set_value = *lib
                .get::<unsafe extern "C" fn(u16, u8, *mut c_void, u32) -> u32>(
                    b"CAN_SetValue\0",
                )
                .map_err(|e| NeurofieldError::InterfaceNotAvailable {
                    reason: format!("CAN_SetValue not found: {}", e),
                })?;

            Ok(PcanLib {
                _lib: lib,
                fn_initialize,
                fn_uninitialize,
                fn_reset,
                fn_read,
                fn_write,
                fn_get_value,
                fn_set_value,
            })
        }
    }

    // ── Wrapped API ──────────────────────────────────────────────────────

    pub fn initialize(&self, channel: u16, baud: u16) -> u32 {
        unsafe { (self.fn_initialize)(channel, baud, 0, 0, 0) }
    }

    pub fn uninitialize(&self, channel: u16) -> u32 {
        unsafe { (self.fn_uninitialize)(channel) }
    }

    pub fn reset(&self, channel: u16) -> u32 {
        unsafe { (self.fn_reset)(channel) }
    }

    pub fn read(&self, channel: u16, msg: &mut PcanMsg, ts: &mut PcanTimestamp) -> u32 {
        unsafe { (self.fn_read)(channel, msg as *mut _, ts as *mut _) }
    }

    pub fn write(&self, channel: u16, msg: &mut PcanMsg) -> u32 {
        unsafe { (self.fn_write)(channel, msg as *mut _) }
    }

    pub fn get_value(&self, channel: u16, param: u8, buf: &mut [u8]) -> u32 {
        unsafe {
            (self.fn_get_value)(
                channel,
                param,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as u32,
            )
        }
    }

    pub fn set_value(&self, channel: u16, param: u8, buf: &mut [u8]) -> u32 {
        unsafe {
            (self.fn_set_value)(
                channel,
                param,
                buf.as_mut_ptr() as *mut c_void,
                buf.len() as u32,
            )
        }
    }
}

// ── Singleton accessor ───────────────────────────────────────────────────────

static PCAN_LIB: OnceLock<Result<PcanLib, String>> = OnceLock::new();

/// Get the global PCAN-Basic library handle.
pub fn pcan_lib() -> Result<&'static PcanLib, NeurofieldError> {
    PCAN_LIB
        .get_or_init(|| PcanLib::load().map_err(|e| e.to_string()))
        .as_ref()
        .map_err(|e| NeurofieldError::InterfaceNotAvailable {
            reason: e.clone(),
        })
}

// ── Helper: check channel condition ──────────────────────────────────────────

/// Query the channel condition for a given USB bus handle.
pub fn channel_condition(bus: UsbBus) -> Result<u32, NeurofieldError> {
    let lib = pcan_lib()?;
    let mut data = [0u8; 4];
    let status = lib.get_value(bus.handle(), PCAN_CHANNEL_CONDITION, &mut data);
    if status != PCAN_ERROR_OK {
        return Err(NeurofieldError::InterfaceNotAvailable {
            reason: format!("CAN_GetValue(CHANNEL_CONDITION) failed: 0x{:08X}", status),
        });
    }
    Ok(u32::from_le_bytes(data))
}
