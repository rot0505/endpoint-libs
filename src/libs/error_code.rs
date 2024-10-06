use serde::*;

/// `ErrorCode` is a wrapper around `u32` that represents an error code.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ErrorCode {
    /// The error code (e.g. `100400` for BadRequest).
    code: u32,
}

impl ErrorCode {
    /// Create a new `ErrorCode` from a `u32`.
    pub fn new(code: u32) -> Self {
        Self { code }
    }

    /// Get the `ErrorCode` as a `u32`.
    /// Just returns the code field.
    pub fn to_u32(self) -> u32 {
        self.code
    }

    /// Getter for the `ErrorCode` code field.
    pub fn code(self) -> u32 {
        self.to_u32()
    }
}
