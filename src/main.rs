use {
    boundbook::{BbfBuilder, BbfReader, MediaType, Result},
    clap::Parser,
    indicatif::{ProgressBar, ProgressStyle},
    std::{
        cmp::Ordering,
        collections::HashMap,
        fs,
        io::Read,
        path::{Path, PathBuf},
    },
    zip::ZipArchive,
};

mod reader;

#[derive(Parser)]
#[command(name = "boundbook")]
#[command(author = "Developed by EF1500")]
#[command(version = "1.0")]
#[command(about = "BBF CLI", long_about = None)]
struct Cli {
    /// Input files or directories
    inputs: Vec<PathBuf>,

    /// Display book structure/metadata
    #[arg(short, long)]
    info: bool,

    /// Perform XXH3 integrity checks on all assets
    #[arg(short, long)]
    verify: bool,

    /// Optional asset index to verify (used with --verify)
    #[arg(long)]
    verify_index: Option<i32>,

    /// Extract pages from BBF file
    #[arg(long)]
    extract: bool,

    /// Output directory for extraction (default: ./extracted)
    #[arg(long, default_value = "./extracted")]
    outdir: PathBuf,

    /// Extract only a specific section
    #[arg(long)]
    section: Option<String>,

    /// Find the end of extraction by matching this string against the next section title
    #[arg(long)]
    rangekey: Option<String>,

    /// Use a text file to define page order (format: filename:index)
    #[arg(long)]
    order: Option<PathBuf>,

    /// Use a text file to define multiple sections (format: Name:Target[:Parent])
    #[arg(long)]
    sections: Option<PathBuf>,

    /// Add a single section marker (format: Name:Target[:Parent])
    #[arg(long = "add-section", value_name = "SECTION")]
    add_sections: Vec<String>,

    /// Add archival metadata (format: Key:Value)
    #[arg(long, value_name = "META")]
    meta: Vec<String>,

    /// Convert CBZ archive to BBF format
    #[arg(long)]
    from_cbz: bool,

    /// Read a BBF file in the terminal
    #[arg(long)]
    read: bool,

    /// Pre-render all pages before reading (uses more memory)
    #[arg(long)]
    prerender: bool,
}

#[derive(Debug, Clone)]
struct PagePlan {
    path: PathBuf,
    filename: String,
    order: i32,
}

#[derive(Debug, Clone)]
struct SectionRequest {
    name: String,
    target: String,
    parent: Option<String>,
    is_filename: bool,
}

#[derive(Debug, Clone)]
struct MetadataRequest {
    key: String,
    value: String,
}

impl PagePlan {
    fn new(path: PathBuf) -> Self {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        Self {
            path,
            filename,
            order: 0,
        }
    }
}

fn compare_pages(a: &PagePlan, b: &PagePlan) -> Ordering {
    match (a.order, b.order) {
        (a_ord, b_ord) if a_ord > 0 && b_ord > 0 => a_ord.cmp(&b_ord),
        (a_ord, _) if a_ord > 0 => Ordering::Less,
        (_, b_ord) if b_ord > 0 => Ordering::Greater,
        (0, 0) => a.filename.cmp(&b.filename),
        (0, _) => Ordering::Less,
        (_, 0) => Ordering::Greater,
        (a_ord, b_ord) => a_ord.cmp(&b_ord),
    }
}

fn trim_quotes(s: &str) -> String {
    let s = s.trim();

    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

fn parse_section_string(s: &str) -> Option<SectionRequest> {
    let parts: Vec<&str> = s.split(':').collect();

    if parts.len() < 2 {
        return None;
    }

    let name = trim_quotes(parts[0]);
    let target = trim_quotes(parts[1]);
    let parent = if parts.len() >= 3 {
        Some(trim_quotes(parts[2]))
    } else {
        None
    };

    let is_filename = !target.chars().all(|c| c.is_ascii_digit());

    Some(SectionRequest {
        name,
        target,
        parent,
        is_filename,
    })
}

fn parse_metadata_string(s: &str) -> Option<MetadataRequest> {
    let parts: Vec<&str> = s.splitn(2, ':').collect();

    if parts.len() != 2 {
        return None;
    }

    Some(MetadataRequest {
        key: trim_quotes(parts[0]),
        value: trim_quotes(parts[1]),
    })
}

fn load_order_file(path: &Path) -> Result<HashMap<String, i32>> {
    let content = fs::read_to_string(path)?;
    let mut map = HashMap::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(colon_pos) = line.rfind(':') {
            let filename = trim_quotes(&line[..colon_pos]);
            let order_str = &line[colon_pos + 1..];

            if let Ok(order) = order_str.parse::<i32>() {
                map.insert(filename, order);
            }
        } else {
            map.insert(trim_quotes(line), 0);
        }
    }

    Ok(map)
}

