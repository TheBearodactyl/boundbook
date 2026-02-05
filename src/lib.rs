//! boundbook - a Rust implementation of the Bound Book specification
mod _types;
mod builder;
mod error;
mod reader;

pub use {
    builder::BbfBuilder,
    error::{BbfError, Result},
    reader::BbfReader,
};

pub mod types {
    //! binary data structures used in BBF files
    //!
    //! contains all struct definitions for headers, footers, and index entries. these are
    //! `#[repr(c, packed)]` structs that map directly to the binary file format.
    //!
    //! # structures
    //!
    //! - [`BbfHeader`] - file header with version and footer offset
    //! - [`BbfFooter`] - file footer with all table offsets and integrity hash
    //! - [`AssetEntry`] - describes a unique image asset with offset, size, and hash
    //! - [`PageEntry`] - links a logical page to an asset
    //! - [`Section`] - defines chapters or volumes with hierarchical structure
    //! - [`Metadata`] - key-value pairs for book information
    //! - [`MediaType`] - enum identifying image format (png, avif, webp, etc)
    //! - [`Expansion`] - reserved for future format extensions
    //!
    //! # usage
    //!
    //! ```no_run
    //! use boundbook::types::*;
    //! use boundbook::BbfReader;
    //!
    //! # fn example() -> boundbook::Result<()> {
    //! let reader = unsafe { BbfReader::open("book.BBF")? };
    //! let assets: &[AssetEntry] = reader.assets()?;
    //! let pages: &[PageEntry] = reader.pages()?;
    //! # Ok(())
    //! # }
    //! ```
    pub use crate::_types::{
        AssetEntry, BbfFooter, BbfHeader, Expansion, MediaType, Metadata, PageEntry, Section,
    };
}

pub mod format {
    //! contains all constant values used in the BBF format including magic numbers, version
    //! info, default alignment settings, size limits, and configuration flags.
    //!
    //! # constants
    //!
    //! - [`MAGIC`] - BBF3 magic number for format identification
    //! - [`VERSION`] - current format version (3)
    //! - [`ALIGNMENT`] - standard 4kb alignment constant
    //! - [`DEFAULT_GUARD_ALIGNMENT`] - default alignment exponent (12 = 4kb)
    //! - [`DEFAULT_SMALL_REAM_THRESHOLD`] - default ream size exponent (16 = 64kb)
    //! - [`MAX_BALE_SIZE`] - maximum index region size (16mb)
    //! - [`MAX_FORME_SIZE`] - maximum string scan length (2kb)
    //!
    //! # flags
    //!
    //! - [`BBF_PETRIFICATION_FLAG`] - marks file as read-only/immutable
    //! - [`BBF_VARIABLE_REAM_SIZE_FLAG`] - enables variable alignment for small assets
    //!
    //! # usage
    //!
    //! ```no_run
    //! use boundbook::BbfBuilder;
    //! use boundbook::format::{BBF_VARIABLE_REAM_SIZE_FLAG, DEFAULT_GUARD_ALIGNMENT};
    //!
    //! # fn example() -> boundbook::Result<()> {
    //! let builder = BbfBuilder::new(
    //!     "book.BBF",
    // 1     DEFAULT_GUARD_ALIGNMENT,
    //!     16,
    //!     BBF_VARIABLE_REAM_SIZE_FLAG
    //! )?;
    //! # Ok(())
    //! # }
    //! ```
    pub use crate::_types::{
        ALIGNMENT, BBF_PETRIFICATION_FLAG, BBF_VARIABLE_REAM_SIZE_FLAG, DEFAULT_GUARD_ALIGNMENT,
        DEFAULT_SMALL_REAM_THRESHOLD, MAGIC, MAX_BALE_SIZE, MAX_FORME_SIZE, VERSION,
    };
}

pub mod prelude {
    //! prelude module for convenient imports
    //!
    //! import this module with `use boundbook::prelude::*;` to get all commonly used types and
    //! functions without needing to specify each one individually.
    //!
    //! # included exports
    //!
    //! - [`BbfBuilder`] - for creating BBF files
    //! - [`BbfReader`] - for reading BBF files
    //! - [`BbfError`] - error type for BBF operations
    //! - [`crate::types::MediaType`] - image format enum
    //!
    //! # usage
    //!
    //! ```no_run
    //! use boundbook::prelude::*;
    //!
    //! fn create_book() -> Result<()> {
    //!     let mut builder = BbfBuilder::with_defaults("manga.BBF")?;
    //!     builder.add_page("cover.png", 0, 0)?;
    //!     builder.finalize()?;
    //!     Ok(())
    //! }
    //! ```
    pub use crate::{BbfBuilder, BbfError, BbfReader, Result, format::*, types::*};
}
