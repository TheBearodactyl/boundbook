#!/usr/bin/env nu

#MISE description="Build the `boundbook` crate"
#MISE alias="b"
#MISE sources=["Cargo.toml", "src/**/*.rs"]

#USAGE flag "--no-cli" help="Don't build with the CLI enabled"

let dont_build_cli = $env | get usage_cli --optional | default "false" | into bool

if $dont_build_cli {
    cargo build -j (sys cpu | length) --profile release
} else {
    cargo build -j (sys cpu | length) --profile release -F cli
}