fn load_sections_file(path: &Path) -> Result<Vec<SectionRequest>> {
    let content = fs::read_to_string(path)?;
    let mut sections = Vec::new();

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        if let Some(section) = parse_section_string(line) {
            sections.push(section);
        }
    }

    Ok(sections)
}

fn find_section_end(
    reader: &BbfReader,
    current_idx: usize,
    range_key: Option<&str>,
) -> Result<u32> {
    let sections = reader.sections();
    let start_page = sections[current_idx].start_index;

    for section in sections.iter().skip(current_idx + 1) {
        if let Some(key) = range_key {
            let title = reader.get_string(section.title_offset)?;
            if title.contains(key) {
                return Ok(section.start_index);
            }
        } else if section.start_index > start_page {
            return Ok(section.start_index);
        }
    }

    Ok(reader.page_count())
}

fn cmd_info(reader: &BbfReader) -> Result<()> {
    println!("Bound Book Format (.bbf) Info");
    println!("------------------------------");
    println!("BBF Version: {}", reader.version());
    println!("Pages:       {}", reader.page_count());
    println!("Assets:      {} (Deduplicated)", reader.asset_count());

    println!("\n[Sections]");
    if reader.sections().is_empty() {
        println!(" No sections defined.");
    } else {
        for section in reader.sections() {
            let title = reader.get_string(section.title_offset)?;
            println!(
                " - {:<20} (Starting Page: {})",
                title,
                section.start_index + 1
            );
        }
    }

    println!("\n[Metadata]");
    if reader.metadata().is_empty() {
        println!(" No metadata found.");
    } else {
        for meta in reader.metadata() {
            let key = reader.get_string(meta.key_offset)?;
            let value = reader.get_string(meta.val_offset)?;
            println!(" - {:<15} {}", format!("{}:", key), value);
        }
    }

    Ok(())
}

fn cmd_verify(reader: &BbfReader, asset_index: Option<i32>) -> Result<()> {
    if let Some(-1) = asset_index {
        let valid = reader.verify_integrity()?;
        println!("Directory Hash: {}", if valid { "OK" } else { "CORRUPT" });
        return if valid {
            Ok(())
        } else {
            Err(boundbook::BbfError::HashMismatch)
        };
    }

    if let Some(idx) = asset_index
        && idx >= 0
    {
        let valid = reader.verify_asset(idx as usize)?;
        println!("Asset {}: {}", idx, if valid { "OK" } else { "CORRUPT" });
        return if valid {
            Ok(())
        } else {
            Err(boundbook::BbfError::HashMismatch)
        };
    }

    println!("Verifying integrity using XXH3 (Parallel)...");
    let valid = reader.verify_integrity()?;

    if valid {
        println!("All integrity checks passed.");
        Ok(())
    } else {
        eprintln!(" [!!] Integrity check failed");
        Err(boundbook::BbfError::HashMismatch)
    }
}

