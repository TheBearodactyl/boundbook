/// BBF3 magic number
///
/// identifies a file as BBF format version 3
pub const MAGIC: &[u8; 4] = b"BBF3";

/// default alignment in bytes
///
/// standard 4kb alignment for mem-mapped access
pub const ALIGNMENT: u64 = 4096;

/// flag indicating permanent read-only status
///
/// when set in header flags, indicates the file should not be modified
pub const BBF_PETRIFICATION_FLAG: u32 = 0x00000001;

/// flag enabling variable ream size optimization
///
/// when set, small assets use reduced alignment for better space efficiency
pub const BBF_VARIABLE_REAM_SIZE_FLAG: u32 = 0x00000002;

/// default guard alignment exponent
///
/// alignment = 1 << 12 = 4096 bytes (4kb)
pub const DEFAULT_GUARD_ALIGNMENT: u8 = 12;

/// default small ream threshold exponent
///
/// threshold = 1 << 16 = 65536 bytes (64kb)
pub const DEFAULT_SMALL_REAM_THRESHOLD: u8 = 16;

/// maximum size for index region
///
/// limits total size of all tables to prevent excessive memory usage
pub const MAX_BALE_SIZE: u64 = 16000000;

/// maximum string length for scanning
///
/// limits null terminator search to prevent unbounded iteration
pub const MAX_FORME_SIZE: u64 = 2048;

/// current BBF format version
pub const VERSION: u16 = 3;

/// image format types
///
/// identifies the media type of asset data for proper decoding
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    /// unknown or unsupported format
    Unknown = 0x00,
    /// av1 image format
    Avif = 0x01,
    /// portable network graphics
    Png = 0x02,
    /// webp image format
    Webp = 0x03,
    /// jpeg xl image format
    Jxl = 0x04,
    /// bitmap image format
    Bmp = 0x05,
    /// graphics interchange format
    Gif = 0x07,
    /// tagged image file format
    Tiff = 0x08,
    /// jpeg image format
    Jpg = 0x09,
}

impl MediaType {
    /// determines media type from file extension
    ///
    /// # Arguments
    ///
    /// * `ext` - file extension string (with or without leading dot)
    ///
    /// # Returns
    ///
    /// the corresponding `MediaType` variant, or `Unknown` if not recognized
    pub fn from_extension(ext: &str) -> Self {
        match ext.trim_start_matches('.').to_lowercase().as_str() {
            "png" => Self::Png,
            "jpg" | "jpeg" => Self::Jpg,
            "avif" => Self::Avif,
            "webp" => Self::Webp,
            "jxl" => Self::Jxl,
            "bmp" => Self::Bmp,
            "gif" => Self::Gif,
            "tiff" | "tif" => Self::Tiff,
            _ => Self::Unknown,
        }
    }

    /// converts media type to standard file extension
    ///
    /// # Returns
    ///
    /// the standard file extension string including the leading dot
    pub const fn as_extension(self) -> &'static str {
        match self {
            Self::Avif => ".avif",
            Self::Png => ".png",
            Self::Jpg => ".jpg",
            Self::Webp => ".webp",
            Self::Jxl => ".jxl",
            Self::Bmp => ".bmp",
            Self::Gif => ".gif",
            Self::Tiff => ".tiff",
            Self::Unknown => ".png",
        }
    }
}

impl From<u8> for MediaType {
    fn from(value: u8) -> Self {
        match value {
            0x01 => Self::Avif,
            0x02 => Self::Png,
            0x03 => Self::Webp,
            0x04 => Self::Jxl,
            0x05 => Self::Bmp,
            0x07 => Self::Gif,
            0x08 => Self::Tiff,
            0x09 => Self::Jpg,
            _ => Self::Unknown,
        }
    }
}

