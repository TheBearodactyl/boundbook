use {
    crate::{
        AssetEntry, BbfBuilder, BbfError, BbfFooter, BbfHeader, MAGIC, MAX_BALE_SIZE,
        MAX_FORME_SIZE, Metadata, PageEntry, Result, Section,
    },
    rayon::iter::{IntoParallelRefIterator, ParallelIterator},
    std::{fs::File, path::Path},
};

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

        let header: BbfHeader = Self::read_struct(&mmap, 0)?;

        if &header.magic != MAGIC {
            return Err(BbfError::InvalidMagic);
        }

        if header.footer_offset as usize + std::mem::size_of::<BbfFooter>() > mmap.len() {
            return Err(BbfError::InvalidOffset(
                "Footer offset out of bounds".to_string(),
            ));
        }

        let footer: BbfFooter = Self::read_struct(&mmap, header.footer_offset as usize)?;

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

    fn read_struct<T: Copy>(mmap: &memmap2::Mmap, offset: usize) -> Result<T> {
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
        let end = pool_start + pool_size;

        if start >= self.mmap.len() || end > self.mmap.len() {
            return Err(BbfError::InvalidOffset(
                "String pool out of bounds".to_string(),
            ));
        }

        let data = &self.mmap[start..end];
        let scan_limit = MAX_FORME_SIZE.min(data.len() as u64) as usize;
        let str_end = data[..scan_limit]
            .iter()
            .position(|&b| b == 0)
            .ok_or_else(|| BbfError::InvalidUtf8)?;

        std::str::from_utf8(&data[..str_end]).map_err(|_| BbfError::InvalidUtf8)
    }

    pub fn assets(&self) -> Result<&[AssetEntry]> {
        let offset = self.footer.asset_offset as usize;
        let count = self.footer.asset_count as usize;
        let size = count * std::mem::size_of::<AssetEntry>();

        if offset + size > self.mmap.len() {
            return Err(BbfError::InvalidOffset("Asset table out of bounds".into()));
        }

        unsafe {
            Ok(std::slice::from_raw_parts(
                self.mmap.as_ptr().add(offset) as *const AssetEntry,
                count,
            ))
        }
    }

    pub fn pages(&self) -> Result<&[PageEntry]> {
        let offset = self.footer.page_offset as usize;
        let count = self.footer.page_count as usize;
        let size = count * std::mem::size_of::<PageEntry>();

        if offset + size > self.mmap.len() {
            return Err(BbfError::InvalidOffset("Page table out of bounds".into()));
        }

        unsafe {
            Ok(std::slice::from_raw_parts(
                self.mmap.as_ptr().add(offset) as *const PageEntry,
                count,
            ))
        }
    }

    pub fn sections(&self) -> Result<&[Section]> {
        let offset = self.footer.section_offset as usize;
        let count = self.footer.section_count as usize;
        let size = count * std::mem::size_of::<Section>();

        if offset + size > self.mmap.len() {
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

    pub fn metadata(&self) -> Result<&[Metadata]> {
        let offset = self.footer.meta_offset as usize;
        let count = self.footer.meta_count as usize;
        let size = count * std::mem::size_of::<Metadata>();

        if offset + size > self.mmap.len() {
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

    pub fn get_asset_data(&self, asset: &AssetEntry) -> Result<&[u8]> {
        let start = asset.file_offset as usize;
        let end = start + asset.file_size as usize;

        if end > self.mmap.len() {
            return Err(BbfError::InvalidOffset("Asset data out of bounds".into()));
        }

        Ok(&self.mmap[start..end])
    }

    pub fn version(&self) -> u16 {
        self.header.version
    }

    pub fn page_count(&self) -> u64 {
        self.footer.page_count
    }

    pub fn asset_count(&self) -> u64 {
        self.footer.asset_count
    }

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

    pub fn header(&self) -> &BbfHeader {
        &self.header
    }

    pub fn footer(&self) -> &BbfFooter {
        &self.footer
    }
}
