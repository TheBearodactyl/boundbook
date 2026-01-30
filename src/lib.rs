//! BBF (Bound Book Format)
//!
//! a high-perf Rust port of the Bound Book Format, a binary container designed specifically for
//! digital comics and manga.
//!
//! BBF is optimized for DirectStorage/mmap stuff, integrity verification, and mixed-codec support.
//! unlike CBZ/CBR archives, BBF has:
//!
//! - 4KB aligned asset storage for DirectStorage compat
//! - native content deduplication via XXH3 hashing
//! - per-asset integrity verification
//! - hierarchical section/chapter organization
//! - arbitrary utf8 metadata storage
//! - footer-indexed format for append-only creation and random access
//!
//! # Examples
//!
//! ```no_run
//! use boundbook::{BbfBuilder, MediaType};
//!
//! let mut builder = BbfBuilder::new("woah.bbf")?;
//!
//! builder.add_metadata("Title", "ur mom")?;
//! builder.add_metadata("Author", "me")?;
//!
//! builder.add_page("cover.png", MediaType::Png)?;
//! builder.add_page("page001.png", MediaType::Png)?;
//!
//! builder.add_section("chapter 1", 0, None)?;
//! builder.add_section("chapter 2", 50, None)?;
//!
//! builder.finalize()?;
//! ```

use {
    rayon::prelude::*,
    std::{
        collections::HashMap,
        fs::File,
        hash::Hasher,
        io::{self, BufWriter, Write},
        path::Path,
    },
    thiserror::Error,
    twox_hash::XxHash3_64,
};

/// BBF magic number
pub const MAGIC: &[u8; 4] = b"BBF1";

/// alignment boundary for asset data
///
/// all assets are 4KB aligned for DirectStorage support and to optimize for SSD/NVMe hardware.
pub const ALIGNMENT: u64 = 4096;

/// errors that can happen when working with BBF files
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
    #[error("{0}")]
    Other(String),
}

impl From<String> for BbfError {
    fn from(value: String) -> Self {
        Self::Other(value)
    }
}

pub type Result<T> = std::result::Result<T, BbfError>;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// media type idents for image assets
///
/// BBF explicitly flags the codec for every asset, allowing readers to initialize the correct
/// decoder withot ft detection
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
    /// infer media type from file ext
    ///
    /// # Arguments
    ///
    /// * `ext` - file ext with/without leading dot
    ///
    /// # Examples
    ///
    /// ```
    /// use boundbook::MediaType;
    ///
    /// assert_eq!(MediaType::from_extension("png"), MediaType::Png);
    /// assert_eq!(MediaType::from_extension(".jpg"), MediaType::Jpg);
    /// assert_eq!(MediaType::from_extension("unknown"), MediaType::Unknown);
    /// ```
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

    /// get the standard file ext for this media type
    ///
    /// # Examples
    ///
    /// ```
    /// use boundbook::MediaType;
    ///
    /// assert_eq!(MediaType::Png.as_extension(), ".png");
    /// assert_eq!(MediaType::Avif.as_extension(), ".avif");
    /// ```
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
    /// convert a byte val to a [`MediaType`]
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

/// BBF file header (13 bytes)
///
/// located at the start of every BBF file. contains magic number, version info, and flags for
/// future use
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BbfHeader {
    /// magic number: b"BBF1"
    pub magic: [u8; 4],
    /// format ver (currently 2)
    pub version: u8,
    /// feature flags (reserved for future use)
    pub flags: u32,
    /// size of this struct
    pub header_len: u16,
    /// reserved for future use
    pub reserved: u64,
}

/// asset entry in the asset table
///
/// describes a single physical data blob (image) with its loc, size, hash, and codec info. assets
/// are deduped by hash
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct AssetEntry {
    /// byte offset from file start to asset data
    pub offset: u64,
    /// size of asset data in bytes
    pub length: u64,
    /// decoded size (currently same as length)
    pub decoded_length: u64,
    /// XXH3-64 hash of assrt data for integrity verification
    pub xxh3_hash: u64,
    /// media type ident (see [`MediaType`])
    pub media_type: u8,
    /// feature flags (reserved for future use)
    pub flags: u8,
    /// alignment padding
    pub padding: [u8; 6],
    /// reserved for future exts (e.g., DirectStorage)
    pub reserved: [u64; 3],
}

