use {miette::Diagnostic, thiserror::Error};

/// Errors that can occur during BBF file operations
///
/// Represents all possible error conditions when reading, writing, or validating BBF files.
/// Uses thiserror for automatic error trait implementations and miette for rich diagnostics.
#[derive(Debug, Error, Diagnostic)]
pub enum BbfError {
    /// I/O operation failed
    ///
    /// Wraps standard io errors from file operations
    #[error("I/O error: {0}")]
    #[diagnostic(
        code(boundbook::io_error),
        help("Check file permissions and ensure the path exists")
    )]
    Io(#[from] std::io::Error),

    /// File doesn't have the BBF3 magic number
    ///
    /// Indicates the file is not a valid BBF format file
    #[error("Invalid magic number (expected BBF3)")]
    #[diagnostic(
        code(boundbook::invalid_magic),
        help("This file is not a valid BBF3 file. Ensure you're opening the correct file format."),
        url("https://docs.rs/your-crate/latest/your-crate/format.html#magic-number")
    )]
    InvalidMagic,

    /// Indicatif can't parse progress bar template
    ///
    /// Indicates an invalid template being used for a progress bar builder
    #[error("Couldn't parse indicatif progress bar template: {0}")]
    #[diagnostic(
        code(boundbook::indicatif_template),
        help(
            "Check your progress bar template syntax. See indicatif documentation for valid templates."
        )
    )]
    IndicatifTemplate(#[from] indicatif::style::TemplateError),

    /// File is too small to contain a valid BBF structure
    ///
    /// File must be at least large enough for header and footer
    #[error("File too small to be a valid BBF file")]
    #[diagnostic(
        code(boundbook::file_too_small),
        help(
            "BBF files require a minimum size for header and footer. The file may be truncated or corrupted."
        ),
        severity(Error)
    )]
    FileTooSmall,

    /// An offset value points outside the file bounds
    #[error("Invalid offset: {description}")]
    #[diagnostic(
        code(boundbook::invalid_offset),
        help(
            "The file structure references data beyond the file boundaries. The file may be corrupted."
        )
    )]
    InvalidOffset {
        /// a description of which offset is invalid
        description: String,
    },

    /// Computed hash does not match stored hash
    ///
    /// Indicates file corruption or tampering
    #[error("Hash mismatch detected - file may be corrupted")]
    #[diagnostic(
        code(boundbook::hash_mismatch),
        help(
            "The file's integrity check failed. This could indicate corruption or tampering. Try re-downloading or restoring from backup."
        ),
        severity(Error)
    )]
    HashMismatch,

    /// String pool contains invalid UTF-8 sequences
    ///
    /// All strings in the pool must be valid UTF-8
    #[error("Invalid UTF-8 in string pool")]
    #[diagnostic(
        code(boundbook::invalid_utf8),
        help(
            "The string pool contains invalid UTF-8 data. The file may be corrupted or created with incompatible encoding."
        )
    )]
    InvalidUtf8,

    /// Arithmetic operation resulted in integer overflow
    #[error("Integer overflow in calculation: {description}")]
    #[diagnostic(
        code(boundbook::integer_overflow),
        help(
            "A size or offset calculation overflowed. The file may contain invalid data or be maliciously crafted."
        ),
        severity(Error)
    )]
    IntegerOverflow {
        /// a description of the error
        description: String,
    },

    /// A reserved field contained non-zero value
    #[error("Reserved field validation failed: {description}")]
    #[diagnostic(
        code(boundbook::reserved_field_nonzero),
        help(
            "A reserved field contains non-zero data. This may indicate an incompatible file version or corruption."
        ),
        severity(Warning)
    )]
    ReservedFieldNonZero {
        /// a description of the error
        description: String,
    },

    /// Alignment exponent exceeds the maximum allowed value
    #[error("Alignment exponent {exponent} exceeds maximum allowed value of 16")]
    #[diagnostic(
        code(boundbook::alignment_too_large),
        help(
            "The alignment exponent must be between 0 and 16. Check the file format specification."
        )
    )]
    AlignmentTooLarge {
        /// the exponent that exceeded 16
        exponent: u8,
    },

    /// Clipboard operation failed
    ///
    /// Wraps errors from the arboard clipboard library
    #[error("Clipboard error: {0}")]
    #[diagnostic(
        code(boundbook::clipboard_error),
        help("Failed to access the system clipboard. Ensure clipboard permissions are granted.")
    )]
    Clipboard(#[from] arboard::Error),

    /// Buffered writer failed to flush
    ///
    /// Wraps errors when extracting inner file from BufWriter
    #[error("{0}")]
    #[diagnostic(
        code(boundbook::bufwriter_error),
        help("Failed to flush buffered data to disk. Check available disk space and permissions.")
    )]
    BufWriter(#[from] std::io::IntoInnerError<std::io::BufWriter<std::fs::File>>),

    /// User input prompt failed
    ///
    /// Wraps errors from the inquire prompting library
    #[error("Error getting user input: {0}")]
    #[diagnostic(
        code(boundbook::inquire_error),
        help(
            "Failed to read user input. Ensure stdin is available and the terminal is interactive."
        )
    )]
    InquireError(#[from] inquire::InquireError),

    /// Other miscellaneous error
    #[error("{message}")]
    #[diagnostic(
        code(boundbook::other),
        help("An unexpected error occurred. Check the error message for details.")
    )]
    Other {
        /// the error message
        message: String,
    },

    /// A generic error
    #[error(transparent)]
    Generic(#[from] Box<dyn std::error::Error>),
}

impl From<String> for BbfError {
    fn from(value: String) -> Self {
        Self::Other { message: value }
    }
}

impl From<miette::Report> for BbfError {
    fn from(value: miette::Report) -> Self {
        Self::Generic(value.into())
    }
}

unsafe impl Send for BbfError {}
unsafe impl Sync for BbfError {}

/// Result type using BbfError
///
/// Standard result type for all BBF operations, combining miette's Result with BbfError
pub type Result<T> = miette::Result<T, BbfError>;
