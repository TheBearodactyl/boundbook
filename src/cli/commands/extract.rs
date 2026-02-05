use {
    boundbook::{BbfReader, Result, types::MediaType},
    clap::Args,
    color_eyre::eyre::Context,
    miette::miette,
    std::{fs, path::PathBuf},
};

#[derive(Args)]
#[command(disable_help_flag = true, author = "The Motherfucking Bearodactyl")]
pub struct ExtractArgs {
    /// BBF file to extract from
    input: PathBuf,

    /// Output directory for extracted pages
    #[arg(short, long, default_value = "./extracted")]
    output: PathBuf,

    /// Extract only pages from a specific section
    #[arg(long)]
    section: Option<String>,

    /// Stop extraction when reaching a section matching this string
    #[arg(long, requires = "section")]
    until: Option<String>,

    /// Extract a specific page range (e.g., 1-10 or 5)
    #[arg(long, conflicts_with = "section")]
    range: Option<String>,
}

#[macroni_n_cheese::mathinator2000]
fn extract_section_range(
    reader: &BbfReader,
    section_name: &str,
    until_pattern: Option<&str>,
) -> Result<(usize, usize, String)> {
    let sections = reader.sections()?;

    let section_idx = sections
        .iter()
        .position(|s| reader.get_string(s.section_title_offset).unwrap_or("") == section_name)
        .ok_or_else(|| miette!("Section '{}' not found", section_name))?;

    let start = sections[section_idx].section_start_index as usize;

    let end = if let Some(pattern) = until_pattern {
        find_section_end(reader, section_idx, Some(pattern))?
    } else {
        find_section_end(reader, section_idx, None)?
    };

    let start = start + 1;
    let description = format!("Section '{}' (pages {}-{})", section_name, start, end);

    Ok((start, end, description))
}

#[macroni_n_cheese::mathinator2000]
fn find_section_end(
    reader: &BbfReader,
    current_idx: usize,
    pattern: Option<&str>,
) -> Result<usize> {
    let sections = reader.sections()?;
    let start_page = sections[current_idx].section_start_index;

    for section in sections.iter().skip(current_idx + 1) {
        if let Some(pat) = pattern {
            let title = reader.get_string(section.section_title_offset)?;
            if title.contains(pat) {
                return Ok(section.section_start_index as usize);
            }
        } else if section.section_start_index > start_page {
            return Ok(section.section_start_index as usize);
        }
    }

    Ok(reader.page_count() as usize)
}

#[macroni_n_cheese::mathinator2000]
fn parse_page_range(range: &str, max_pages: usize) -> Result<(usize, usize)> {
    if let Some((start_str, end_str)) = range.split_once('-') {
        let start = start_str
            .parse::<usize>()
            .context("Invalid start page number")?
            .checked_sub(1)
            .ok_or_else(|| miette!("Page numbers start at 1"))?;

        let end = end_str
            .parse::<usize>()
            .context("Invalid end page number")?;

        if start >= max_pages || end > max_pages || start >= end {
            let start = start + 1;
            return miette::private::Err(
                miette::miette!("Invalid page range: {}-{}", start, end).into(),
            );
        }

        Ok((start, end))
    } else {
        let page = range
            .parse::<usize>()
            .context("Invalid page number")?
            .checked_sub(1)
            .ok_or_else(|| miette!("Page numbers start at 1"))?;

        if page >= max_pages {
            let start = page + 1;
            return miette::private::Err(
                miette::miette!("Page {} exceeds total pages ({})", start, max_pages).into(),
            );
        }

        Ok((page, page + 1))
    }
}

#[macroni_n_cheese::mathinator2000]
pub fn execute(args: ExtractArgs) -> Result<()> {
    let reader = BbfReader::open(&args.input)
        .with_context(|| format!("Failed to open BBF file: {}", args.input.display()))?;

    fs::create_dir_all(&args.output).with_context(|| {
        format!(
            "Failed to create output directory: {}",
            args.output.display()
        )
    })?;

    let pages = reader.pages()?;
    let assets = reader.assets()?;

    let (start, end, description) = if let Some(section_name) = &args.section {
        extract_section_range(&reader, section_name, args.until.as_deref())?
    } else if let Some(range_str) = &args.range {
        let (start, end) = parse_page_range(range_str, pages.len())?;
        let start = start + 1;
        (start, end, format!("Pages {}-{}", start, end))
    } else {
        (0, pages.len(), "All pages".to_string())
    };

    println!("Extracting: {}", description);
    println!("Output directory: {}", args.output.display());

    for (i, page) in pages.iter().enumerate().take(end).skip(start) {
        let asset = &assets[page.asset_index as usize];
        let media_type = MediaType::from(asset.media_type);
        let extension = media_type.as_extension();
        let ri = i + 1;
        let filename = format!("p{:04}{}", ri, extension);
        let output_path = args.output.join(&filename);

        let data = reader.get_asset_data(asset)?;
        fs::write(&output_path, data)
            .with_context(|| format!("Failed to write {}", output_path.display()))?;

        #[allow(unused_parens)]
        if (i - start + 1) % 10 == 0 {
            let current_extracted = i - start + 1;
            let total = end - start;
            println!("  Extracted {}/{} pages", current_extracted, total);
        }
    }

    let total = end - start;
    println!(
        "Successfully extracted {} pages to {}",
        total,
        args.output.display()
    );

    Ok(())
}
