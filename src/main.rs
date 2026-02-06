//! boundbook - a Rust port of the Bound Book specification
//!
//! [see libbbf](https://github.com/ef1500/libbbf)

mod cli;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() -> boundbook::Result<()> {
    cli::app()
}
