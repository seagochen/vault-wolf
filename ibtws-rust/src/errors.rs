//! Error types for the ibtws-rust library.

use thiserror::Error;

/// Top-level error type for the IB API client library.
#[derive(Debug, Error)]
pub enum IBApiError {
    /// TCP connection failure or socket error.
    #[error("Connection error: {0}")]
    Connection(String),

    /// Failed to encode a request message.
    #[error("Encoding error: {0}")]
    Encoding(String),

    /// Failed to decode a response message.
    #[error("Decoding error: {0}")]
    Decoding(String),

    /// Protocol-level error (version mismatch, bad message format, etc.).
    #[error("Protocol error: {0}")]
    Protocol(String),

    /// Error reported by the TWS/Gateway server.
    #[error("Server error (id={id}, code={code}): {message}")]
    Server {
        id: i32,
        code: i32,
        message: String,
        advanced_order_reject_json: String,
    },

    /// Operation timed out waiting for a response.
    #[error("Timeout: {0}")]
    Timeout(String),

    /// Connection was unexpectedly closed.
    #[error("Disconnected: {0}")]
    Disconnected(String),

    /// I/O error from the underlying transport.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience Result type for this library.
pub type Result<T> = std::result::Result<T, IBApiError>;
