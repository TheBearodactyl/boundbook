pub const MAGIC: &[u8; 4] = b"BBF1";
pub const ALIGNMENT: u64 = 4096;

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
    pub version: u8,
    pub flags: u32,
    pub header_len: u16,
    pub reserved: u64,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct AssetEntry {
    pub offset: u64,
    pub length: u64,
    pub decoded_length: u64,
    pub xxh3_hash: u64,
    pub media_type: u8,
    pub flags: u8,
    pub padding: [u8; 6],
    pub reserved: [u64; 3],
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct PageEntry {
    pub asset_index: u32,
    pub flags: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Section {
    pub title_offset: u32,
    pub start_index: u32,
    pub parent_index: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Metadata {
    pub key_offset: u32,
    pub val_offset: u32,
}

#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BbfFooter {
    pub string_pool_offset: u64,
    pub asset_table_offset: u64,
    pub asset_count: u32,
    pub page_table_offset: u64,
    pub page_count: u32,
    pub section_table_offset: u64,
    pub section_count: u32,
    pub meta_table_offset: u64,
    pub key_count: u32,
    pub extra_offset: u64,
    pub index_hash: u64,
    pub magic: [u8; 4],
}
