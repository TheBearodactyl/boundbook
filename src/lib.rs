use std::{
    collections::HashMap,
    fs::File,
    io::{self, BufWriter, Write},
    path::Path,
};
use thiserror::Error;
use twox_hash::XxHash3_64;

const MAGIC: &[u8; 4] = b"BBF1";
const ALIGNMENT: u64 = 4096;

#[derive(Debug, Error)]
pub enum BbfError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Invalid magic number")]
    InvalidMagic,
    #[error("File too small")]
    FileTooSmall,
    #[error("Invalid offset: {0}")]
    InvalidOffset(String),
    #[error("Hash mismatch")]
    HashMismatch,
}

pub type Result<T> = std::result::Result<T, BbfError>;

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
        match ext.to_lowercase().as_str() {
            ".png" | "png" => Self::Png,
            ".jpg" | "jpeg" | ".jpeg" | "jpg" => Self::Jpg,
            ".avif" | "avif" => Self::Avif,
            ".webp" | "webp" => Self::Webp,
            ".jxl" | "jxl" => Self::Jxl,
            ".bmp" | "bmp" => Self::Bmp,
            ".gif" | "gif" => Self::Gif,
            ".tiff" | "tiff" => Self::Tiff,
            _ => Self::Unknown,
        }
    }

    pub fn as_extension(&self) -> &'static str {
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

pub struct BbfBuilder {
    pub writer: BufWriter<File>,
    pub current_offset: u64,
    pub assets: Vec<AssetEntry>,
    pub pages: Vec<PageEntry>,
    pub sections: Vec<Section>,
    pub metadata: Vec<Metadata>,
    pub string_pool: Vec<u8>,
    pub dedupe_map: HashMap<u64, u32>,
    pub string_map: HashMap<String, u32>,
}

impl BbfBuilder {
    pub fn new<P: AsRef<Path>>(output_path: P) -> Result<Self> {
        let file = File::create(output_path)?;
        let mut writer = BufWriter::new(file);

        let header = BbfHeader {
            magic: *MAGIC,
            version: 2,
            flags: 0,
            header_len: std::mem::size_of::<BbfHeader>() as u16,
            reserved: 0,
        };

        unsafe {
            let header_bytes = std::slice::from_raw_parts(
                &header as *const BbfHeader as *const u8,
                std::mem::size_of::<BbfHeader>(),
            );

            writer.write_all(header_bytes)?;
        }

        let current_offset = std::mem::size_of::<BbfHeader>() as u64;

        Ok(Self {
            writer,
            current_offset,
            assets: Vec::new(),
            pages: Vec::new(),
            sections: Vec::new(),
            metadata: Vec::new(),
            string_pool: Vec::new(),
            dedupe_map: HashMap::new(),
            string_map: HashMap::new(),
        })
    }

    fn align_padding(&mut self) -> Result<()> {
        let padding = (ALIGNMENT - (self.current_offset % ALIGNMENT)) % ALIGNMENT;

        if padding > 0 {
            let zeros = vec![0u8; padding as usize];
            self.writer.write_all(&zeros)?;
            self.current_offset += padding;
        }

        Ok(())
    }

    fn calculate_hash(data: &[u8]) -> u64 {
        use std::hash::Hasher;
        let mut hasher = XxHash3_64::with_seed(0);
        hasher.write(data);
        hasher.finish()
    }

    pub fn add_page<P: AsRef<Path>>(&mut self, image_path: P, media_type: MediaType) -> Result<()> {
        let data = std::fs::read(image_path)?;
        let hash = Self::calculate_hash(&data);

        let asset_index = if let Some(&idx) = self.dedupe_map.get(&hash) {
            idx
        } else {
            self.align_padding()?;

            let asset = AssetEntry {
                offset: self.current_offset,
                length: data.len() as u64,
                decoded_length: data.len() as u64,
                xxh3_hash: hash,
                media_type: media_type as u8,
                flags: 0,
                padding: [0; 6],
                reserved: [0; 3],
            };

            self.writer.write_all(&data)?;
            self.current_offset += data.len() as u64;

            let idx = self.assets.len() as u32;
            self.assets.push(asset);
            self.dedupe_map.insert(hash, idx);
            idx
        };

        self.pages.push(PageEntry {
            asset_index,
            flags: 0,
        });

        Ok(())
    }

