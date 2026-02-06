use {
    boundbook::prelude::*,
    clap::Args,
    hashbrown::HashMap,
    indicatif::{ProgressBar, ProgressStyle},
    miette::{Context, IntoDiagnostic, miette},
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

#[derive(Args)]
#[command(author = "The Motherfucking Bearodactyl")]
pub struct CreateArgs {
    /// Input files or directories containing images
    #[arg(required = true)]
    inputs: Vec<PathBuf>,

    /// Output BBF file path
    #[arg(short = 'o', long)]
    output: PathBuf,

    /// Page order file (format: filename:index)
    #[arg(short = 'O', long)]
    order: Option<PathBuf>,

    /// Sections file (format: Name:Target[:Parent])
    #[arg(short = 'S', long)]
    sections: Option<PathBuf>,

    /// Add section markers (format: Name:Target[:Parent])
    #[arg(short = 's', long = "section")]
    add_sections: Vec<String>,

    /// Add metadata (format: Key:Value[:Parent])
    #[arg(short = 'm', long = "meta")]
    metadata: Vec<String>,

    /// Byte alignment exponent (default: 12 = 4096 bytes)
    #[arg(short = 'a', long, default_value_t = DEFAULT_GUARD_ALIGNMENT)]
    alignment: u8,

    /// Ream size exponent (default: 16 = 65536 bytes)
    #[arg(short = 'r', long, default_value_t = DEFAULT_SMALL_REAM_THRESHOLD)]
    ream_size: u8,

    /// Enable variable ream size for smaller files
    #[arg(short = 'v', long)]
    variable_ream_size: bool,

    /// Auto-detect subdirectories with images and create sections from directory names
    #[arg(short = 'd', long)]
    auto_detect_sections: bool,
}

#[derive(Debug, Clone)]
struct PagePlan {
    path: PathBuf,
    filename: String,
    order: i32,
    section: Option<String>,
}

impl PagePlan {
    fn new(path: PathBuf, section: Option<String>) -> Self {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        Self {
            path,
            filename,
            order: 0,
            section,
        }
    }
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
    parent: Option<String>,
}

fn compare_pages(a: &PagePlan, b: &PagePlan) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    match (&a.section, &b.section) {
        (Some(a_sec), Some(b_sec)) if a_sec != b_sec => a_sec.cmp(b_sec),
        _ => match (a.order, b.order) {
            (a_ord, b_ord) if a_ord > 0 && b_ord > 0 => a_ord.cmp(&b_ord),
            (a_ord, _) if a_ord > 0 => Ordering::Less,
            (_, b_ord) if b_ord > 0 => Ordering::Greater,
            _ => a.filename.cmp(&b.filename),
        },
    }
}

#[macroni_n_cheese::mathinator2000]
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
    let parts: Vec<&str> = s.splitn(3, ':').collect();
    if parts.len() < 2 {
        return None;
    }

    let parent = if parts.len() >= 3 {
        Some(trim_quotes(parts[2]))
    } else {
        None
    };

    Some(MetadataRequest {
        key: trim_quotes(parts[0]),
        value: trim_quotes(parts[1]),
        parent,
    })
}