fn cmd_extract(
    reader: &BbfReader,
    outdir: &Path,
    target_section: Option<&str>,
    range_key: Option<&str>,
) -> Result<()> {
    fs::create_dir_all(outdir)?;

    let pages = reader.pages();
    let assets = reader.assets();
    let sections = reader.sections();

    let (start, end) = if let Some(section_name) = target_section {
        let section_idx = sections
            .iter()
            .position(|s| reader.get_string(s.title_offset).unwrap_or("") == section_name)
            .ok_or_else(|| format!("Section '{}' not found", section_name))?;

        let start = sections[section_idx].start_index as usize;
        let end = find_section_end(reader, section_idx, range_key)? as usize;
        (start, end)
    } else {
        (0, pages.len())
    };

    println!(
        "Extracting: {} (Pages {} to {})",
        target_section.unwrap_or("Full Book"),
        start + 1,
        end
    );

    for (i, item) in pages.iter().enumerate().take(end).skip(start) {
        let page = &item;
        let asset = &assets[page.asset_index as usize];

        let media_type = MediaType::from(asset.media_type);
        let extension = media_type.as_extension();
        let filename = format!("p{:03}{}", i + 1, extension);
        let output_path = outdir.join(filename);

        let data = reader.get_asset_data(asset);
        fs::write(&output_path, data)?;

        println!("  Extracted: {}", output_path.display());
    }

    println!("Done.");
    Ok(())
}

fn cmd_mux(
    inputs: Vec<PathBuf>,
    output: PathBuf,
    order_file: Option<PathBuf>,
    sections_file: Option<PathBuf>,
    add_sections: Vec<SectionRequest>,
    metadata: Vec<MetadataRequest>,
) -> Result<()> {
    let order_map = if let Some(path) = order_file {
        load_order_file(&path)?
    } else {
        HashMap::new()
    };

    let mut sections_from_file = if let Some(path) = sections_file {
        load_sections_file(&path)?
    } else {
        Vec::new()
    };

    sections_from_file.extend(add_sections);

    let mut manifest = Vec::new();
    for input in inputs.clone() {
        if input.is_dir() {
            for entry in fs::read_dir(input)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    let mut plan = PagePlan::new(path);
                    if let Some(&order) = order_map.get(&plan.filename) {
                        plan.order = order;
                    }
                    manifest.push(plan);
                }
            }
        } else if input.is_file() {
            let mut plan = PagePlan::new(input);
            if let Some(&order) = order_map.get(&plan.filename) {
                plan.order = order;
            }
            manifest.push(plan);
        }
    }

    manifest.sort_by(compare_pages);

    let mut builder = BbfBuilder::new(&output)?;
    let pages_pb = ProgressBar::new(manifest.len() as u64)
        .with_message("Adding pages")
        .with_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
                .expect("Failed to build template")
                .progress_chars("##-"),
        );
    let mut file_to_page: HashMap<String, u32> = HashMap::new();

    for (i, page) in manifest.iter().enumerate() {
        let ext = page.path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let media_type = MediaType::from_extension(ext);

        builder.add_page(&page.path, media_type)?;
        pages_pb.inc(1);
        file_to_page.insert(page.filename.clone(), i as u32);
    }

    pages_pb.finish_with_message("Added all pages!");

    let mut section_name_to_idx: HashMap<String, u32> = HashMap::new();
    for (i, section_req) in sections_from_file.iter().enumerate() {
        let page_index = if section_req.is_filename {
            file_to_page
                .get(&section_req.target)
                .copied()
                .unwrap_or_else(|| {
                    eprintln!(
                        "Warning: Section target file '{}' not found. Defaulting to page 1.",
                        section_req.target
                    );
                    0
                })
        } else {
            section_req
                .target
                .parse::<u32>()
                .unwrap_or(1)
                .saturating_sub(1)
        };

        let parent_idx = section_req
            .parent
            .as_ref()
            .and_then(|p| section_name_to_idx.get(p).copied());

        builder.add_section(&section_req.name, page_index, parent_idx)?;
        section_name_to_idx.insert(section_req.name.clone(), i as u32);
    }

    for meta in metadata {
        builder.add_metadata(&meta.key, &meta.value)?;
    }

    builder.finalize()?;

    println!(
        "Successfully created {} ({} pages)",
        output.display(),
        manifest.len()
    );

    Ok(())
}

