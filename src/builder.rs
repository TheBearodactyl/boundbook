use {
    crate::prelude::*,
    hashbrown::HashMap,
    miette::IntoDiagnostic,
    std::{
        fs::File,
        io::{BufWriter, Seek, Write},
        path::Path,
    },
    xxhash_rust::xxh3::{Xxh3, xxh3_128},
};

/// a BBF file builder
///
/// provides methods for making a bound book format (BBF) file with assets, pages, sections, and
/// metadata. this builder handles deduplication of assets, string pooling, alignment, and integrity
/// hashing.
pub struct BbfBuilder {
    /// buffered writer for the output file
    writer: BufWriter<File>,
    /// current byte offset in the output file
    current_offset: u64,
    /// collection of asset entries (deduped image data)
    assets: Vec<AssetEntry>,
    /// collection of page entries (refs to assets)
    pages: Vec<PageEntry>,
    /// collection of section entries (chapter/section organization)
    sections: Vec<Section>,
    /// collection of metadata k-v pairs
    metadata: Vec<Metadata>,
    /// pooled null-terminated strings for efficient storage
    string_pool: Vec<u8>,
    /// maps asset hashes to their indices for deduplication
    dedupe_map: HashMap<u128, u64>,
    /// maps strings to their offsets in the string pool
    string_map: HashMap<String, u64>,
    /// alignment exponent for guard alignment (actual bytes = 1 << guard_value)
    guard_value: u8,
    /// ream size exponent for small asset threshold (actual bytes = 1 << ream_value)
    ream_value: u8,
    /// flags for header config
    header_flags: u32,
}

