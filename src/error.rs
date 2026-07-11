//! Error types returned by every fallible operation in this crate.

/// The error type for all `rustmc` operations.
#[derive(Debug, thiserror::Error)]
pub enum RustmcError {
    /// Underlying I/O failure (socket, connect, read, write).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// The server's JSON status payload could not be parsed.
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    /// DNS resolution (A/AAAA or SRV) failed.
    #[error("DNS error: {0}")]
    Dns(String),

    /// The server sent a response that does not match the expected protocol
    /// shape (missing field, out-of-range length, bad framing, etc).
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// No response was received within the configured timeout.
    #[error("Timeout after {0:?}")]
    Timeout(std::time::Duration),

    /// The address string passed to `Address::parse` could not be parsed.
    #[error("Invalid address: {0}")]
    InvalidAddress(String),

    /// A VarInt exceeded 5 bytes or its 5th byte had payload bits set beyond
    /// what fits in an `i32`.
    #[error("VarInt too large")]
    VarIntOverflow,
}