/// page entry in the page table
///
/// defines the logicial reading order by referencing assets. multiple pages can reference the same
/// asset
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct PageEntry {
    /// index into the asset table
    pub asset_index: u32,
    /// feature flags (reserved for future use)
    pub flags: u32,
}

/// section marker (chapter, vol, etc.)
///
/// defines hierarchical organization with optional parent references.
/// enables nested toc and bulk extraction
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Section {
    /// offset into string pool for section title
    pub title_offset: u32,
    /// first page index (0-based) where this section starts
    pub start_index: u32,
    /// parent section index, or 0xffffffff if root level
    pub parent_index: u32,
}

/// metadata k-v pair
///
/// stores arbitrary archive info (title, author, tags, etc.) using offsets into the deduped string
/// pool
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct Metadata {
    /// offset into string pool for key
    pub key_offset: u32,
    /// offset into string pool for value
    pub val_offset: u32,
}

/// BBF file footer (76 bytes)
///
/// located at the end of every BBF file. contains offsets to all tables and a hash of the entire
/// index structure for integrity verification
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct BbfFooter {
    /// offset to start of string pool
    pub string_pool_offset: u64,
    /// offset to start of asset table
    pub asset_table_offset: u64,
    /// number of entries in asset table
    pub asset_count: u32,
    /// offset to start of page table
    pub page_table_offset: u64,
    /// number of entries in page table
    pub page_count: u32,
    /// offset to start of section table
    pub section_table_offset: u64,
    /// number of entries in section table
    pub section_count: u32,
    /// offset to start of metadata table
    pub meta_table_offset: u64,
    /// number of entries in metadata table
    pub key_count: u32,
    /// reserved for future use
    pub extra_offset: u64,
    /// XXH3-64 hash of all index data (string pool through metadata table)
    pub index_hash: u64,
    /// magic number: b"BBF1" (for validation)
    pub magic: [u8; 4],
}

/// builder for making BBF files
///
/// provides a high-lvl interface for making BBF archives with automatic deduping, alignment, and
/// integrity hashing
pub struct BbfBuilder {
    /// buffered file writer
    pub writer: BufWriter<File>,
    /// current write pos in file
    pub current_offset: u64,
    /// list of all unique assets
    pub assets: Vec<AssetEntry>,
    /// logical page order
    pub pages: Vec<PageEntry>,
    /// section markers
    pub sections: Vec<Section>,
    /// metadata entries
    pub metadata: Vec<Metadata>,
    /// deduped string pool (`\0` terminated utf8)
    pub string_pool: Vec<u8>,
    /// hash-to-asset-idx map for deduplication
    pub dedupe_map: HashMap<u64, u32>,
    /// string-to-offset map for string pool deduplication
    pub string_map: HashMap<String, u32>,
}

impl BbfBuilder {
    /// makes a new BBF builder
    ///
    /// opens the output file and writes the header
    ///
    /// # Arguments
    ///
    /// * `output_path` - path where the BBF file will be made
    ///
    /// # Errors
    ///
    /// returns an error if the file can't be made or written
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

    /// write padding bytes to align to the next 4KB boundary
    ///
    /// this makes sure that all assets start at section-aligned offsets for good DirectStorage and
    /// SSD/NVMe perf
    fn align_padding(&mut self) -> Result<()> {
        let padding = (ALIGNMENT - (self.current_offset % ALIGNMENT)) % ALIGNMENT;

        if padding > 0 {
            let zeros = vec![0u8; padding as usize];
            self.writer.write_all(&zeros)?;
            self.current_offset += padding;
        }

        Ok(())
    }

    /// calculate XXH3-64 hash of data
    ///
    /// used for both asset deduplication and integrity verification
    fn calculate_hash(data: &[u8]) -> u64 {
        let mut hasher = XxHash3_64::with_seed(0);
        hasher.write(data);
        hasher.finish()
    }

    /// add a page to the book
    ///
    /// if an identical page (by hash) already exists, it'll be referenced rather than stored again
    ///
    /// # Arguments
    ///
    /// * `image_path` - path to the image file
    /// * `media_type` - media type of the image
    ///
    /// # Errors
    ///
    /// returns an error if the file can't be read or written
    ///
    /// # Examples
    ///
    /// ```no_run
    /// # use boundbook::{BbfBuilder, MediaType};
    /// # fn main() -> boundbook::Result<()> {
    /// let mut builder = BbfBuilder::new("out.bbf")?;
    /// builder.add_page("cover.png", MediaType::Png)?;
    /// builder.add_page("page001.avif", MediaType::Avif)?;
    /// # builder.finalize()?;
    /// # Ok(())
    /// # }
    /// ```
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