/// BBF file header
///
/// fixed-size header at the start of every BBF file containing version info, alignment settings,
/// and the offset to the footer
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BbfHeader {
    /// magic number identifying BBF3 format
    pub magic: [u8; 4],
    /// format version number
    pub version: u16,
    /// size of this header struct in bytes
    pub header_len: u16,
    /// configuration flags (petrification, variable ream, etc)
    pub flags: u32,
    /// alignment exponent (actual alignment = 1 << alignment)
    pub alignment: u8,
    /// ream size exponent (actual size = 1 << ream_size)
    pub ream_size: u8,
    /// reserved for future use, must be zero
    pub reserved_extra: u16,
    /// byte offset to the footer in the file
    pub footer_offset: u64,
    /// reserved for future use, must be all zeros
    pub reserved: [u8; 40],
}

/// asset entry describing a unique piece of image data
///
/// each asset represents deduplicated image data stored in the file. multiple pages can reference
/// the same asset if they contain identical image data.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct AssetEntry {
    /// byte offset where asset data begins in the file
    pub file_offset: u64,
    /// 128-bit xxh3 hash of the asset data [low 64 bits, high 64 bits]
    pub asset_hash: [u64; 2],
    /// size of the asset data in bytes
    pub file_size: u64,
    /// asset-specific flags
    pub flags: u32,
    /// reserved for future use, must be zero
    pub reserved_value: u16,
    /// media type identifier (png, jpg, webp, etc)
    pub media_type: u8,
    /// reserved for future use, must be all zeros
    pub reserved: [u8; 9],
}

/// page entry linking to an asset
///
/// represents a single page in the book that displays an asset. pages are ordered sequentially
/// and can have flags for special rendering or behavior.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct PageEntry {
    /// index into the assets array
    pub asset_index: u64,
    /// page-specific flags
    pub flags: u32,
    /// reserved for future use, must be all zeros
    pub reserved: [u8; 4],
}

/// section entry for hierarchical organization
///
/// represents a chapter, part, or other organizational unit. sections can be nested by referencing
/// a parent section.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Section {
    /// offset in string pool for the section title
    pub section_title_offset: u64,
    /// first page index in this section
    pub section_start_index: u64,
    /// offset in string pool for parent section title, or u64::max if top-level
    pub section_parent_offset: u64,
    /// reserved for future use, must be all zeros
    pub reserved: [u8; 8],
}

/// metadata key-value pair
///
/// stores arbitrary metadata like author, title, isbn, publisher, or other book information.
/// metadata can optionally be associated with a specific section.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Metadata {
    /// offset in string pool for the metadata key
    pub key_offset: u64,
    /// offset in string pool for the metadata value
    pub value_offset: u64,
    /// offset in string pool for parent section, or u64::max if global
    pub parent_offset: u64,
    /// reserved for future use, must be all zeros
    pub reserved: [u8; 8],
}

/// expansion entry for future format extensions
///
/// reserved structure for future format additions without breaking compatibility
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Expansion {
    /// reserved fields for future data
    pub exp_reserved: [u64; 10],
    /// flags for expansion features
    pub flags: u32,
    /// reserved for future use, must be all zeros
    pub reserved: [u8; 44],
}

/// BBF file footer
///
/// contains all table offsets, counts, and an integrity hash of the index region. located at the
/// offset specified in the header.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BbfFooter {
    /// byte offset where asset table begins
    pub asset_offset: u64,
    /// byte offset where page table begins
    pub page_offset: u64,
    /// byte offset where section table begins
    pub section_offset: u64,
    /// byte offset where metadata table begins
    pub meta_offset: u64,
    /// byte offset where expansion table begins (unused)
    pub expansion_offset: u64,
    /// byte offset where string pool begins
    pub string_pool_offset: u64,
    /// total size of string pool in bytes
    pub string_pool_size: u64,
    /// number of asset entries
    pub asset_count: u64,
    /// number of page entries
    pub page_count: u64,
    /// number of section entries
    pub section_count: u64,
    /// number of metadata entries
    pub meta_count: u64,
    /// number of expansion entries (unused)
    pub expansion_count: u64,
    /// footer-specific flags
    pub flags: u32,
    /// size of this footer struct in bytes
    pub footer_len: u8,
    /// padding bytes for alignment, must be all zeros
    pub padding: [u8; 3],
    /// 64-bit xxh3 hash of all index data (asset table through string pool)
    pub footer_hash: u64,
    /// reserved for future use, must be all zeros
    pub reserved: [u8; 144],
}
