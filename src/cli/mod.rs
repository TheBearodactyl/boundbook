mod commands;

#[derive(clap::Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// Create a BBF file from images
    Create(commands::create::CreateArgs),

    /// Display BBF file information
    Info(commands::info::InfoArgs),

    /// Verify BBF file integrity
    Verify(commands::verify::VerifyArgs),

    /// Extract pages from a BBF file
    Extract(commands::extract::ExtractArgs),

    /// Convert CBZ archive to BBF format
    FromCbz(commands::from_cbz::FromCbzArgs),

    /// Read a BBF file in the terminal
    Read(commands::read::ReadArgs),
}

pub fn app() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let cli = <Cli as clap::Parser>::parse();

    match cli.command {
        Commands::Create(args) => commands::create::execute(args),
        Commands::Info(args) => commands::info::execute(args),
        Commands::Verify(args) => commands::verify::execute(args),
        Commands::Extract(args) => commands::extract::execute(args),
        Commands::FromCbz(args) => commands::from_cbz::execute(args),
        Commands::Read(args) => commands::read::execute(args),
    }
}
