#!/usr/bin/env nu

#MISE description="Generate the README"
#MISE depends=["build"]
#MISE outputs=["README.md"]

let header = "# `boundbook` - A Rust port of [libbbf](https://github.com/ef1500/libbbf) with some extras

Follows the [Bound Book Format specification (v3)](https://github.com/ef1500/libbbf/blob/main/SPECNOTE.txt)

Features I've added so far:

- A CBZ-to-BBF converter
- An in-terminal book reader

## Installing the CLI

To install the boundbook CLI, run:

```rs
cargo install boundbook -F cli
```

Without the `cli` feature flag, the `boundbook` binary will do nothing.

---

"

let the_rest_of_the_readme = $"(./target/release/boundbook.exe docs)"
let readme = ([$header $the_rest_of_the_readme] | str join)

$readme | save README.md -f
