use thiserror::Error;

/// errors that can occur during BBF file operations
///
/// represents all possible error conditions when reading, writing, or validating BBF files.
/// uses thiserror for automatic error trait implementations.
#[derive(Debug, Error)]
pub enum BbfError {
    /// i/o op failed
    ///
    /// wraps standard io errors from file ops
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// file doesn't have the BBF3 magic number
    ///
    /// indicates the file is not a valid BBF format file
    #[error("Invalid magic number (expected BBF3)")]
    InvalidMagic,

    /// indicatif can't parse progress bar template
    ///
    /// indicates an invalid template being used for a progress bar builder
    #[error("Couldn't parse indicatif progress bar template: {0}")]
    IndicatifTemplate(#[from] indicatif::style::TemplateError),

    /// file is too small to contain a valid BBF structure
    ///
    /// file must be at least large enough for header and footer
    #[error("File too small to be a valid BBF file")]
    FileTooSmall,

    /// an offset value points outside the file bounds
    ///
    /// # Arguments
    ///
    /// 1. description of which offset is invalid
    #[error("Invalid offset: {0}")]
    InvalidOffset(String),

    /// computed hash does not match stored hash
    ///
    /// indicates file corruption or tampering
    #[error("Hash mismatch detected - file may be corrupted")]
    HashMismatch,

    /// string pool contains invalid utf-8 sequences
    ///
    /// all strings in the pool must be valid utf-8
    #[error("Invalid UTF-8 in string pool")]
    InvalidUtf8,

    /// arithmetic operation resulted in integer overflow
    ///
    /// # Arguments
    ///
    /// 1. description of which calculation overflowed
    #[error("Integer overflow in calculation: {0}")]
    IntegerOverflow(String),

    /// a reserved field contained non-zero value
    ///
    /// # Arguments
    ///
    /// 1. description of which reserved field was non-zero
    #[error("Reserved field validation failed: {0}")]
    ReservedFieldNonZero(String),

    /// alignment exponent exceeds the maximum allowed value
    ///
    /// # Arguments
    ///
    /// 1. the invalid alignment exponent value
    #[error("Alignment exponent {0} exceeds maximum allowed value of 16")]
    AlignmentTooLarge(u8),

    /// clipboard operation failed
    ///
    /// wraps errors from the arboard clipboard library
    #[error("Clipboard error: {0}")]
    Clipboard(#[from] arboard::Error),

    /// general error report
    ///
    /// wraps color_eyre error reports
    #[error("{0}")]
    ColorEyreReport(#[from] color_eyre::Report),

    /// buffered writer failed to flush
    ///
    /// wraps errors when extracting inner file from bufwriter
    #[error("{0}")]
    BufWriter(#[from] std::io::IntoInnerError<std::io::BufWriter<std::fs::File>>),

    /// user input prompt failed
    ///
    /// wraps errors from the inquire prompting library
    #[error("Error getting user input: {0}")]
    InquireError(#[from] inquire::InquireError),

    /// other miscellaneous error
    ///
    /// # Arguments
    ///
    /// 1. description of the error
    #[error("{0}")]
    Other(String),

    /// miette error report
    ///
    /// wraps miette error reports
    #[error("{0}")]
    MietteReport(miette::Report),
}

impl From<String> for BbfError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}

impl From<miette::Report> for BbfError {
    fn from(value: miette::Report) -> Self {
        Self::MietteReport(value)
    }
}

/// result type using BBFerror
///
/// standard result type for all BBF operations, combining color_eyre's result with BBFerror
pub type Result<T> = color_eyre::Result<T, BbfError>;
