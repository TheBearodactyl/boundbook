mod commands;
mod help;

#[derive(clap::Parser)]
#[command(name = "boundbook", author = "EF1500", version = "1.0", about = "BBF CLI", long_about = None)]
#[command(disable_help_flag = true, disable_help_subcommand = true)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(clap::Subcommand)]
pub enum Commands {
    /// Print help
    Help {
        /// The subcommand to get help for
        subcommand: Option<String>,

        #[arg(hide = true, short, long)]
        gen_markdown: bool,
    },

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

    /// Generate CLI completions
    Complete(commands::complete::CompleteArgs),
}

pub fn app() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let argv = <Cli as clap::Parser>::parse();

    match argv.command {
        Commands::Help {
            gen_markdown,
            subcommand,
        } => {
            if gen_markdown {
                clap_markdown::print_help_markdown::<Cli>();
                return Ok(());
            } else {
                if let Some(subcmd) = subcommand {
                    help::rose_pine_printer_for_subcommand(&subcmd, help::RosePineVariant::Main)
                        .unwrap()
                        .print_help();
                } else {
                    help::rose_pine_printer(help::RosePineVariant::Main, None).print_help();
                }
            }

            Ok(())
        }

        Commands::Create(args) => commands::create::execute(args),
        Commands::Info(args) => commands::info::execute(args),
        Commands::Verify(args) => commands::verify::execute(args),
        Commands::Extract(args) => commands::extract::execute(args),
        Commands::FromCbz(args) => commands::from_cbz::execute(args),
        Commands::Read(args) => commands::read::execute(args),
        Commands::Complete(args) => commands::complete::execute(args),
    }
}
