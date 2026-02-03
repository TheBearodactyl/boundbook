pub const MAGIC: &[u8; 4] = b"BBF3";
pub const ALIGNMENT: u64 = 4096;
pub const BBF_PETRIFICATION_FLAG: u32 = 0x00000001;
pub const BBF_VARIABLE_REAM_SIZE_FLAG: u32 = 0x00000002;
pub const DEFAULT_GUARD_ALIGNMENT: u8 = 12;
pub const DEFAULT_SMALL_REAM_THRESHOLD: u8 = 16;
pub const MAX_BALE_SIZE: u64 = 16000000;
pub const MAX_FORME_SIZE: u64 = 2048;
pub const VERSION: u16 = 3;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaType {
    Unknown = 0x00,
    Avif = 0x01,
    Png = 0x02,
    Webp = 0x03,
    Jxl = 0x04,
    Bmp = 0x05,
    Gif = 0x07,
    Tiff = 0x08,
    Jpg = 0x09,
}

impl MediaType {
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

    pub fn as_extension(self) -> &'static str {
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

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BbfHeader {
    pub magic: [u8; 4],
    pub version: u16,
    pub header_len: u16,
    pub flags: u32,
    pub alignment: u8,
    pub ream_size: u8,
    pub reserved_extra: u16,
    pub footer_offset: u64,
    pub reserved: [u8; 40],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct AssetEntry {
    pub file_offset: u64,
    pub asset_hash: [u64; 2],
    pub file_size: u64,
    pub flags: u32,
    pub reserved_value: u16,
    pub media_type: u8,
    pub reserved: [u8; 9],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct PageEntry {
    pub asset_index: u64,
    pub flags: u32,
    pub reserved: [u8; 4],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Section {
    pub section_title_offset: u64,
    pub section_start_index: u64,
    pub section_parent_offset: u64,
    pub reserved: [u8; 8],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Metadata {
    pub key_offset: u64,
    pub value_offset: u64,
    pub parent_offset: u64,
    pub reserved: [u8; 8],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Expansion {
    pub exp_reserved: [u64; 10],
    pub flags: u32,
    pub reserved: [u8; 44],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BbfFooter {
    pub asset_offset: u64,
    pub page_offset: u64,
    pub section_offset: u64,
    pub meta_offset: u64,
    pub expansion_offset: u64,
    pub string_pool_offset: u64,
    pub string_pool_size: u64,
    pub asset_count: u64,
    pub page_count: u64,
    pub section_count: u64,
    pub meta_count: u64,
    pub expansion_count: u64,
    pub flags: u32,
    pub footer_len: u8,
    pub padding: [u8; 3],
    pub footer_hash: u64,
    pub reserved: [u8; 144],
}
