use thiserror::Error;

#[derive(Debug, Error)]
pub enum BbfError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Invalid magic number (expected BBF3)")]
    InvalidMagic,

    #[error("File too small to be a valid BBF file")]
    FileTooSmall,

    #[error("Invalid offset: {0}")]
    InvalidOffset(String),

    #[error("Hash mismatch detected - file may be corrupted")]
    HashMismatch,

    #[error("Invalid UTF-8 in string pool")]
    InvalidUtf8,

    #[error("Integer overflow in calculation: {0}")]
    IntegerOverflow(String),

    #[error("Reserved field validation failed: {0}")]
    ReservedFieldNonZero(String),

    #[error("Alignment exponent {0} exceeds maximum allowed value of 16")]
    AlignmentTooLarge(u8),

    #[error("Clipboard error: {0}")]
    Clipboard(#[from] arboard::Error),

    #[error("{0}")]
    Report(#[from] color_eyre::Report),

    #[error("{0}")]
    BufWriter(#[from] std::io::IntoInnerError<std::io::BufWriter<std::fs::File>>),

    #[error("{0}")]
    Other(String),
}

impl From<String> for BbfError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}

pub type Result<T> = color_eyre::Result<T, BbfError>;
