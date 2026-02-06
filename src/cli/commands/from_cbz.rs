use {
    boundbook::{BbfBuilder, Result, types::MediaType},
    clap::Args,
    miette::{Context, IntoDiagnostic},
    std::{
        fs,
        io::Read,
        path::{Path, PathBuf},
    },
    zip::ZipArchive,
};

#[derive(Args)]
#[command(disable_help_flag = true, author = "The Motherfucking Bearodactyl")]
pub struct FromCbzArgs {
    /// Input CBZ file
    input: PathBuf,

    /// Output BBF file
    #[arg(short, long)]
    output: PathBuf,

    /// Add metadata (format: Key:Value[:Parent])
    #[arg(long = "meta")]
    metadata: Vec<String>,

    /// Keep temporary files for debugging
    #[arg(long)]
    keep_temp: bool,
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

/// # Panics
///
/// panics if it fails to get the time since [`std::time::UNIX_EPOCH`]
#[macroni_n_cheese::mathinator2000]
pub fn execute(args: FromCbzArgs) -> Result<()> {
    println!("Converting CBZ to BBF: {}", args.input.display());

    let file = fs::File::open(&args.input)
        .into_diagnostic()
        .with_context(|| format!("Failed to open CBZ file: {}", args.input.display()))?;

    let mut archive = ZipArchive::new(file)
        .into_diagnostic()
        .context("Failed to read CBZ archive - file may be corrupted")?;

    let mut entries = collect_image_entries(&mut archive).into_diagnostic()?;
    entries.sort_by(|a, b| a.1.cmp(&b.1));

    println!("Found {} image pages", entries.len());

    let temp_dir = std::env::temp_dir().join(format!(
        "cbz_convert_{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    ));

    fs::create_dir_all(&temp_dir)
        .into_diagnostic()
        .with_context(|| format!("Failed to create temp directory: {}", temp_dir.display()))?;

    let temp_files = extract_to_temp(&mut archive, &entries, &temp_dir).into_diagnostic()?;

    drop(archive);

    let mut builder = BbfBuilder::with_defaults(&args.output)
        .into_diagnostic()
        .context("Failed to create BBF builder")?;

    for meta_str in &args.metadata {
        if let Some((key, value, parent)) = parse_metadata(meta_str) {
            builder.add_metadata(&key, &value, parent.as_deref());
        }
    }

    if let Some(filename) = args.input.file_name().and_then(|n| n.to_str()) {
        builder.add_metadata("Source", filename, None);
    }
    builder.add_metadata("Converted-From", "CBZ", None);

    for (i, (path, _media_type)) in temp_files.iter().enumerate() {
        builder
            .add_page(path, 0, 0)
            .into_diagnostic()
            .with_context(|| format!("Failed to add page {}", i.saturating_add(1)))?;

        #[allow(unused_parens)]
        if (i + 1) % 10 == 0 {
            println!(
                "  Processed {}/{} pages",
                i.saturating_add(1),
                temp_files.len()
            );
        }
    }

    builder.finalize().into_diagnostic()?;

    if !args.keep_temp {
        fs::remove_dir_all(&temp_dir).ok();
    } else {
        println!("Temporary files kept at: {}", temp_dir.display());
    }

    println!("Successfully converted to {}", args.output.display());

    Ok(())
}
