use {
    boundbook::{BbfError, BbfReader, Result},
    clap::Args,
    miette::{Context, IntoDiagnostic},
    std::path::PathBuf,
};

#[derive(Args)]
#[command(author = "The Motherfucking Bearodactyl")]
pub struct VerifyArgs {
    /// BBF file to verify
    input: PathBuf,

    /// Verify only the index hash (faster)
    #[arg(long)]
    index_only: bool,

    /// Verify a specific asset by index
    #[arg(long, conflicts_with = "index_only")]
    asset: Option<usize>,
}

pub fn execute(args: VerifyArgs) -> Result<()> {
    let reader = BbfReader::open(&args.input)
        .into_diagnostic()
        .with_context(|| format!("Failed to open BBF file: {}", args.input.display()))?;

    if let Some(asset_index) = args.asset {
        println!("Verifying asset {}...", asset_index);
        let valid = reader
            .verify_asset(asset_index)
            .into_diagnostic()
            .with_context(|| format!("Failed to verify asset {}", asset_index))?;

        if valid {
            println!("✓ Asset {} integrity OK", asset_index);
            Ok(())
        } else {
            Err(BbfError::from(miette::miette!(
                "✗ Asset {} is corrupted",
                asset_index
            )))
        }
    } else if args.index_only {
        println!("Verifying index hash...");
        let valid = reader.verify_integrity().into_diagnostic()?;

        if valid {
            println!("✓ Index integrity OK");
            Ok(())
        } else {
            Err(BbfError::from(miette::miette!(
                "✗ Index hash mismatch - file may be corrupted"
            )))
        }
    } else {
        println!("Verifying complete file integrity (parallel)...");
        let valid = reader.verify_integrity().into_diagnostic()?;

        if valid {
            println!("✓ All integrity checks passed");
            println!("  • Index hash: OK");
            println!("  • {} assets verified: OK", reader.asset_count());
            Ok(())
        } else {
            Err(BbfError::from(miette::miette!(
                "✗ Integrity check failed - file is corrupted"
            )))
        }
    }
}
