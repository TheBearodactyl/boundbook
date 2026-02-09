#!/usr/bin/env nu

#MISE description="Generate completion files"
#MISE alias="c"
#MISE depends=["build"]

./target/release/boundbook.exe complete bash | save completions/boundbook.bash -f
./target/release/boundbook.exe complete clink | save completions/boundbook.clink -f
./target/release/boundbook.exe complete elvish | save completions/boundbook.elv -f
./target/release/boundbook.exe complete fig | save completions/boundbook.fig -f
./target/release/boundbook.exe complete fish | save completions/boundbook.fish -f
./target/release/boundbook.exe complete nushell | save completions/boundbook.nu -f
./target/release/boundbook.exe complete power-shell | save completions/boundbook.ps1 -f
./target/release/boundbook.exe complete zsh | save completions/boundbook.zsh -f
