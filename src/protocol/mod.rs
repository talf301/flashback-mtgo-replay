// src/protocol/mod.rs

// --- New modules (Phase A) ---
pub mod framing;
pub mod opcodes;

// --- Existing modules (kept until Phase C replacement) ---
pub mod decoder;
pub mod raw_analyzer;
pub use decoder::{GameEvent, DecodedEvent, decode_packet, decode_stream};
pub use raw_analyzer::{PacketAnalysis, Pattern, analyze_packet, analyze_dump_file};

// --- Error type shared by all protocol modules ---

use thiserror::Error;

#[derive(Debug, Error)]
pub enum DecodeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("unexpected EOF: {context}")]
    UnexpectedEof { context: &'static str },

    #[error("invalid checksum: expected {expected}, got {got}")]
    InvalidChecksum { expected: i32, got: i32 },

    #[error("diff out of bounds: {context}")]
    DiffOutOfBounds { context: &'static str },

    #[error("diff size mismatch: expected {expected}, got {got}")]
    DiffSizeMismatch { expected: u32, got: u32 },
}

impl DecodeError {
    /// Returns true if this error represents an end-of-stream condition.
    pub fn is_eof(&self) -> bool {
        match self {
            DecodeError::UnexpectedEof { .. } => true,
            DecodeError::Io(e) => e.kind() == std::io::ErrorKind::UnexpectedEof,
            _ => false,
        }
    }
}
