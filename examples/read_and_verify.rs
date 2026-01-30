use boundbook::BbfReader;

fn main() -> boundbook::Result<()> {
    let reader = BbfReader::open("woah.bbf")?;

    println!("BBF Version: {}", reader.version());
    println!("Pages: {}", reader.page_count());
    println!("Assets: {} (deduped)", reader.asset_count());

    println!("\n[metadata]:");

    for meta in reader.metadata() {
        let key = reader.get_string(meta.key_offset)?;
        let value = reader.get_string(meta.val_offset)?;
        println!("  {}: {}", key, value);
    }

    println!("\n[sections]");

    if reader.sections().is_empty() {
        println!("  no sections defined");
    } else {
        for section in reader.sections() {
            let title = reader.get_string(section.title_offset)?;
            println!("  {} (starts at page {})", title, section.start_index + 1);
        }
    }

    println!("\n[verification]");
    match reader.verify_integrity() {
        Ok(true) => println!("  all good!"),
        Ok(false) => println!("  failed!"),
        Err(e) => println!("  error: {}", e),
    }

    Ok(())
}
