use {
    crate::{
        AssetEntry, BBF_VARIABLE_REAM_SIZE_FLAG, BbfFooter, BbfHeader, DEFAULT_GUARD_ALIGNMENT,
        DEFAULT_SMALL_REAM_THRESHOLD, MAGIC, MediaType, Metadata, PageEntry, Result, Section,
        VERSION,
    },
    hashbrown::HashMap,
    std::{
        fs::File,
        io::{BufWriter, Write},
        path::Path,
    },
    xxhash_rust::xxh3::{Xxh3, xxh3_128},
};

pub struct BbfBuilder {
    writer: BufWriter<File>,
    current_offset: u64,
    assets: Vec<AssetEntry>,
    pages: Vec<PageEntry>,
    sections: Vec<Section>,
    metadata: Vec<Metadata>,
    string_pool: Vec<u8>,
    dedupe_map: HashMap<u128, u64>,
    string_map: HashMap<String, u64>,
    guard_value: u8,
    ream_value: u8,
    header_flags: u32,
}

impl BbfBuilder {
    pub fn new<P: AsRef<Path>>(
        output_path: P,
        alignment: u8,
        ream_size: u8,
        flags: u32,
    ) -> Result<Self> {
        let file = File::create(output_path)?;
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

        Self::write_struct(&mut writer, &header)?;
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

    pub fn with_defaults<P: AsRef<Path>>(output_path: P) -> Result<Self> {
        Self::new(
            output_path,
            DEFAULT_GUARD_ALIGNMENT,
            DEFAULT_SMALL_REAM_THRESHOLD,
            BBF_VARIABLE_REAM_SIZE_FLAG,
        )
    }

    fn write_struct<T>(writer: &mut BufWriter<File>, data: &T) -> Result<()> {
        unsafe {
            let bytes =
                std::slice::from_raw_parts(data as *const T as *const u8, std::mem::size_of::<T>());
            writer.write_all(bytes)?;
        }
        Ok(())
    }

    fn align_padding(&mut self, alignment_bytes: u64) -> Result<()> {
        let remainder = self.current_offset % alignment_bytes;
        if remainder == 0 {
            return Ok(());
        }

        let padding = alignment_bytes - remainder;
        let zeros = vec![0u8; padding as usize];
        self.writer.write_all(&zeros)?;
        self.current_offset += padding;
        Ok(())
    }

    pub fn calculate_hash_128(data: &[u8]) -> u128 {
        xxh3_128(data)
    }

    pub fn calculate_hash_64(data: &[u8]) -> u64 {
        let mut hasher = Xxh3::new();
        hasher.update(data);
        hasher.digest()
    }

    pub fn add_page<P: AsRef<Path>>(
        &mut self,
        image_path: P,
        page_flags: u32,
        asset_flags: u32,
    ) -> Result<()> {
        let data = std::fs::read(image_path.as_ref())?;
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

            self.align_padding(actual_alignment)?;

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

            self.writer.write_all(&data)?;
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

    pub fn add_section(
        &mut self,
        title: &str,
        start_index: u64,
        parent: Option<&str>,
    ) -> Result<()> {
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
        Ok(())
    }

    pub fn add_metadata(&mut self, key: &str, val: &str, parent: Option<&str>) -> Result<()> {
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
        Ok(())
    }

    pub fn finalize(mut self) -> Result<()> {
        let mut hasher = Xxh3::new();

        let assets_bytes = unsafe {
            std::slice::from_raw_parts(
                self.assets.as_ptr() as *const u8,
                self.assets.len() * std::mem::size_of::<AssetEntry>(),
            )
        };
        let offset_assets = self.current_offset;
        self.writer.write_all(assets_bytes)?;
        hasher.update(assets_bytes);
        self.current_offset += assets_bytes.len() as u64;

        let pages_bytes = unsafe {
            std::slice::from_raw_parts(
                self.pages.as_ptr() as *const u8,
                self.pages.len() * std::mem::size_of::<PageEntry>(),
            )
        };
        let offset_pages = self.current_offset;
        self.writer.write_all(pages_bytes)?;
        hasher.update(pages_bytes);
        self.current_offset += pages_bytes.len() as u64;

        let sections_bytes = unsafe {
            std::slice::from_raw_parts(
                self.sections.as_ptr() as *const u8,
                self.sections.len() * std::mem::size_of::<Section>(),
            )
        };
        let offset_sections = self.current_offset;
        self.writer.write_all(sections_bytes)?;
        hasher.update(sections_bytes);
        self.current_offset += sections_bytes.len() as u64;

        let metadata_bytes = unsafe {
            std::slice::from_raw_parts(
                self.metadata.as_ptr() as *const u8,
                self.metadata.len() * std::mem::size_of::<Metadata>(),
            )
        };
        let offset_meta = self.current_offset;
        self.writer.write_all(metadata_bytes)?;
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

        Self::write_struct(&mut self.writer, &footer)?;

        self.writer.flush()?;
        self.writer.get_mut().sync_all()?;

        let mut file = self.writer.into_inner()?;
        use std::io::Seek;
        file.seek(std::io::SeekFrom::Start(0))?;

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
            file.write_all(bytes)?;
        }

        file.sync_all()?;
        Ok(())
    }

    pub fn asset_count(&self) -> usize {
        self.assets.len()
    }

    pub fn page_count(&self) -> usize {
        self.pages.len()
    }

    pub fn section_count(&self) -> usize {
        self.sections.len()
    }

    pub fn metadata_count(&self) -> usize {
        self.metadata.len()
    }
}
