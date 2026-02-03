mod cli;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> color_eyre::Result<()> {
    cli::app()
}
