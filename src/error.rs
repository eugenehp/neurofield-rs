//! Error types for the Neurofield API.

/// All errors that can occur within the Neurofield API.
#[derive(Debug, thiserror::Error)]
pub enum NeurofieldError {
    /// The requested PCAN interface is not available or PCANBasic library not found.
    #[error("PCAN interface not available: {reason}")]
    InterfaceNotAvailable { reason: String },

    /// No EEG device was discovered on the bus.
    #[error("No EEG device found on the CAN bus")]
    NoEegDevice,

    /// A CAN message was received with an unexpected payload length.
    #[error("Unexpected data length: expected {expected}, got {got}")]
    UnexpectedDataLength { expected: usize, got: usize },

    /// Timed out waiting for a CAN message.
    #[error("PCAN read timeout")]
    Timeout,

    /// The connected device does not support the requested operation.
    #[error("Operation not supported: {0}")]
    NotSupported(String),

    /// Bus error — usually means the device was disconnected.
    #[error("CAN bus error (device still on?): {0}")]
    BusError(String),

    /// Could not send a message on the CAN bus after multiple retries.
    #[error("Could not send message: {0}")]
    SendFailed(String),
}
