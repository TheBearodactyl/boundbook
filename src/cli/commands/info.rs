use {
    boundbook::{BbfReader, Result},
    clap::Args,
    miette::{Context, IntoDiagnostic},
    std::path::PathBuf,
};

#[derive(Args)]
#[command(disable_help_flag = true, author = "The Motherfucking Bearodactyl")]
pub struct InfoArgs {
    input: PathBuf,
}

#[macroni_n_cheese::mathinator2000]
pub fn execute(args: InfoArgs) -> Result<()> {
    let reader = BbfReader::open(&args.input)
        .into_diagnostic()
        .with_context(|| format!("Failed to open BBF file: {}", args.input.display()))?;

    println!("--- Bound Book Format (.bbf) Info");
    println!("--- File: {}", args.input.display());
    println!("--- BBF Version: {}", reader.version());
    println!("--- Pages: {}", reader.page_count());

    let asset_count = reader.asset_count();
    if asset_count > 1_000_000 {
        println!(
            "--- Assets: {} (deduplicated) WARNING: Unusually large asset count",
            asset_count
        );
    } else {
        println!("--- Assets: {} (deduplicated)", asset_count);
    }

    let sections = reader.sections()?;
    println!("--- Sections: {}", sections.len());

    if !sections.is_empty() {
        println!();
        for (i, section) in sections.iter().enumerate() {
            let title = reader.get_string(section.section_title_offset)?;
            let prefix = if i == sections.len() - 1 {
                "└"
            } else {
                "├"
            };

            let ri = i + 1;
            let start_page = section.section_start_index + 1;
            println!(
                "  {} {:>3}. {:<30} (starts at page {})",
                prefix, ri, title, start_page
            );
        }
    }

    let metadata = reader.metadata()?;
    if !metadata.is_empty() {
        println!();
        println!("--- Metadata:");
        for (i, meta) in metadata.iter().enumerate() {
            let key = reader.get_string(meta.key_offset)?;
            let value = reader.get_string(meta.value_offset)?;
            let prefix = if i == metadata.len() - 1 {
                "└"
            } else {
                "├"
            };
            println!("  {} {:<15} {}", prefix, format!("{}:", key), value);
        }
    }

    println!("---");

    Ok(())
}