impl BbfBuilder {
    /// makes a new BBF builder with custom alignment and ream size
    ///
    /// initializes a new BBF file at the specified path and writes the header. validates that
    /// alignment and ream size exponents are within acceptable bounds. prompts user for
    /// confirmation if alignment exponent exceeds 16 to prevent excessive fragmentation.
    ///
    /// # Arguments
    ///
    /// * `output_path` - path where the BBF file will be made
    /// * `alignment` - alignment exp for asset data (actual alignment = 1 << alignment bytes)
    /// * `ream_size` - ream size exp for small asset threshold (actual size = 1 << ream_size bytes)
    /// * `flags` - header flags for config opts
    ///
    /// # Returns
    ///
    /// a new [`BbfBuilder`] instance ready to accept assets, pages, sections, and metadata
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - user declines the confirmation prompt for alignment > 16
    /// - alignment exponent exceeds 16 (64kb) without user confirmation
    /// - ream size exponent exceeds 16 (64kb)
    /// - file creation fails
    /// - writing the initial header fails
    pub fn new<P: AsRef<Path>>(
        output_path: P,
        alignment: u8,
        ream_size: u8,
        flags: u32,
    ) -> Result<Self> {
        if alignment > 16 {
            return Err(BbfError::Other {
                message: "Alignment exponent must not exceed 16 (64KB). This creates excessive fragmentation.".into()
            });
        }

        if ream_size > 16 {
            return Err(BbfError::Other {
                message: "Ream size exponent must not exceed 16 (64KB)".into(),
            });
        }

        let file = File::create(output_path)
            .into_diagnostic()
            .map_err(BbfError::from)?;
        let mut writer = BufWriter::new(file);

        let header = BbfHeader {
            magic: *MAGIC,
            version: VERSION,
            header_len: std::mem::size_of::<BbfHeader>() as u16,
            flags: 0,
            alignment: 0,
            ream_size: 0,
            reserved_extra: 0,
            footer_offset: 0,
            reserved: [0; 40],
        };

        Self::write_struct(&mut writer, &header)
            .into_diagnostic()
            .map_err(BbfError::from)?;
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
            guard_value: alignment,
            ream_value: ream_size,
            header_flags: flags,
        })
    }

    /// creates a new BBF builder with default settings
    ///
    /// initializes a builder with standard alignment (12, or 4kb) and ream size (16, or 64kb), and
    /// enables variable ream size flag for optimizing small assets
    ///
    /// # Arguments
    ///
    /// * `output_path` - path where the BBF file will be created
    ///
    /// # Returns
    ///
    /// a new `BbfBuilder` instance configured with default values
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - file creation fails
    /// - writing the initial header fails
    pub fn with_defaults<P: AsRef<Path>>(output_path: P) -> Result<Self> {
        Self::new(
            output_path,
            DEFAULT_GUARD_ALIGNMENT,
            DEFAULT_SMALL_REAM_THRESHOLD,
            BBF_VARIABLE_REAM_SIZE_FLAG,
        )
        .into_diagnostic()
        .map_err(|e| e.into())
    }

    /// writes a struct directly to the buffered writer as raw bytes
    ///
    /// converts the struct to its raw byte representation and writes it to the file. this is used
    /// for writing fixed-size binary structures like headers and footers.
    ///
    /// # Arguments
    ///
    /// * `writer` - the buffered file writer
    /// * `data` - reference to the struct to write
    ///
    /// # Returns
    ///
    /// unit type on success, indicating the write operation completed
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - writing bytes to the writer fails
    ///
    /// # Safety
    ///
    /// uses unsafe to create a raw byte slice from the struct ptr. this is safe because the
    /// struct is #[repr(c, packed)] and all fields are simple types with no padding or refs.
    fn write_struct<T>(writer: &mut BufWriter<File>, data: &T) -> Result<()> {
        unsafe {
            let bytes =
                std::slice::from_raw_parts(data as *const T as *const u8, std::mem::size_of::<T>());
            writer.write_all(bytes).into_diagnostic()?;
        }
        Ok(())
    }

    /// aligns the current file offset to the specified alignment boundary
    ///
    /// calculates the padding needed to reach the next alignment boundary and writes zeros to fill
    /// the gap. this ensures assets are correctly aligned for efficient mem-mapped access.
    ///
    /// # Arguments
    ///
    /// * `alignment_bytes` - the alignment boundary in bytes (must be power of 2)
    ///
    /// # Returns
    ///
    /// unit type on success, indicating alignment padding was added
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - writing padding zeros fails
    /// - arithmetic operations overflow (protected by macroni_n_cheese::mathinator2000)
    #[macroni_n_cheese::mathinator2000]
    fn align_padding(&mut self, alignment_bytes: u64) -> Result<()> {
        let remainder = self.current_offset % alignment_bytes;
        if remainder == 0 {
            return Ok(());
        }

        let padding = alignment_bytes - remainder;
        let zeros = vec![0u8; padding as usize];
        self.writer.write_all(&zeros).into_diagnostic()?;
        self.current_offset += padding;
        Ok(())
    }

    /// calculates the 128-bit xxh3 hash of the given data
    ///
    /// computes a 128-bit hash using the xxh3 algo, used for asset deduplication and integrity
    /// verification.
    ///
    /// # Arguments
    ///
    /// * `data` - the byte slice to hash
    ///
    /// # Returns
    ///
    /// the 128-bit hash value as a u128
    pub fn calculate_hash_128(data: &[u8]) -> u128 {
        xxh3_128(data)
    }

    /// calculates the 64-bit xxh3 hash of the given data
    ///
    /// computes a 64-bit hash using the xxh3 algorithm, used for footer integrity verification.
    ///
    /// # Arguments
    ///
    /// * `data` - the byte slice to hash
    ///
    /// # Returns
    ///
    /// the 64-bit hash value as a u64
    pub fn calculate_hash_64(data: &[u8]) -> u64 {
        let mut hasher = Xxh3::new();
        hasher.update(data);
        hasher.digest()
    }

    /// adds a page (image) to the book
    ///
    /// reads the image file, calculates its hash, and either reuses an existing asset
    /// (deduplication) or adds a new asset entry. applies appropriate alignment based on file
    /// size and configuration. creates a page entry that references the asset.
    ///
    /// # Arguments
    ///
    /// * `image_path` - path to the image file to add
    /// * `page_flags` - flags for page-specific configuration
    /// * `asset_flags` - flags for asset-specific configuration
    ///
    /// # Returns
    ///
    /// unit type on success, indicating the page was added
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - reading the image file fails
    /// - arithmetic operations overflow (protected by macroni_n_cheese::mathinator2000)
    /// - writing image data to the buffer fails
    /// - aligning padding fails
    #[macroni_n_cheese::mathinator2000]
    pub fn add_page<P: AsRef<Path>>(
        &mut self,
        image_path: P,
        page_flags: u32,
        asset_flags: u32,
    ) -> Result<()> {
        let data = std::fs::read(image_path.as_ref()).into_diagnostic()?;
        let hash_128 = Self::calculate_hash_128(&data);

        let asset_index = if let Some(&idx) = self.dedupe_map.get(&hash_128) {
            idx
        } else {
            let alignment_bytes = 1u64 << self.guard_value;
            let threshold_bytes = 1u64 << self.ream_value;

            let variable_align = self.header_flags & BBF_VARIABLE_REAM_SIZE_FLAG != 0;

            let actual_alignment = if variable_align && (data.len() as u64) < threshold_bytes {
                8
            } else {
                alignment_bytes
            };

            self.align_padding(actual_alignment).into_diagnostic()?;

            let media_type = MediaType::from_extension(
                image_path
                    .as_ref()
                    .extension()
                    .and_then(|s| s.to_str())
                    .unwrap_or(""),
            );

            let asset = AssetEntry {
                file_offset: self.current_offset,
                asset_hash: [hash_128 as u64, (hash_128 >> 64) as u64],
                file_size: data.len() as u64,
                flags: asset_flags,
                reserved_value: 0,
                media_type: media_type as u8,
                reserved: [0; 9],
            };

            self.writer.write_all(&data).into_diagnostic()?;
            self.current_offset += data.len() as u64;

            let idx = self.assets.len() as u64;
            self.assets.push(asset);
            self.dedupe_map.insert(hash_128, idx);
            idx
        };

        self.pages.push(PageEntry {
            asset_index,
            flags: page_flags,
            reserved: [0; 4],
        });

        Ok(())
    }

    /// gets or adds a string to the string pool
    ///
    /// checks if the string already exists in the pool and returns its offset, or adds it as a
    /// null-terminated string if new. this enables efficient storage of repeated strings like
    /// section titles and metadata keys.
    ///
    /// # Arguments
    ///
    /// * `s` - the string to add or retrieve
    ///
    /// # Returns
    ///
    /// the byte offset of the string in the string pool
    fn get_or_add_string(&mut self, s: &str) -> u64 {
        if let Some(&offset) = self.string_map.get(s) {
            return offset;
        }

        let offset = self.string_pool.len() as u64;
        self.string_pool.extend_from_slice(s.as_bytes());
        self.string_pool.push(0);
        self.string_map.insert(s.to_string(), offset);
        offset
    }

    /// adds a section (chapter/part) to the book
    ///
    /// creates a section entry with a title, starting page index, and optional parent section.
    /// sections organize pages into hierarchical structures like chapters and sub-chapters.
    ///
    /// # Arguments
    ///
    /// * `title` - the section title (stored in string pool)
    /// * `start_index` - the first page index in this section
    /// * `parent` - optional parent section title for hierarchical organization
    ///
    /// # Returns
    ///
    /// unit type on success (sections are stored internally)
    pub fn add_section(&mut self, title: &str, start_index: u64, parent: Option<&str>) {
        let title_offset = self.get_or_add_string(title);
        let parent_offset = parent
            .map(|p| self.get_or_add_string(p))
            .unwrap_or(u64::MAX);

        self.sections.push(Section {
            section_title_offset: title_offset,
            section_start_index: start_index,
            section_parent_offset: parent_offset,
            reserved: [0; 8],
        });
    }

    /// adds a metadata key-val pair to the book
    ///
    /// stores arbitrary metadata like author, title, publisher, or isbn. metadata can optionally
    /// be associated with a parent section.
    ///
    /// # Arguments
    ///
    /// * `key` - the metadata key (stored in string pool)
    /// * `val` - the metadata val (stored in string pool)
    /// * `parent` - optional parent section for section-specific metadata
    ///
    /// # Returns
    ///
    /// unit type on success (metadata is stored internally)
    pub fn add_metadata(&mut self, key: &str, val: &str, parent: Option<&str>) {
        let key_offset = self.get_or_add_string(key);
        let val_offset = self.get_or_add_string(val);
        let parent_offset = parent
            .map(|p| self.get_or_add_string(p))
            .unwrap_or(u64::MAX);

        self.metadata.push(Metadata {
            key_offset,
            value_offset: val_offset,
            parent_offset,
            reserved: [0; 8],
        });
    }

    /// finalizes the book file and writes all indices
    ///
    /// writes all asset, page, section, and metadata tables to the file, followed by the string
    /// pool. calculates an integrity hash over all index data, writes the footer with all
    /// offsets and counts, then seeks back to the beginning to update the header with final
    /// values. flushes and syncs all data to disk.
    ///
    /// # Returns
    ///
    /// unit type on success, indicating the BBF file is complete and ready for use
    ///
    /// # Errors
    ///
    /// returns an error if:
    /// - writing any table data fails
    /// - arithmetic operations overflow (protected by macroni_n_cheese::mathinator2000)
    /// - writing the footer fails
    /// - flushing the buffer fails
    /// - syncing to disk fails
    /// - extracting the inner file from bufwriter fails
    /// - seeking to the start of the file fails
    /// - writing the updated header fails
    #[macroni_n_cheese::mathinator2000]
    pub fn finalize(mut self) -> Result<()> {
        let mut hasher = Xxh3::new();

        let assets_bytes = unsafe {
            std::slice::from_raw_parts(
                self.assets.as_ptr() as *const u8,
                self.assets.len() * std::mem::size_of::<AssetEntry>(),
            )
        };
        let offset_assets = self.current_offset;
        self.writer.write_all(assets_bytes).into_diagnostic()?;
        hasher.update(assets_bytes);
        self.current_offset += assets_bytes.len() as u64;

        let pages_bytes = unsafe {
            std::slice::from_raw_parts(
                self.pages.as_ptr() as *const u8,
                self.pages.len() * std::mem::size_of::<PageEntry>(),
            )
        };
        let offset_pages = self.current_offset;
        self.writer.write_all(pages_bytes).into_diagnostic()?;
        hasher.update(pages_bytes);
        self.current_offset += pages_bytes.len() as u64;

        let sections_bytes = unsafe {
            std::slice::from_raw_parts(
                self.sections.as_ptr() as *const u8,
                self.sections.len() * std::mem::size_of::<Section>(),
            )
        };
        let offset_sections = self.current_offset;
        self.writer.write_all(sections_bytes).into_diagnostic()?;
        hasher.update(sections_bytes);
        self.current_offset += sections_bytes.len() as u64;

        let metadata_bytes = unsafe {
            std::slice::from_raw_parts(
                self.metadata.as_ptr() as *const u8,
                self.metadata.len() * std::mem::size_of::<Metadata>(),
            )
        };
        let offset_meta = self.current_offset;
        self.writer.write_all(metadata_bytes).into_diagnostic()?;
        hasher.update(metadata_bytes);
        self.current_offset += metadata_bytes.len() as u64;

        let offset_strings = self.current_offset;
        let str_pool_size = self.string_pool.len() as u64;
        if str_pool_size > 0 {
            self.writer.write_all(&self.string_pool)?;
            hasher.update(&self.string_pool);
            self.current_offset += str_pool_size;
        }

        let index_hash = hasher.digest();
        let footer_offset = self.current_offset;

        let footer = BbfFooter {
            asset_offset: offset_assets,
            page_offset: offset_pages,
            section_offset: offset_sections,
            meta_offset: offset_meta,
            expansion_offset: 0,
            string_pool_offset: offset_strings,
            string_pool_size: str_pool_size,
            asset_count: self.assets.len() as u64,
            page_count: self.pages.len() as u64,
            section_count: self.sections.len() as u64,
            meta_count: self.metadata.len() as u64,
            expansion_count: 0,
            flags: 0,
            footer_len: std::mem::size_of::<BbfFooter>() as u8,
            padding: [0; 3],
            footer_hash: index_hash,
            reserved: [0; 144],
        };

        Self::write_struct(&mut self.writer, &footer).into_diagnostic()?;

        self.writer.flush().into_diagnostic()?;
        self.writer.get_mut().sync_all().into_diagnostic()?;

        let mut file = self.writer.into_inner().into_diagnostic()?;
        file.seek(std::io::SeekFrom::Start(0)).into_diagnostic()?;

        let header = BbfHeader {
            magic: *MAGIC,
            version: VERSION,
            header_len: std::mem::size_of::<BbfHeader>() as u16,
            flags: self.header_flags,
            alignment: self.guard_value,
            ream_size: self.ream_value,
            reserved_extra: 0,
            footer_offset,
            reserved: [0; 40],
        };

        unsafe {
            let bytes = std::slice::from_raw_parts(
                &header as *const BbfHeader as *const u8,
                std::mem::size_of::<BbfHeader>(),
            );
            file.write_all(bytes).into_diagnostic()?;
        }

        file.sync_all().into_diagnostic()?;
        Ok(())
    }

    /// returns the current number of assets
    ///
    /// # Returns
    ///
    /// count of unique assets added (after deduplication)
    pub const fn asset_count(&self) -> usize {
        self.assets.len()
    }

    /// returns the current number of pages
    ///
    /// # Returns
    ///
    /// count of pages added to the book
    pub const fn page_count(&self) -> usize {
        self.pages.len()
    }

    /// returns the current number of sections
    ///
    /// # Returns
    ///
    /// count of sections added to the book
    pub const fn section_count(&self) -> usize {
        self.sections.len()
    }

    /// returns the current number of metadata entries
    ///
    /// # Returns
    ///
    /// count of metadata key-value pairs added
    pub const fn metadata_count(&self) -> usize {
        self.metadata.len()
    }
}