    /// get or add a string to the string pool
    ///
    /// strings are deduped and stored as null-terminated utf8. returns the offset into the string
    /// pool
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

    /// add a section marker (chapter, vol, etc.)
    ///
    /// # Arguments
    ///
    /// * `title` - display name for this section
    /// * `start_page` - zero-based index of the first page in this section
    /// * `parent` - optional parent section index for hierarchical organization
    pub fn add_section(&mut self, title: &str, start_page: u32, parent: Option<u32>) -> Result<()> {
        let title_offset = self.get_or_add_string(title);

        self.sections.push(Section {
            title_offset,
            start_index: start_page,
            parent_index: parent.unwrap_or(0xFFFFFFFF),
        });

        Ok(())
    }

    /// add a metadata k-v pair
    ///
    /// # Arguments
    ///
    /// * `key` - metadata key (e.g., "Title", "Author", "Tags")
    /// * `val` - metadata val
    pub fn add_metadata(&mut self, key: &str, val: &str) -> Result<()> {
        let key_offset = self.get_or_add_string(key);
        let val_offset = self.get_or_add_string(val);

        self.metadata.push(Metadata {
            key_offset,
            val_offset,
        });

        Ok(())
    }

    /// finalize the BBF file
    ///
    /// writes the string pool, all index tables, and the footer with integrity hashes. must be
    /// called to complete the file
    ///
    /// # Errors
    ///
    /// returns an error if any write operation fails
    pub fn finalize(mut self) -> Result<()> {
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

/// reader for accessing BBF files
///
/// uses memory-mapped IO for fast random access without loading the entire file. supports
/// integrity verification and parallel hash checking
pub struct BbfReader {
    /// mem-mapped file data
    mmap: memmap2::Mmap,
    /// parsed header
    header: BbfHeader,
    /// parsed footer
    footer: BbfFooter,
}

impl BbfReader {
    /// open and mem-map a BBF file
    ///
    /// # Arguments
    ///
    /// * `path` - path to the BBF file
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - file can't be opened
    /// - file is too small
    /// - magic number is invalid
    /// - mem mapping fails
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

    /// get a null-terminated string from the string pool
    ///
    /// # Arguments
    ///
    /// * `offset` - byte offset into the string pool
    ///
    /// # Errors
    ///
    /// returns an error if the offset is OOB or the data isn't valid utf8
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

    /// get a slice of all asset entries
    ///
    /// # Safety
    ///
    /// this function interprets raw bytes as [`AssetEntry`] structures. the file format guarantees
    /// correct alignment
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

    /// get a slice of all page entries
    ///
    /// pages are in logical reading order
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

    /// get a slice of all section entries
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

    /// get a slice of all metadata entries
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

    /// get the raw data for an asset
    ///
    /// returns a zero-copy slice directly from the mem-mapped file
    ///
    /// # Arguments
    ///
    /// * `asset` - asset entry to retrieve data for
    pub fn get_asset_data(&self, asset: &AssetEntry) -> &[u8] {
        &self.mmap[asset.offset as usize..(asset.offset + asset.length) as usize]
    }

    /// get the BBF format version
    pub fn version(&self) -> u8 {
        self.header.version
    }

    /// get the total number of pages
    pub fn page_count(&self) -> u32 {
        self.footer.page_count
    }

    /// get the total number of assets
    pub fn asset_count(&self) -> u32 {
        self.footer.asset_count
    }

    /// verify the integrity of the entire file
    ///
    /// performs 2 levels of verification:
    /// 1. verifies the index hash (string pool through metadata table)
    /// 2. verifies all asset hashes in parallel using rayon
    ///
    /// # Performance
    ///
    /// this op uses parallel processing and is WAY faster than sequential verification, especially
    /// on multi-core systems
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if all integrity checks pass
    /// - `Ok(false)` if any hash mismatch is detected
    pub fn verify_integrity(&self) -> Result<bool> {
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

    /// verify the integrity of a single asset
    ///
    /// # Arguments
    ///
    /// * `index` - index of the asset to verify
    ///
    /// # Errors
    ///
    /// returns an error if the index is OOB
    ///
    /// # Returns
    ///
    /// - `Ok(true)` if the asset hash matches
    /// - `Ok(false)` if the asset is corrupted
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