fn cmd_cbz_to_bbf(cbz_path: &Path, output: &Path, metadata: Vec<MetadataRequest>) -> Result<()> {
    println!("Converting CBZ to BBF: {}", cbz_path.display());

    let file = fs::File::open(cbz_path)?;
    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to open CBZ archive: {}", e))?;

    let mut entries = Vec::new();

    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to read archive entry {}: {}", i, e))?;

        let name = file.name().to_string();

        if file.is_dir() || name.starts_with('.') || name.starts_with("__MACOSX") {
            continue;
        }

        let ext = Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let media_type = MediaType::from_extension(ext);

        if matches!(media_type, MediaType::Unknown) {
            continue;
        }

        entries.push((i, name, media_type));
    }

    entries.sort_by(|a, b| a.1.cmp(&b.1));

    println!("Found {} image pages", entries.len());

    let temp_dir = std::env::temp_dir().join(format!(
        "cbz_convert_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    ));
    fs::create_dir_all(&temp_dir)?;

    let mut temp_files = Vec::new();
    for (idx, _, media_type) in &entries {
        let mut file = archive
            .by_index(*idx)
            .map_err(|e| format!("Failed to read entry: {}", e))?;

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        let filename = format!("page_{:04}{}", temp_files.len(), media_type.as_extension());
        let temp_path = temp_dir.join(&filename);
        fs::write(&temp_path, buffer)?;

        temp_files.push((temp_path, *media_type));
    }

    drop(archive);

    let mut builder = BbfBuilder::new(output)?;

    for meta in metadata {
        builder.add_metadata(&meta.key, &meta.value)?;
    }

    if let Some(filename) = cbz_path.file_name().and_then(|n| n.to_str()) {
        builder.add_metadata("Source", filename)?;
    }

    builder.add_metadata("Converted-From", "CBZ")?;

    for (i, (path, media_type)) in temp_files.iter().enumerate() {
        builder.add_page(path, *media_type)?;
        if (i + 1) % 10 == 0 {
            println!("  Processed {}/{} pages", i + 1, temp_files.len());
        }
    }

    builder.finalize()?;

    fs::remove_dir_all(&temp_dir)?;

    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.read {
        if cli.inputs.is_empty() {
            eprintln!("Error: No .bbf input specified.");
            std::process::exit(1);
        }

        let mut reader = reader::BookReader::new(&cli.inputs[0])?;
        reader.run(cli.prerender)?;
        return Ok(());
    }

    if cli.from_cbz {
        if cli.inputs.len() < 2 {
            eprintln!("Error: Provide CBZ input and BBF output filename.");
            eprintln!("Usage: bbfmux --from-cbz <input.cbz> <output.bbf>");
            std::process::exit(1);
        }

        let cbz_input = &cli.inputs[0];
        let bbf_output = &cli.inputs[1];

        let metadata: Vec<MetadataRequest> = cli
            .meta
            .iter()
            .filter_map(|m| parse_metadata_string(m))
            .collect();

        cmd_cbz_to_bbf(cbz_input, bbf_output, metadata)?;
    } else if cli.info || cli.verify || cli.extract {
        if cli.inputs.is_empty() {
            eprintln!("Error: No .bbf input specified.");
            std::process::exit(1);
        }

        let reader = BbfReader::open(&cli.inputs[0])?;

        if cli.info {
            cmd_info(&reader)?;
        }

        if cli.verify {
            cmd_verify(&reader, cli.verify_index)?;
        }

        if cli.extract {
            cmd_extract(
                &reader,
                &cli.outdir,
                cli.section.as_deref(),
                cli.rangekey.as_deref(),
            )?;
        }
    } else {
        if cli.inputs.len() < 2 {
            eprintln!("Error: Provide inputs and an output filename.");
            std::process::exit(1);
        }

        let mut inputs = cli.inputs;
        let output = inputs.pop().unwrap();

        let sections: Vec<SectionRequest> = cli
            .add_sections
            .iter()
            .filter_map(|s| parse_section_string(s))
            .collect();

        let metadata: Vec<MetadataRequest> = cli
            .meta
            .iter()
            .filter_map(|m| parse_metadata_string(m))
            .collect();

        cmd_mux(inputs, output, cli.order, cli.sections, sections, metadata)?;
    }

    Ok(())
}