    fn get_or_add_string(&mut self, s: &str) -> u32 {
        if let Some(&offset) = self.string_map.get(s) {
            return offset;
        }

        let offset = self.string_pool.len() as u32;
        self.string_pool.extend_from_slice(s.as_bytes());
        self.string_pool.push(0);
        self.string_map.insert(s.to_string(), offset);
        offset
    }

    pub fn add_section(&mut self, title: &str, start_page: u32, parent: Option<u32>) -> Result<()> {
        let title_offset = self.get_or_add_string(title);

        self.sections.push(Section {
            title_offset,
            start_index: start_page,
            parent_index: parent.unwrap_or(0xFFFFFFFF),
        });

        Ok(())
    }

    pub fn add_metadata(&mut self, key: &str, value: &str) -> Result<()> {
        let key_offset = self.get_or_add_string(key);
        let val_offset = self.get_or_add_string(value);

        self.metadata.push(Metadata {
            key_offset,
            val_offset,
        });

        Ok(())
    }

    pub fn finalize(mut self) -> Result<()> {
        use std::hash::Hasher;

        let mut hasher = XxHash3_64::with_seed(0);

        let write_and_hash = |writer: &mut BufWriter<File>,
                              hasher: &mut XxHash3_64,
                              data: &[u8]|
         -> io::Result<()> {
            if !data.is_empty() {
                writer.write_all(data)?;
                hasher.write(data);
            }
            Ok(())
        };

        let footer = BbfFooter {
            string_pool_offset: self.current_offset,
            asset_table_offset: 0,
            asset_count: self.assets.len() as u32,
            page_table_offset: 0,
            page_count: self.pages.len() as u32,
            section_table_offset: 0,
            section_count: self.sections.len() as u32,
            meta_table_offset: 0,
            key_count: self.metadata.len() as u32,
            extra_offset: 0,
            index_hash: 0,
            magic: *MAGIC,
        };

        write_and_hash(&mut self.writer, &mut hasher, &self.string_pool)?;
        self.current_offset += self.string_pool.len() as u64;

        let footer = BbfFooter {
            asset_table_offset: self.current_offset,
            ..footer
        };

        unsafe {
            let assets_bytes = std::slice::from_raw_parts(
                self.assets.as_ptr() as *const u8,
                self.assets.len() * std::mem::size_of::<AssetEntry>(),
            );
            write_and_hash(&mut self.writer, &mut hasher, assets_bytes)?;
            self.current_offset += assets_bytes.len() as u64;
        }

        let footer = BbfFooter {
            page_table_offset: self.current_offset,
            ..footer
        };

        unsafe {
            let pages_bytes = std::slice::from_raw_parts(
                self.pages.as_ptr() as *const u8,
                self.pages.len() * std::mem::size_of::<PageEntry>(),
            );
            write_and_hash(&mut self.writer, &mut hasher, pages_bytes)?;
            self.current_offset += pages_bytes.len() as u64;
        }

        let footer = BbfFooter {
            section_table_offset: self.current_offset,
            ..footer
        };

        unsafe {
            let sections_bytes = std::slice::from_raw_parts(
                self.sections.as_ptr() as *const u8,
                self.sections.len() * std::mem::size_of::<Section>(),
            );
            write_and_hash(&mut self.writer, &mut hasher, sections_bytes)?;
            self.current_offset += sections_bytes.len() as u64;
        }

        let footer = BbfFooter {
            meta_table_offset: self.current_offset,
            ..footer
        };

        unsafe {
            let metadata_bytes = std::slice::from_raw_parts(
                self.metadata.as_ptr() as *const u8,
                self.metadata.len() * std::mem::size_of::<Metadata>(),
            );
            write_and_hash(&mut self.writer, &mut hasher, metadata_bytes)?;
        }

        let final_footer = BbfFooter {
            index_hash: hasher.finish(),
            ..footer
        };

        unsafe {
            let footer_bytes = std::slice::from_raw_parts(
                &final_footer as *const BbfFooter as *const u8,
                std::mem::size_of::<BbfFooter>(),
            );
            self.writer.write_all(footer_bytes)?;
        }

        self.writer.flush()?;
        Ok(())
    }
}

pub struct BbfReader {
    mmap: memmap2::Mmap,
    header: BbfHeader,
    footer: BbfFooter,
}

