//! boundbook - a Rust port of the Bound Book specification
//!
//! [see libbbf](https://github.com/ef1500/libbbf)
#[cfg(feature = "cli")]
mod cli;

#[cfg(feature = "cli")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "cli")]
fn main() {
    miette::set_hook(Box::new(|_| {
        Box::new(miette::MietteHandlerOpts::new().color(true).build())
    }))
    .ok();

    if let Err(err) = cli::app() {
        eprintln!("{:?}", miette::Report::from(err));
        std::process::exit(1);
    }
}

#[cfg(not(feature = "cli"))]
fn main() {}
