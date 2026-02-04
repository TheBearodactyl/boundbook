# `boundbook` - A Rust port of [libbbf](https://github.com/ef1500/libbbf)

---

# Command-Line Help for `boundbook`

This document contains the help content for the `boundbook` command-line program.

**Command Overview:**

- [`boundbook`↴](#boundbook)
- [`boundbook help`↴](#boundbook-help)
- [`boundbook create`↴](#boundbook-create)
- [`boundbook info`↴](#boundbook-info)
- [`boundbook verify`↴](#boundbook-verify)
- [`boundbook extract`↴](#boundbook-extract)
- [`boundbook from-cbz`↴](#boundbook-from-cbz)
- [`boundbook read`↴](#boundbook-read)
- [`boundbook complete`↴](#boundbook-complete)

## `boundbook`

BBF CLI

**Usage:** `boundbook <COMMAND>`

###### **Subcommands:**

- `help` — Print help
- `create` — Create a BBF file from images
- `info` — Display BBF file information
- `verify` — Verify BBF file integrity
- `extract` — Extract pages from a BBF file
- `from-cbz` — Convert CBZ archive to BBF format
- `read` — Read a BBF file in the terminal
- `complete` — Generate CLI completions

## `boundbook help`

Print help

**Usage:** `boundbook help [SUBCOMMAND]`

###### **Arguments:**

- `<SUBCOMMAND>` — The subcommand to get help for

## `boundbook create`

Create a BBF file from images

**Usage:** `boundbook create [OPTIONS] --output <OUTPUT> <INPUTS>...`

###### **Arguments:**

- `<INPUTS>` — Input files or directories containing images

###### **Options:**

- `-o`, `--output <OUTPUT>` — Output BBF file path
- `--order <ORDER>` — Page order file (format: filename:index)
- `--sections <SECTIONS>` — Sections file (format: Name:Target[:Parent])
- `--section <ADD_SECTIONS>` — Add section markers (format: Name:Target[:Parent])
- `--meta <METADATA>` — Add metadata (format: Key:Value[:Parent])
- `--alignment <ALIGNMENT>` — Byte alignment exponent (default: 12 = 4096 bytes)

  Default value: `12`

- `--ream-size <REAM_SIZE>` — Ream size exponent (default: 16 = 65536 bytes)

  Default value: `16`

- `--variable-ream-size` — Enable variable ream size for smaller files

## `boundbook info`

Display BBF file information

**Usage:** `boundbook info <INPUT>`

###### **Arguments:**

- `<INPUT>`

## `boundbook verify`

Verify BBF file integrity

**Usage:** `boundbook verify [OPTIONS] <INPUT>`

###### **Arguments:**

- `<INPUT>` — BBF file to verify

###### **Options:**

- `--index-only` — Verify only the index hash (faster)
- `--asset <ASSET>` — Verify a specific asset by index

## `boundbook extract`

Extract pages from a BBF file

**Usage:** `boundbook extract [OPTIONS] <INPUT>`

###### **Arguments:**

- `<INPUT>` — BBF file to extract from

###### **Options:**

- `-o`, `--output <OUTPUT>` — Output directory for extracted pages

  Default value: `./extracted`

- `--section <SECTION>` — Extract only pages from a specific section
- `--until <UNTIL>` — Stop extraction when reaching a section matching this string
- `--range <RANGE>` — Extract a specific page range (e.g., 1-10 or 5)

## `boundbook from-cbz`

Convert CBZ archive to BBF format

**Usage:** `boundbook from-cbz [OPTIONS] --output <OUTPUT> <INPUT>`

###### **Arguments:**

- `<INPUT>` — Input CBZ file

###### **Options:**

- `-o`, `--output <OUTPUT>` — Output BBF file
- `--meta <METADATA>` — Add metadata (format: Key:Value[:Parent])
- `--keep-temp` — Keep temporary files for debugging

## `boundbook read`

Read a BBF file in the terminal

**Usage:** `boundbook read [OPTIONS] <INPUT>`

###### **Arguments:**

- `<INPUT>` — BBF file to read

###### **Options:**

- `--prerender` — Pre-render all pages before reading (uses more memory but smoother navigation)

## `boundbook complete`

Generate CLI completions

**Usage:** `boundbook complete <SHELL>`

###### **Arguments:**

- `<SHELL>`

  Possible values: `bash`, `elvish`, `fish`, `power-shell`, `zsh`, `nushell`, `clink`

<hr/>

<small><i>
This document was generated automatically by
<a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