impl BbfReader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { memmap2::Mmap::map(&file)? };

        if mmap.len() < std::mem::size_of::<BbfHeader>() + std::mem::size_of::<BbfFooter>() {
            return Err(BbfError::FileTooSmall);
        }

        let header: BbfHeader =
            unsafe { std::ptr::read_unaligned(mmap.as_ptr() as *const BbfHeader) };

        if &header.magic != MAGIC {
            return Err(BbfError::InvalidMagic);
        }

        let footer_offset = mmap.len() - std::mem::size_of::<BbfFooter>();
        let footer: BbfFooter = unsafe {
            std::ptr::read_unaligned((mmap.as_ptr().add(footer_offset)) as *const BbfFooter)
        };

        if &footer.magic != MAGIC {
            return Err(BbfError::InvalidMagic);
        }

        Ok(Self {
            mmap,
            header,
            footer,
        })
    }

    pub fn get_string(&self, offset: u32) -> Result<&str> {
        let pool_start = self.footer.string_pool_offset as usize;
        let pool_end = self.footer.asset_table_offset as usize;

        if offset as usize >= pool_end - pool_start {
            return Err(BbfError::InvalidOffset(format!("String offset {}", offset)));
        }

        let start = pool_start + offset as usize;
        let data = &self.mmap[start..pool_end];

        let end = data.iter().position(|&b| b == 0).unwrap_or(data.len());
        std::str::from_utf8(&data[..end])
            .map_err(|_| BbfError::InvalidOffset("Invalid UTF-8".to_string()))
    }

    pub fn assets(&self) -> &[AssetEntry] {
        unsafe {
            std::slice::from_raw_parts(
                self.mmap
                    .as_ptr()
                    .add(self.footer.asset_table_offset as usize)
                    as *const AssetEntry,
                self.footer.asset_count as usize,
            )
        }
    }

    pub fn pages(&self) -> &[PageEntry] {
        unsafe {
            std::slice::from_raw_parts(
                self.mmap
                    .as_ptr()
                    .add(self.footer.page_table_offset as usize)
                    as *const PageEntry,
                self.footer.page_count as usize,
            )
        }
    }

    pub fn sections(&self) -> &[Section] {
        unsafe {
            std::slice::from_raw_parts(
                self.mmap
                    .as_ptr()
                    .add(self.footer.section_table_offset as usize)
                    as *const Section,
                self.footer.section_count as usize,
            )
        }
    }

    pub fn metadata(&self) -> &[Metadata] {
        unsafe {
            std::slice::from_raw_parts(
                self.mmap
                    .as_ptr()
                    .add(self.footer.meta_table_offset as usize) as *const Metadata,
                self.footer.key_count as usize,
            )
        }
    }

    pub fn get_asset_data(&self, asset: &AssetEntry) -> &[u8] {
        &self.mmap[asset.offset as usize..(asset.offset + asset.length) as usize]
    }

    pub fn version(&self) -> u8 {
        self.header.version
    }

    pub fn page_count(&self) -> u32 {
        self.footer.page_count
    }

    pub fn asset_count(&self) -> u32 {
        self.footer.asset_count
    }

    pub fn verify_integrity(&self) -> Result<bool> {
        use rayon::prelude::*;

        let meta_start = self.footer.string_pool_offset as usize;
        let meta_size = self.mmap.len() - std::mem::size_of::<BbfFooter>() - meta_start;
        let calc_hash = BbfBuilder::calculate_hash(&self.mmap[meta_start..meta_start + meta_size]);

        if calc_hash != self.footer.index_hash {
            return Ok(false);
        }

        let all_valid = self.assets().par_iter().all(|asset| {
            let data = self.get_asset_data(asset);
            let hash = BbfBuilder::calculate_hash(data);
            hash == asset.xxh3_hash
        });

        Ok(all_valid)
    }

    pub fn verify_asset(&self, index: usize) -> Result<bool> {
        let assets = self.assets();
        if index >= assets.len() {
            return Err(BbfError::InvalidOffset(format!("Asset index {}", index)));
        }

        let asset = &assets[index];
        let data = self.get_asset_data(asset);
        let hash = BbfBuilder::calculate_hash(data);
        Ok(hash == asset.xxh3_hash)
    }
}
