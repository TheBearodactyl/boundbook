#!/usr/bin/env nu

#MISE description="Run lints on the code via clippy"
#MISE alias="lint"

cargo clippy
cargo check
