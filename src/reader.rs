use {
    crate::prelude::*,
    rayon::iter::{IntoParallelRefIterator, ParallelIterator},
    std::{fs::File, path::Path},
};

/// a BBF file reader
///
/// provides ro access to BBF files via mem-mapping for efficient random access to assets, pages,
/// sections, and metadata. supports integrity verification via hash checking.
#[derive(Debug)]
pub struct BbfReader {
    /// mem-mapped file contents
    mmap: memmap2::Mmap,
    /// parsed file header
    header: BbfHeader,
    /// parsed file footer with all index info
    footer: BbfFooter,
}

impl BbfReader {
    /// opens and validates a BBF file
    ///
    /// mem-maps the file for efficient access, validates the magic num, checks reserved fields,
    /// verifies footer offset is within bounds, and validates index region size constraints
    ///
    /// # Arguments
    ///
    /// * `path` - path to the BBF file to open
    ///
    /// # Returns
    ///
    /// a `BbfReader` instance ready for querying assets, pages, sections, and metadata
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - file opening fails
    /// - mem mapping fails
    /// - file is too small to contain header and footer
    /// - magic number is not "BBF3"
    /// - footer offset is out of bounds
    /// - index region exceeds max_bale_size
    /// - arithmetic operations overflow (see [`macroni_n_cheese::mathinator2000`])
    ///
    /// # Safety
    ///
    /// uses unsafe for mem-mapping and reading structs from raw ptrs. mem-mapping is safe because
    /// the file is opened ro and the mmap lifetime is tied to the reader. struct reads are safe
    /// because bounds are checked and structs are #[repr(c, packed)].
    #[macroni_n_cheese::mathinator2000]
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path)?;
        let mmap = unsafe { memmap2::Mmap::map(&file)? };

        if mmap.len() < std::mem::size_of::<BbfHeader>() + std::mem::size_of::<BbfFooter>() {
            return Err(BbfError::FileTooSmall);
        }

        let header: BbfHeader = unsafe { Self::read_struct(&mmap, 0)? };

        if &header.magic != MAGIC {
            return Err(BbfError::InvalidMagic);
        }

        if header.reserved_extra != 0 {
            eprintln!("Warning: BBF header has nonzero reserved_extra field");
        }

        if header.reserved != [0; 40] {
            eprintln!("Warning: BBF header has nonzero reserved fields");
        }

        if header.footer_offset as usize + std::mem::size_of::<BbfFooter>() > mmap.len() {
            return Err(BbfError::InvalidOffset(
                "Footer offset out of bounds".to_string(),
            ));
        }

        let footer: BbfFooter = unsafe { Self::read_struct(&mmap, header.footer_offset as usize)? };

        if footer.flags != 0 {
            eprintln!("Warning: BBF footer has nonzero flags");
        }

        if footer.padding != [0; 3] {
            eprintln!("Warning: BBF footer has nonzero padding");
        }

        if footer.reserved != [0; 144] {
            eprintln!("Warning: BBF footer has nonzero reserved fields");
        }

        let index_size = mmap.len() as u64 - footer.asset_offset;
        if index_size > MAX_BALE_SIZE {
            return Err(BbfError::Other(format!(
                "Index region too large: {} bytes",
                index_size
            )));
        }

        Ok(Self {
            mmap,
            header,
            footer,
        })
    }

    /// reads a struct from the mem-mapped file at the given offset
    ///
    /// validates that the offset and struct size are within file bounds, then reads the struct
    /// using unaligned ptr access (required for packed structs).
    ///
    /// # Arguments
    ///
    /// * `mmap` - the mem-mapped file
    /// * `offset` - byte offset where the struct begins
    ///
    /// # Returns
    ///
    /// a copy of the struct read from the file
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - offset plus struct size exceeds file length
    /// - arithmetic operations overflow (see [`macroni_n_cheese::mathinator2000`])
    ///
    /// # Safety
    ///
    /// uses unsafe to read from a raw ptr. this is safe because bounds are validated before
    /// reading and unaligned reads handle packed struct layouts correctly.
    #[macroni_n_cheese::mathinator2000]
    unsafe fn read_struct<T: Copy>(mmap: &memmap2::Mmap, offset: usize) -> Result<T> {
        if offset + std::mem::size_of::<T>() > mmap.len() {
            return Err(BbfError::InvalidOffset(format!(
                "Struct read at offset {} exceeds file size",
                offset
            )));
        }

        unsafe {
            Ok(std::ptr::read_unaligned(
                mmap.as_ptr().add(offset) as *const T
            ))
        }
    }

    /// gets a string from the string pool at the given offset
    ///
    /// reads a null-terminated utf-8 string from the string pool. scans up to max_forme_size bytes
    /// for the null terminator to prevent unbounded scanning.
    ///
    /// # Arguments
    ///
    /// * `offset` - byte offset within the string pool (relative to pool start)
    ///
    /// # Returns
    ///
    /// a string slice borrowed from the mem-mapped file, valid for the reader's lifetime
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - offset is beyond the string pool size
    /// - string pool offset calculations overflow
    /// - no null terminator found within max_forme_size bytes
    /// - string contains invalid utf-8
    /// - arithmetic operations overflow (see [`macroni_n_cheese::mathinator2000`])
    #[macroni_n_cheese::mathinator2000]
    pub fn get_string(&self, offset: u64) -> Result<&str> {
        let pool_start = self.footer.string_pool_offset as usize;
        let pool_size = self.footer.string_pool_size as usize;

        if offset as usize >= pool_size {
            return Err(BbfError::InvalidOffset(format!(
                "String offset {} out of bounds",
                offset
            )));
        }

        let start = pool_start + offset as usize;
        let pool_end = pool_start
            .checked_add(pool_size)
            .ok_or_else(|| BbfError::InvalidOffset("String pool offset + size overflow".into()))?;

        if start >= self.mmap.len() || pool_end > self.mmap.len() {
            return Err(BbfError::InvalidOffset(
                "String pool out of bounds".to_string(),
            ));
        }

        let data = &self.mmap[start..pool_end];
        let scan_limit = MAX_FORME_SIZE.min(data.len() as u64) as usize;
        let str_end = data[..scan_limit]
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| BbfError::InvalidUtf8)?;

        std::str::from_utf8(&data[..str_end]).map_err(|_| BbfError::InvalidUtf8)
    }

    /// returns a slice of all asset entries
    ///
    /// provides direct access to the asset table via the mem-mapped file. validates that the
    /// asset table is within file bounds.
    ///
    /// # Returns
    ///
    /// a slice of all asset entries in the file
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - asset table size calculation overflows
    /// - asset table extends beyond file bounds
    pub fn assets(&self) -> Result<&[AssetEntry]> {
        let offset = self.footer.asset_offset as usize;
        let count = self.footer.asset_count as usize;
        let size = count
            .checked_mul(std::mem::size_of::<AssetEntry>())
            .ok_or_else(|| {
                BbfError::InvalidOffset("Asset table size calculation overflow".into())
            })?;

        let end = offset
            .checked_add(size)
            .ok_or_else(|| BbfError::InvalidOffset("Asset table offset + size overflow".into()))?;

        if end > self.mmap.len() {
            return Err(BbfError::InvalidOffset("Asset table out of bounds".into()));
        }

        unsafe {
            Ok(std::slice::from_raw_parts(
                self.mmap.as_ptr().add(offset) as *const AssetEntry,
                count,
            ))
        }
    }

    /// returns a slice of all page entries
    ///
    /// provides direct access to the page table via the mem-mapped file. validates that the
    /// page table is within file bounds.
    ///
    /// # Returns
    ///
    /// a slice of all page entries in the file
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - page table size calculation overflows
    /// - page table extends beyond file bounds
    pub fn pages(&self) -> Result<&[PageEntry]> {
        let offset = self.footer.page_offset as usize;
        let count = self.footer.page_count as usize;

        let size = count
            .checked_mul(std::mem::size_of::<PageEntry>())
            .ok_or_else(|| {
                BbfError::InvalidOffset("Page table size calculation overflow".into())
            })?;

        let end = offset
            .checked_add(size)
            .ok_or_else(|| BbfError::InvalidOffset("Page table offset + size overflow".into()))?;

        if end > self.mmap.len() {
            return Err(BbfError::InvalidOffset("Page table out of bounds".into()));
        }

        unsafe {
            Ok(std::slice::from_raw_parts(
                self.mmap.as_ptr().add(offset) as *const PageEntry,
                count,
            ))
        }
    }

    /// returns a slice of all section entries
    ///
    /// provides direct access to the section table via the mem-mapped file. validates that the
    /// section table is within file bounds.
    ///
    /// # Returns
    ///
    /// a slice of all section entries in the file
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - section table size calculation overflows
    /// - section table extends beyond file bounds
    pub fn sections(&self) -> Result<&[Section]> {
        let offset = self.footer.section_offset as usize;
        let count = self.footer.section_count as usize;

        let size = count
            .checked_mul(std::mem::size_of::<Section>())
            .ok_or_else(|| {
                BbfError::InvalidOffset("Section table size calculation overflow".into())
            })?;

        let end = offset.checked_add(size).ok_or_else(|| {
            BbfError::InvalidOffset("Section table offset + size overflow".into())
        })?;

        if end > self.mmap.len() {
            return Err(BbfError::InvalidOffset(
                "Section table out of bounds".into(),
            ));
        }

        unsafe {
            Ok(std::slice::from_raw_parts(
                self.mmap.as_ptr().add(offset) as *const Section,
                count,
            ))
        }
    }

    /// returns a slice of all metadata entries
    ///
    /// provides direct access to the metadata table via the mem-mapped file. validates that the
    /// metadata table is within file bounds.
    ///
    /// # Returns
    ///
    /// a slice of all metadata entries in the file
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - metadata table size calculation overflows
    /// - metadata table extends beyond file bounds
    pub fn metadata(&self) -> Result<&[Metadata]> {
        let offset = self.footer.meta_offset as usize;
        let count = self.footer.meta_count as usize;

        let size = count
            .checked_mul(std::mem::size_of::<Metadata>())
            .ok_or_else(|| {
                BbfError::InvalidOffset("Metadata table size calculation overflow".into())
            })?;

        let end = offset.checked_add(size).ok_or_else(|| {
            BbfError::InvalidOffset("Metadata table offset + size overflow".into())
        })?;

        if end > self.mmap.len() {
            return Err(BbfError::InvalidOffset(
                "Metadata table out of bounds".into(),
            ));
        }

        unsafe {
            Ok(std::slice::from_raw_parts(
                self.mmap.as_ptr().add(offset) as *const Metadata,
                count,
            ))
        }
    }

    /// retrieves the raw binary data for an asset
    ///
    /// returns a slice of the mem-mapped file containing the asset's image data. validates that
    /// the asset's file offset and size are within bounds.
    ///
    /// # Arguments
    ///
    /// * `asset` - the asset entry whose data should be retrieved
    ///
    /// # Returns
    ///
    /// a byte slice containing the asset's raw image data
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - asset offset plus size overflows
    /// - asset data extends beyond file bounds
    pub fn get_asset_data(&self, asset: &AssetEntry) -> Result<&[u8]> {
        let start = asset.file_offset as usize;

        let end = start
            .checked_add(asset.file_size as usize)
            .ok_or_else(|| BbfError::InvalidOffset("Asset offset + size overflow".into()))?;

        if end > self.mmap.len() {
            return Err(BbfError::InvalidOffset("Asset data out of bounds".into()));
        }

        Ok(&self.mmap[start..end])
    }

    /// returns the BBF format version number
    ///
    /// # Returns
    ///
    /// the version number from the file header
    pub const fn version(&self) -> u16 {
        self.header.version
    }

    /// returns the total number of pages
    ///
    /// # Returns
    ///
    /// the page count from the file footer
    pub const fn page_count(&self) -> u64 {
        self.footer.page_count
    }

    /// returns the total number of unique assets
    ///
    /// # Returns
    ///
    /// the asset count from the file footer (after deduplication)
    pub const fn asset_count(&self) -> u64 {
        self.footer.asset_count
    }

    /// verifies the integrity of the entire file
    ///
    /// validates both the footer hash (covering all index data) and all individual asset hashes in
    /// parallel. this ensures the file has not been corrupted or tampered with.
    ///
    /// # Returns
    ///
    /// true if all hashes match, false if any hash mismatch is detected
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - index region bounds are invalid
    /// - retrieving asset data fails
    pub fn verify_integrity(&self) -> Result<bool> {
        let meta_start = self.footer.asset_offset as usize;
        let footer_start = self.header.footer_offset as usize;

        if meta_start >= footer_start || footer_start > self.mmap.len() {
            return Err(BbfError::InvalidOffset(
                "Invalid index region bounds".into(),
            ));
        }

        let calc_hash = BbfBuilder::calculate_hash_64(&self.mmap[meta_start..footer_start]);

        if calc_hash != self.footer.footer_hash {
            return Ok(false);
        }

        let assets = self.assets()?;
        let all_valid = assets.par_iter().all(|asset| {
            if let Ok(data) = self.get_asset_data(asset) {
                let hash_128 = BbfBuilder::calculate_hash_128(data);
                let hash_low = hash_128 as u64;
                let hash_high = (hash_128 >> 64) as u64;
                hash_low == asset.asset_hash[0] && hash_high == asset.asset_hash[1]
            } else {
                false
            }
        });

        Ok(all_valid)
    }

    /// verifies the integrity of a single asset
    ///
    /// calculates the xxh3 128-bit hash of the asset's data and compares it to the stored hash
    /// in the asset entry.
    ///
    /// # Arguments
    ///
    /// * `index` - the asset index to verify
    ///
    /// # Returns
    ///
    /// true if the asset's hash matches, false otherwise
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - asset index is out of bounds
    /// - retrieving asset data fails
    pub fn verify_asset(&self, index: usize) -> Result<bool> {
        let assets = self.assets()?;
        if index >= assets.len() {
            return Err(BbfError::InvalidOffset(format!("Asset index {}", index)));
        }

        let asset = &assets[index];
        let data = self.get_asset_data(asset)?;
        let hash_128 = BbfBuilder::calculate_hash_128(data);
        let hash_low = hash_128 as u64;
        let hash_high = (hash_128 >> 64) as u64;

        Ok(hash_low == asset.asset_hash[0] && hash_high == asset.asset_hash[1])
    }

    /// returns a reference to the file header
    ///
    /// # Returns
    ///
    /// the parsed BBF header containing version, alignment, and footer offset info
    pub const fn header(&self) -> &BbfHeader {
        &self.header
    }

    /// returns a reference to the file footer
    ///
    /// # Returns
    ///
    /// the parsed bbf footer containing all table offsets, counts, and integrity hash
    pub const fn footer(&self) -> &BbfFooter {
        &self.footer
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        unused,
        clippy::undocumented_unsafe_blocks,
        clippy::missing_safety_doc,
        clippy::missing_panics_doc
    )]
    use {
        super::*,
        crate::{_types::VERSION, BbfBuilder},
        assert2::check as assert,
        std::io::Write,
        tempfile::NamedTempFile,
    };

    unsafe fn read_unaligned<T>(val: T) -> T {
        let field_ptr = std::ptr::addr_of!(val);
        unsafe { field_ptr.read_unaligned() }
    }

    fn create_test_bbf_file() -> NamedTempFile {
        let temp_output = NamedTempFile::new().unwrap();
        let test_image = NamedTempFile::new().unwrap();
        test_image.as_file().write_all(&vec![1u8; 1024]).unwrap();

        let mut builder = BbfBuilder::with_defaults(temp_output.path()).unwrap();
        builder.add_page(test_image.path(), 0, 0).unwrap();
        builder.add_section("Chapter 1", 0, None);
        builder.add_metadata("title", "Test Book", None);
        builder.finalize().unwrap();

        temp_output
    }

    #[test]
    fn test_reader_opens_valid_file() {
        let test_file = create_test_bbf_file();
        let reader = BbfReader::open(test_file.path()).unwrap();

        assert!(reader.version() == VERSION);
        assert!(reader.page_count() == 1);
        assert!(reader.asset_count() == 1);
    }

    #[test]
    fn test_reader_rejects_invalid_magic() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(b"BADF").unwrap();
        temp_file.write_all(&[0u8; 100]).unwrap();

        let result = BbfReader::open(temp_file.path());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BbfError::InvalidMagic));
    }

    #[test]
    fn test_reader_rejects_too_small_file() {
        let mut temp_file = NamedTempFile::new().unwrap();
        temp_file.write_all(MAGIC).unwrap();

        let result = BbfReader::open(temp_file.path());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), BbfError::FileTooSmall));
    }

    #[test]
    fn test_reader_gets_assets() {
        let test_file = create_test_bbf_file();
        let reader = BbfReader::open(test_file.path()).unwrap();

        let assets = reader.assets().unwrap();
        assert!(assets.len() == 1);
        assert!(unsafe { read_unaligned(assets[0].file_size) } == 1024);
    }

    #[test]
    fn test_reader_gets_pages() {
        let test_file = create_test_bbf_file();
        let reader = BbfReader::open(test_file.path()).unwrap();

        let pages = reader.pages().unwrap();
        assert!(pages.len() == 1);
        assert!(unsafe { read_unaligned(pages[0].asset_index) } == 0);
    }

    #[test]
    fn test_reader_gets_sections() {
        let test_file = create_test_bbf_file();
        let reader = BbfReader::open(test_file.path()).unwrap();

        let sections = reader.sections().unwrap();
        assert!(sections.len() == 1);
    }

    #[test]
    fn test_reader_gets_metadata() {
        let test_file = create_test_bbf_file();
        let reader = BbfReader::open(test_file.path()).unwrap();

        let metadata = reader.metadata().unwrap();
        assert!(metadata.len() == 1);
    }

    #[test]
    fn test_reader_gets_string() {
        let test_file = create_test_bbf_file();
        let reader = BbfReader::open(test_file.path()).unwrap();

        let sections = reader.sections().unwrap();
        let title = reader.get_string(sections[0].section_title_offset).unwrap();
        assert!(title == "Chapter 1");
    }

    #[test]
    fn test_reader_gets_asset_data() {
        let test_file = create_test_bbf_file();
        let reader = BbfReader::open(test_file.path()).unwrap();

        let assets = reader.assets().unwrap();
        let data = reader.get_asset_data(&assets[0]).unwrap();
        assert!(data.len() == 1024);
        assert!(data.iter().all(|&b| b == 1));
    }

    #[test]
    fn test_verify_integrity_valid_file() {
        let test_file = create_test_bbf_file();
        let reader = BbfReader::open(test_file.path()).unwrap();

        let is_valid = reader.verify_integrity().unwrap();
        assert!(is_valid);
    }

    #[test]
    fn test_verify_asset_valid() {
        let test_file = create_test_bbf_file();
        let reader = BbfReader::open(test_file.path()).unwrap();

        let is_valid = reader.verify_asset(0).unwrap();
        assert!(is_valid);
    }

    #[test]
    fn test_verify_asset_invalid_index() {
        let test_file = create_test_bbf_file();
        let reader = BbfReader::open(test_file.path()).unwrap();

        let result = reader.verify_asset(999);
        assert!(result.is_err());
    }

    #[test]
    fn test_string_offset_out_of_bounds() {
        let test_file = create_test_bbf_file();
        let reader = BbfReader::open(test_file.path()).unwrap();

        let result = reader.get_string(u64::MAX);
        assert!(result.is_err());
    }
}
