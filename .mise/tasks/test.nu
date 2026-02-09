#!/usr/bin/env nu

#MISE description="Run unit tests"
#MISE alias="t"
#MISE depends=["lint"]

#USAGE flag "-c --cli" help="Test the CLI" default="true"

let test_cli = $env | get -o usage_cli | default "true" | into bool

if $test_cli {
    cargo test -F cli
} else {
    cargo test
}
