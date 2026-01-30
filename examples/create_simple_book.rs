use boundbook::BbfBuilder;

fn main() -> boundbook::Result<()> {
    let mut builder = BbfBuilder::new("woah.bbf")?;

    builder.add_metadata("title", "Skibidi Ohio Rizz")?;
    builder.add_metadata("author", "The Rizzler")?;
    builder.add_metadata("urmom", "Is Fat")?;

    builder.add_page("./images/page1.png", boundbook::MediaType::Png)?;
    builder.add_page("./images/page2.png", boundbook::MediaType::Png)?;
    builder.add_page("./images/page3.png", boundbook::MediaType::Png)?;

    builder.finalize()?;

    Ok(())
}
