use {
    crate::{
        ALIGNMENT, AssetEntry, BbfFooter, BbfHeader, MAGIC, MediaType, Metadata, PageEntry, Result,
        Section,
    },
    std::{
        collections::HashMap,
        fs::File,
        hash::Hasher,
        io::{BufWriter, Write},
        path::Path,
    },
    twox_hash::XxHash3_64,
};

pub struct BbfBuilder {
    writer: BufWriter<File>,
    current_offset: u64,
    assets: Vec<AssetEntry>,
    pages: Vec<PageEntry>,
    sections: Vec<Section>,
    metadata: Vec<Metadata>,
    string_pool: Vec<u8>,
    dedupe_map: HashMap<u64, u32>,
    string_map: HashMap<String, u32>,
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
        })
    }

    fn write_struct<T>(writer: &mut BufWriter<File>, data: &T) -> Result<()> {
        unsafe {
            let bytes =
                std::slice::from_raw_parts(data as *const T as *const u8, std::mem::size_of::<T>());
            writer.write_all(bytes)?;
        }
        Ok(())
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

    pub fn calculate_hash(data: &[u8]) -> u64 {
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

    pub fn add_metadata(&mut self, key: &str, val: &str) -> Result<()> {
        let key_offset = self.get_or_add_string(key);
        let val_offset = self.get_or_add_string(val);
        self.metadata.push(Metadata {
            key_offset,
            val_offset,
        });
        Ok(())
    }

    pub fn finalize(mut self) -> Result<()> {
        let mut hasher = XxHash3_64::with_seed(0);
        let string_pool = self.string_pool.clone();

        self.current_offset = self.write_and_hash(&string_pool, &mut hasher)?;

        let mut footer = BbfFooter {
            string_pool_offset: self.current_offset - string_pool.len() as u64,
            asset_table_offset: self.current_offset,
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

        let assets_bytes = unsafe {
            std::slice::from_raw_parts(
                self.assets.as_ptr() as *const u8,
                self.assets.len() * std::mem::size_of::<AssetEntry>(),
            )
        };
        self.current_offset = self.write_and_hash(assets_bytes, &mut hasher)?;
        footer.page_table_offset = self.current_offset;

        let pages_bytes = unsafe {
            std::slice::from_raw_parts(
                self.pages.as_ptr() as *const u8,
                self.pages.len() * std::mem::size_of::<PageEntry>(),
            )
        };
        self.current_offset = self.write_and_hash(pages_bytes, &mut hasher)?;
        footer.section_table_offset = self.current_offset;

        let sections_bytes = unsafe {
            std::slice::from_raw_parts(
                self.sections.as_ptr() as *const u8,
                self.sections.len() * std::mem::size_of::<Section>(),
            )
        };
        self.current_offset = self.write_and_hash(sections_bytes, &mut hasher)?;
        footer.meta_table_offset = self.current_offset;

        let metadata_bytes = unsafe {
            std::slice::from_raw_parts(
                self.metadata.as_ptr() as *const u8,
                self.metadata.len() * std::mem::size_of::<Metadata>(),
            )
        };
        self.write_and_hash(metadata_bytes, &mut hasher)?;

        footer.index_hash = hasher.finish();
        Self::write_struct(&mut self.writer, &footer)?;
        self.writer.flush()?;
        Ok(())
    }

    fn write_and_hash(&mut self, data: &[u8], hasher: &mut XxHash3_64) -> Result<u64> {
        if !data.is_empty() {
            self.writer.write_all(data)?;
            hasher.write(data);
            Ok(self.current_offset + data.len() as u64)
        } else {
            Ok(self.current_offset)
        }
    }
}