#[cfg(test)]
mod tests {
    #![allow(unused, clippy::missing_panics_doc)]
    use {
        super::*,
        assert2::check as assert,
        std::io::Write,
        tempfile::{NamedTempFile, TempPath},
    };

    fn create_test_image(extension: &str, size: usize) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        let data = vec![0u8; size];

        file.write_all(&data).unwrap();
        file.flush().unwrap();

        let path = file.path().with_extension(extension);
        std::fs::copy(file.path(), &path).unwrap();
        NamedTempFile::from_parts(file.into_parts().0, TempPath::from_path(path))
    }

    #[test]
    fn test_builder_creates_valid_file() {
        let temp_output = NamedTempFile::new().unwrap();
        let builder = BbfBuilder::with_defaults(temp_output.path()).unwrap();
        assert!(builder.asset_count() == 0);
        assert!(builder.page_count() == 0);
    }

    #[test]
    fn test_builder_rejects_excessive_alignment() {
        let temp_output = NamedTempFile::new().unwrap();
        let result = BbfBuilder::new(temp_output.path(), 17, 12, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_builder_rejects_excessive_ream_size() {
        let temp_output = NamedTempFile::new().unwrap();
        let result = BbfBuilder::new(temp_output.path(), 12, 17, 0);
        assert!(result.is_err());
    }

    #[test]
    fn test_hash_calculation_128() {
        let data1 = b"test data";
        let data2 = b"test data";
        let data3 = b"different data";

        let hash1 = BbfBuilder::calculate_hash_128(data1);
        let hash2 = BbfBuilder::calculate_hash_128(data2);
        let hash3 = BbfBuilder::calculate_hash_128(data3);

        assert!(hash1 == hash2);
        assert!(hash1 != hash3);
    }

    #[test]
    fn test_hash_calculation_64() {
        let data1 = b"test data";
        let data2 = b"test data";
        let data3 = b"different data";

        let hash1 = BbfBuilder::calculate_hash_64(data1);
        let hash2 = BbfBuilder::calculate_hash_64(data2);
        let hash3 = BbfBuilder::calculate_hash_64(data3);

        assert!(hash1 == hash2);
        assert!(hash1 != hash3);
    }

    #[test]
    fn test_add_single_page() {
        let temp_output = NamedTempFile::new().unwrap();
        let test_image = create_test_image("png", 1024);
        let mut builder = BbfBuilder::with_defaults(temp_output.path()).unwrap();

        builder.add_page(test_image.path(), 0, 0).unwrap();

        assert!(builder.page_count() == 1);
        assert!(builder.asset_count() == 1);
    }

    #[test]
    fn test_asset_deduplication() {
        let temp_output = NamedTempFile::new().unwrap();
        let test_image = create_test_image("png", 1024);
        let mut builder = BbfBuilder::with_defaults(temp_output.path()).unwrap();

        builder.add_page(test_image.path(), 0, 0).unwrap();
        builder.add_page(test_image.path(), 0, 0).unwrap();

        assert!(builder.page_count() == 2);
        assert!(builder.asset_count() == 1);
    }

    #[test]
    fn test_multiple_unique_assets() {
        let temp_output = NamedTempFile::new().unwrap();
        let image1 = create_test_image("png", 1024);
        let image2 = create_test_image("jpg", 2048);
        let mut builder = BbfBuilder::with_defaults(temp_output.path()).unwrap();

        builder.add_page(image1.path(), 0, 0).unwrap();
        builder.add_page(image2.path(), 0, 0).unwrap();

        assert!(builder.page_count() == 2);
        assert!(builder.asset_count() == 2);
    }

    #[test]
    fn test_string_pooling() {
        let temp_output = NamedTempFile::new().unwrap();
        let mut builder = BbfBuilder::with_defaults(temp_output.path()).unwrap();

        let offset1 = builder.get_or_add_string("Chapter 1");
        let offset2 = builder.get_or_add_string("Chapter 1");
        let offset3 = builder.get_or_add_string("Chapter 2");

        assert!(offset1 == offset2);
        assert!(offset1 != offset3);
    }

    #[test]
    fn test_add_section() {
        let temp_output = NamedTempFile::new().unwrap();
        let mut builder = BbfBuilder::with_defaults(temp_output.path()).unwrap();

        builder.add_section("Chapter 1", 0, None);
        assert!(builder.section_count() == 1);

        builder.add_section("Chapter 2", 10, Some("Part 1"));
        assert!(builder.section_count() == 2);
    }

    #[test]
    fn test_add_metadata() {
        let temp_output = NamedTempFile::new().unwrap();
        let mut builder = BbfBuilder::with_defaults(temp_output.path()).unwrap();

        builder.add_metadata("author", "Test Author", None);
        assert!(builder.metadata_count() == 1);

        builder.add_metadata("title", "Test Book", None);
        assert!(builder.metadata_count() == 2);
    }

    #[test]
    fn test_media_type_from_extension() {
        assert!(MediaType::from_extension("png") == MediaType::Png);
        assert!(MediaType::from_extension(".png") == MediaType::Png);
        assert!(MediaType::from_extension("PNG") == MediaType::Png);
        assert!(MediaType::from_extension("jpg") == MediaType::Jpg);
        assert!(MediaType::from_extension("jpeg") == MediaType::Jpg);
        assert!(MediaType::from_extension("avif") == MediaType::Avif);
        assert!(MediaType::from_extension("webp") == MediaType::Webp);
        assert!(MediaType::from_extension("unknown") == MediaType::Unknown);
    }

    #[test]
    fn test_finalize_creates_valid_file() {
        let temp_output = NamedTempFile::new().unwrap();
        let test_image = create_test_image("png", 1024);
        let mut builder = BbfBuilder::with_defaults(temp_output.path()).unwrap();

        builder.add_page(test_image.path(), 0, 0).unwrap();
        builder.add_section("Chapter 1", 0, None);
        builder.add_metadata("title", "Test Book", None);
        builder.finalize().unwrap();

        let metadata = std::fs::metadata(temp_output.path()).unwrap();
        assert!(metadata.len() > 0);
    }

    #[test]
    fn test_variable_ream_size_alignment() {
        let temp_output = NamedTempFile::new().unwrap();
        let small_image = create_test_image("png", 100);
        let large_image = create_test_image("jpg", 100000);

        let mut builder =
            BbfBuilder::new(temp_output.path(), 12, 10, BBF_VARIABLE_REAM_SIZE_FLAG).unwrap();

        builder.add_page(small_image.path(), 0, 0).unwrap();
        builder.add_page(large_image.path(), 0, 0).unwrap();

        assert!(builder.page_count() == 2);
        assert!(builder.asset_count() == 2);
    }

    #[test]
    fn test_finalize_empty_book() {
        let temp_output = NamedTempFile::new().unwrap();
        let builder = BbfBuilder::with_defaults(temp_output.path()).unwrap();
        builder.finalize().unwrap();

        let reader = crate::BbfReader::open(temp_output.path()).unwrap();
        assert!(reader.page_count() == 0);
        assert!(reader.asset_count() == 0);
    }

    #[test]
    fn test_builder_boundary_alignment_values() {
        let temp0 = NamedTempFile::new().unwrap();
        let result0 = BbfBuilder::new(temp0.path(), 0, 0, 0);
        assert!(result0.is_ok());

        let temp16 = NamedTempFile::new().unwrap();
        let result16 = BbfBuilder::new(temp16.path(), 16, 16, 0);
        assert!(result16.is_ok());
    }

    #[test]
    fn test_string_pool_null_termination() {
        let temp_output = NamedTempFile::new().unwrap();
        let test_image = create_test_image("png", 512);
        let mut builder = BbfBuilder::with_defaults(temp_output.path()).unwrap();

        builder.add_page(test_image.path(), 0, 0).unwrap();
        builder.add_section("Alpha", 0, None);
        builder.add_section("Beta", 0, None);
        builder.add_metadata("key1", "val1", None);
        builder.finalize().unwrap();

        let reader = crate::BbfReader::open(temp_output.path()).unwrap();
        let sections = reader.sections().unwrap();
        let s0 = reader.get_string(sections[0].section_title_offset).unwrap();
        let s1 = reader.get_string(sections[1].section_title_offset).unwrap();
        assert!(s0 == "Alpha");
        assert!(s1 == "Beta");

        let meta = reader.metadata().unwrap();
        let k = reader.get_string(meta[0].key_offset).unwrap();
        let v = reader.get_string(meta[0].value_offset).unwrap();
        assert!(k == "key1");
        assert!(v == "val1");
    }

    #[test]
    fn test_petrification_flag_preserved() {
        let temp_output = NamedTempFile::new().unwrap();
        let test_image = create_test_image("png", 256);
        let mut builder = BbfBuilder::new(
            temp_output.path(),
            12,
            16,
            BBF_PETRIFICATION_FLAG,
        )
        .unwrap();

        builder.add_page(test_image.path(), 0, 0).unwrap();
        builder.finalize().unwrap();

        let reader = crate::BbfReader::open(temp_output.path()).unwrap();
        let header = reader.header();
        assert!(header.flags & BBF_PETRIFICATION_FLAG != 0);
    }
}
