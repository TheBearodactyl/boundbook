use {
    boundbook::BbfReader,
    clap::Args,
    color_eyre::eyre::{Context, Result},
    std::path::PathBuf,
};

#[derive(Args)]
pub struct InfoArgs {
    input: PathBuf,
}

pub fn execute(args: InfoArgs) -> Result<()> {
    let reader = BbfReader::open(&args.input)
        .with_context(|| format!("Failed to open BBF file: {}", args.input.display()))?;

    println!("╭─ Bound Book Format (.bbf) Info");
    println!("├─ File: {}", args.input.display());
    println!("├─ BBF Version: {}", reader.version());
    println!("├─ Pages: {}", reader.page_count());
    println!("├─ Assets: {} (deduplicated)", reader.asset_count());

    let sections = reader.sections();
    println!("├─ Sections: {}", sections.len());

    if !sections.is_empty() {
        println!("│");
        for (i, section) in sections.iter().enumerate() {
            let title = reader.get_string(section.title_offset)?;
            let prefix = if i == sections.len() - 1 {
                "└"
            } else {
                "├"
            };
            println!(
                "│  {} {:>3}. {:<30} (starts at page {})",
                prefix,
                i + 1,
                title,
                section.start_index + 1
            );
        }
    }

    let metadata = reader.metadata();
    if !metadata.is_empty() {
        println!("│");
        println!("├─ Metadata:");
        for (i, meta) in metadata.iter().enumerate() {
            let key = reader.get_string(meta.key_offset)?;
            let value = reader.get_string(meta.val_offset)?;
            let prefix = if i == metadata.len() - 1 {
                "└"
            } else {
                "├"
            };
            println!("│  {} {:<15} {}", prefix, format!("{}:", key), value);
        }
    }

    println!("╰─");

    Ok(())
}
