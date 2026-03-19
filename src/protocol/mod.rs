// src/protocol/mod.rs

pub mod framing;
pub mod opcodes;
pub mod fls;
pub mod game_messages;
pub mod statebuf;

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
    pub fn is_eof(&self) -> bool {
        match self {
            DecodeError::UnexpectedEof { .. } => true,
            DecodeError::Io(e) => e.kind() == std::io::ErrorKind::UnexpectedEof,
            _ => false,
        }
    }
}
