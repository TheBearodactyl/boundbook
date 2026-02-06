use {
    boundbook::{BbfBuilder, Result, types::MediaType},
    clap::Args,
    miette::{Context, IntoDiagnostic, miette},
    std::{
        fs,
        io::Read,
        path::{Path, PathBuf},
    },
    zip::ZipArchive,
};

#[derive(Args)]
#[command(author = "The Motherfucking Bearodactyl")]
pub struct FromCbzArgs {
    /// Input CBZ file or directory containing CBZ files
    input: PathBuf,

    /// Output BBF file
    #[arg(short = 'o', long)]
    output: PathBuf,

    /// Add metadata (format: Key:Value[:Parent])
    #[arg(short = 'm', long = "meta")]
    metadata: Vec<String>,

    /// Keep temporary files for debugging
    #[arg(short = 'k', long)]
    keep_temp: bool,

    /// Process directory of CBZ files as chapters
    #[arg(short = 'd', long)]
    directory_mode: bool,
}

#[derive(Debug)]
struct ChapterInfo {
    name: String,
    pages: Vec<(PathBuf, MediaType)>,
}

fn collect_image_entries(
    archive: &mut ZipArchive<fs::File>,
) -> Result<Vec<(usize, String, MediaType)>> {
    let mut entries = Vec::new();

    for i in 0..archive.len() {
        let file = archive
            .by_index(i)
            .into_diagnostic()
            .with_context(|| format!("Failed to read archive entry {}", i))?;

        let name = file.name().to_string();

        if file.is_dir()
            || name.starts_with('.')
            || name.starts_with("__MACOSX")
            || name.contains("/.")
        {
            continue;
        }

        let ext = std::path::Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let media_type = MediaType::from_extension(ext);

        if !matches!(media_type, MediaType::Unknown) {
            entries.push((i, name, media_type));
        }
    }

    Ok(entries)
}

fn extract_to_temp(
    archive: &mut ZipArchive<fs::File>,
    entries: &[(usize, String, MediaType)],
    temp_dir: &Path,
) -> Result<Vec<(PathBuf, MediaType)>> {
    let mut temp_files = Vec::new();

    for (idx, _, media_type) in entries {
        let mut file = archive
            .by_index(*idx)
            .into_diagnostic()
            .with_context(|| format!("Failed to read entry at index {}", idx))?;

        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)
            .into_diagnostic()
            .context("Failed to read archive entry data")?;

        let filename = format!("page_{:04}{}", temp_files.len(), media_type.as_extension());
        let temp_path = temp_dir.join(&filename);

        fs::write(&temp_path, buffer)
            .into_diagnostic()
            .with_context(|| format!("Failed to write temp file: {}", temp_path.display()))?;

        temp_files.push((temp_path, *media_type));
    }

    Ok(temp_files)
}

fn parse_metadata(s: &str) -> Option<(String, String, Option<String>)> {
    let parts: Vec<&str> = s.splitn(3, ':').collect();
    if parts.len() >= 2 {
        let parent = if parts.len() >= 3 {
            Some(parts[2].trim().to_string())
        } else {
            None
        };
        Some((
            parts[0].trim().to_string(),
            parts[1].trim().to_string(),
            parent,
        ))
    } else {
        None
    }
}

fn is_cbz_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        matches!(ext.to_lowercase().as_str(), "cbz" | "zip")
    } else {
        false
    }
}

fn collect_cbz_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut cbz_files = Vec::new();

    for entry in fs::read_dir(dir)
        .into_diagnostic()
        .with_context(|| format!("Failed to read directory: {}", dir.display()))?
    {
        let entry = entry.into_diagnostic()?;
        let path = entry.path();

        if path.is_file() && is_cbz_file(&path) {
            cbz_files.push(path);
        }
    }

    alphanumeric_sort::sort_path_slice(&mut cbz_files);

    Ok(cbz_files)
}

#[macroni_n_cheese::mathinator2000]
fn process_cbz_to_chapter(
    cbz_path: &Path,
    base_temp_dir: &Path,
    chapter_index: usize,
) -> Result<ChapterInfo> {
    let next_chapter = chapter_index + 1;
    let chapter_name = cbz_path
        .file_stem()
        .and_then(|n| n.to_str())
        .unwrap_or(&format!("Chapter {}", next_chapter))
        .to_string();

    println!("Processing: {} ...", chapter_name);

    let file = fs::File::open(cbz_path)
        .into_diagnostic()
        .with_context(|| format!("Failed to open CBZ file: {}", cbz_path.display()))?;

    let mut archive = ZipArchive::new(file)
        .into_diagnostic()
        .context("Failed to read CBZ archive - file may be corrupted")?;

    let mut entries = collect_image_entries(&mut archive).into_diagnostic()?;
    entries.sort_by(|a, b| a.1.cmp(&b.1));

    println!("  Found {} image pages", entries.len());

    let temp_dir = base_temp_dir.join(format!("chapter_{:03}", chapter_index));
    fs::create_dir_all(&temp_dir)
        .into_diagnostic()
        .with_context(|| format!("Failed to create temp directory: {}", temp_dir.display()))?;

    let pages = extract_to_temp(&mut archive, &entries, &temp_dir).into_diagnostic()?;

    Ok(ChapterInfo {
        name: chapter_name,
        pages,
    })
}

