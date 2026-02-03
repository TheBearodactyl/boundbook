use {
    crate::{
        AssetEntry, BbfBuilder, BbfError, BbfFooter, BbfHeader, MAGIC, Metadata, PageEntry, Result,
        Section,
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

        std::str::from_utf8(&data[..end]).map_err(|_| BbfError::InvalidUtf8)
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
