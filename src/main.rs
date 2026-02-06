//! boundbook - a Rust port of the Bound Book specification
//!
//! [see libbbf](https://github.com/ef1500/libbbf)
mod cli;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

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