fn process_directory_of_cbz(input_dir: &Path, base_temp_dir: &Path) -> Result<Vec<ChapterInfo>> {
    let cbz_files = collect_cbz_files(input_dir)?;

    if cbz_files.is_empty() {
        return Err(miette!("No CBZ files found in directory: {}", input_dir.display()).into());
    }

    println!("Found {} CBZ files to process", cbz_files.len());
    println!();

    let mut chapters = Vec::new();

    for (index, cbz_path) in cbz_files.iter().enumerate() {
        let chapter = process_cbz_to_chapter(cbz_path, base_temp_dir, index)?;
        chapters.push(chapter);
    }

    Ok(chapters)
}

/// # Panics
///
/// panics if it fails to get the time since [`std::time::UNIX_EPOCH`]
#[macroni_n_cheese::mathinator2000]
pub fn execute(args: FromCbzArgs) -> Result<()> {
    let temp_dir_name = format!(
        "cbz_convert_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    let base_temp_dir = std::env::temp_dir().join(temp_dir_name);

    fs::create_dir_all(&base_temp_dir)
        .into_diagnostic()
        .with_context(|| {
            format!(
                "Failed to create temp directory: {}",
                base_temp_dir.display()
            )
        })?;

    let chapters = if args.input.is_dir() || args.directory_mode {
        if !args.input.is_dir() {
            return Err(miette!("Input is not a directory: {}", args.input.display()).into());
        }
        println!(
            "Converting directory of CBZ files to BBF: {}",
            args.input.display()
        );
        println!();
        process_directory_of_cbz(&args.input, &base_temp_dir)?
    } else {
        println!("Converting CBZ to BBF: {}", args.input.display());
        vec![process_cbz_to_chapter(&args.input, &base_temp_dir, 0)?]
    };

    let mut builder = BbfBuilder::with_defaults(&args.output)
        .into_diagnostic()
        .context("Failed to create BBF builder")?;

    for meta_str in &args.metadata {
        if let Some((key, value, parent)) = parse_metadata(meta_str) {
            builder.add_metadata(&key, &value, parent.as_deref());
        }
    }

    if chapters.len() == 1 {
        if let Some(filename) = args.input.file_name().and_then(|n| n.to_str()) {
            builder.add_metadata("Source", filename, None);
        }
    } else {
        if let Some(dirname) = args.input.file_name().and_then(|n| n.to_str()) {
            builder.add_metadata("Source", dirname, None);
        }

        builder.add_metadata("Chapters", &chapters.len().to_string(), None);
    }

    builder.add_metadata("Converted-From", "CBZ", None);

    println!();
    println!("Building BBF file...");

    let mut total_pages: u64 = 0;
    let mut section_pages: Vec<(String, u64)> = Vec::new();

    for (chapter_idx, chapter) in chapters.iter().enumerate() {
        let first_page_of_chapter = total_pages;

        let next_chapter = chapter_idx + 1;
        println!(
            "  Adding Chapter {}/{}: {} ({} pages)",
            next_chapter,
            chapters.len(),
            chapter.name,
            chapter.pages.len()
        );

        for (page_idx, (path, _media_type)) in chapter.pages.iter().enumerate() {
            let next_page = page_idx + 1;
            builder
                .add_page(path, 0, 0)
                .into_diagnostic()
                .with_context(|| {
                    format!(
                        "Failed to add page {} from chapter {}",
                        next_page, chapter.name
                    )
                })?;

            total_pages += 1;
        }

        if chapters.len() > 1 {
            section_pages.push((chapter.name.clone(), first_page_of_chapter));
        }
    }

    for (section_name, first_page) in section_pages {
        builder.add_section(&section_name, first_page, None);
    }

    builder.finalize().into_diagnostic()?;

    if !args.keep_temp {
        fs::remove_dir_all(&base_temp_dir).ok();
    } else {
        println!();
        println!("Temporary files kept at: {}", base_temp_dir.display());
    }

    println!();
    println!(
        "Successfully converted to {} ({} pages, {} chapters)",
        args.output.display(),
        total_pages,
        chapters.len()
    );

    Ok(())
}