#[macroni_n_cheese::mathinator2000]
fn load_order_file(path: &PathBuf) -> Result<HashMap<String, i32>> {
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

fn load_sections_file(path: &PathBuf) -> Result<Vec<SectionRequest>> {
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

/// Check if a directory contains only image files (and possibly subdirectories)
fn directory_contains_images(dir: &Path) -> Result<bool> {
    for entry in fs::read_dir(dir)
        .into_diagnostic()
        .with_context(|| format!("Failed to read directory: {}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && is_image_file(&path) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Collect all subdirectories that contain images
fn collect_image_directories(parent: &Path) -> Result<Vec<PathBuf>> {
    let mut image_dirs = Vec::new();

    for entry in fs::read_dir(parent)
        .into_diagnostic()
        .with_context(|| format!("Failed to read directory: {}", parent.display()))?
    {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() && directory_contains_images(&path)? {
            image_dirs.push(path);
        }
    }

    Ok(image_dirs)
}

fn collect_image_files(
    inputs: &[PathBuf],
    order_map: &HashMap<String, i32>,
    auto_detect: bool,
) -> Result<Vec<PagePlan>> {
    let mut manifest = Vec::new();

    for input in inputs {
        if input.is_dir() {
            if auto_detect {
                let image_dirs = collect_image_directories(input)?;

                if !image_dirs.is_empty() {
                    for subdir in image_dirs {
                        let section_name = subdir
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Unknown")
                            .to_string();

                        for entry in fs::read_dir(&subdir).into_diagnostic().with_context(|| {
                            format!("Failed to read directory: {}", subdir.display())
                        })? {
                            let entry = entry?;
                            let path = entry.path();

                            if path.is_file() && is_image_file(&path) {
                                let mut plan = PagePlan::new(path, Some(section_name.clone()));
                                if let Some(&order) = order_map.get(&plan.filename) {
                                    plan.order = order;
                                }
                                manifest.push(plan);
                            }
                        }
                    }
                    continue;
                }
            }

            for entry in fs::read_dir(input)
                .into_diagnostic()
                .with_context(|| format!("Failed to read directory: {}", input.display()))?
            {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() && is_image_file(&path) {
                    let mut plan = PagePlan::new(path, None);
                    if let Some(&order) = order_map.get(&plan.filename) {
                        plan.order = order;
                    }
                    manifest.push(plan);
                }
            }
        } else if input.is_file() && is_image_file(input) {
            let mut plan = PagePlan::new(input.clone(), None);
            if let Some(&order) = order_map.get(&plan.filename) {
                plan.order = order;
            }
            manifest.push(plan);
        }
    }

    Ok(manifest)
}

fn is_image_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        !matches!(MediaType::from_extension(ext), MediaType::Unknown)
    } else {
        false
    }
}

fn resolve_section_target(
    section: &SectionRequest,
    file_to_page: &HashMap<String, u64>,
) -> Result<u64> {
    if section.is_filename {
        file_to_page
            .get(&section.target)
            .copied()
            .ok_or_else(|| {
                miette!(
                    "Section target file '{}' not found in pages",
                    section.target
                )
            })
            .map_err(|e| e.into())
    } else {
        section
            .target
            .parse::<u64>()
            .into_diagnostic()
            .context("Invalid page number")?
            .checked_sub(1)
            .ok_or_else(|| miette!("Page number must be at least 1"))
            .map_err(|e| e.into())
    }
}

pub fn execute(args: CreateArgs) -> Result<()> {
    let order_map = if let Some(path) = &args.order {
        load_order_file(path)?
    } else {
        HashMap::new()
    };

    let mut sections_from_file = if let Some(path) = &args.sections {
        load_sections_file(path)?
    } else {
        Vec::new()
    };

    sections_from_file.extend(
        args.add_sections
            .iter()
            .filter_map(|s| parse_section_string(s)),
    );

    let metadata: Vec<_> = args
        .metadata
        .iter()
        .filter_map(|m| parse_metadata_string(m))
        .collect();

    let mut manifest = collect_image_files(&args.inputs, &order_map, args.auto_detect_sections)?;
    manifest.sort_by(compare_pages);

    let flags = if args.variable_ream_size {
        BBF_VARIABLE_REAM_SIZE_FLAG
    } else {
        0
    };

    let mut builder = BbfBuilder::new(&args.output, args.alignment, args.ream_size, flags)
        .into_diagnostic()
        .context("Failed to create BBF builder")?;

    let pb = ProgressBar::new(manifest.len() as u64)
        .with_message("Adding pages")
        .with_style(
            ProgressStyle::default_bar()
                .template("[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}")
                .map_err(BbfError::from)
                .into_diagnostic()?
                .progress_chars("##-"),
        );

    let mut file_to_page: HashMap<String, u64> = HashMap::new();
    let mut section_first_pages: HashMap<String, u64> = HashMap::new();

    for (i, page) in manifest.iter().enumerate() {
        builder
            .add_page(&page.path, 0, 0)
            .into_diagnostic()
            .with_context(|| format!("Failed to add page: {}", page.path.display()))?;
        pb.inc(1);
        file_to_page.insert(page.filename.clone(), i as u64);

        if let Some(section_name) = &page.section {
            section_first_pages
                .entry(section_name.clone())
                .or_insert(i as u64);
        }
    }

    pb.finish_with_message("Added all pages!");

    for (section_name, first_page_idx) in section_first_pages {
        builder.add_section(&section_name, first_page_idx, None);
    }

    let mut section_name_to_idx: HashMap<String, u64> = HashMap::new();
    for (i, section_req) in sections_from_file.iter().enumerate() {
        let page_index = resolve_section_target(section_req, &file_to_page)?;

        let parent_name = section_req.parent.as_deref();
        builder.add_section(&section_req.name, page_index, parent_name);
        section_name_to_idx.insert(section_req.name.clone(), i as u64);
    }

    for meta in metadata {
        builder.add_metadata(&meta.key, &meta.value, meta.parent.as_deref());
    }

    builder.finalize()?;

    println!(
        "Successfully created {} ({} pages)",
        args.output.display(),
        manifest.len()
    );

    Ok(())
}
