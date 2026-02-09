# `boundbook` - A Rust port of [libbbf](https://github.com/ef1500/libbbf) with some extras

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

# Command-Line Help for `boundbook`

This document contains the help content for the `boundbook` command-line program.

**Command Overview:**

* [`boundbook`↴](#boundbook)
* [`boundbook docs`↴](#boundbook-docs)
* [`boundbook create`↴](#boundbook-create)
* [`boundbook info`↴](#boundbook-info)
* [`boundbook verify`↴](#boundbook-verify)
* [`boundbook extract`↴](#boundbook-extract)
* [`boundbook from-cbz`↴](#boundbook-from-cbz)
* [`boundbook read`↴](#boundbook-read)
* [`boundbook complete`↴](#boundbook-complete)

## `boundbook`

BBF CLI

**Usage:** `boundbook <COMMAND>`

###### **Subcommands:**

* `docs` — Print help
* `create` — Create a BBF file from images
* `info` — Display BBF file information
* `verify` — Verify BBF file integrity
* `extract` — Extract pages from a BBF file
* `from-cbz` — Convert CBZ archive to BBF format
* `read` — Read a BBF file in the terminal
* `complete` — Generate CLI completions



## `boundbook docs`

Print help

**Usage:** `boundbook docs`



## `boundbook create`

Create a BBF file from images

**Usage:** `boundbook create [OPTIONS] --output <OUTPUT> <INPUTS>...`

###### **Arguments:**

* `<INPUTS>` — Input files or directories containing images

###### **Options:**

* `-o`, `--output <OUTPUT>` — Output BBF file path
* `-O`, `--order <ORDER>` — Page order file (format: filename:index)
* `-S`, `--sections <SECTIONS>` — Sections file (format: Name:Target[:Parent])
* `-s`, `--section <ADD_SECTIONS>` — Add section markers (format: Name:Target[:Parent])
* `-m`, `--meta <METADATA>` — Add metadata (format: Key:Value[:Parent])
* `-a`, `--alignment <ALIGNMENT>` — Byte alignment exponent (default: 12 = 4096 bytes)

  Default value: `12`
* `-r`, `--ream-size <REAM_SIZE>` — Ream size exponent (default: 16 = 65536 bytes)

  Default value: `16`
* `-v`, `--variable-ream-size` — Enable variable ream size for smaller files
* `-d`, `--auto-detect-sections` — Auto-detect subdirectories with images and create sections from directory names



## `boundbook info`

Display BBF file information

**Usage:** `boundbook info <INPUT>`

###### **Arguments:**

* `<INPUT>`



## `boundbook verify`

Verify BBF file integrity

**Usage:** `boundbook verify [OPTIONS] <INPUT>`

###### **Arguments:**

* `<INPUT>` — BBF file to verify

###### **Options:**

* `--index-only` — Verify only the index hash (faster)
* `--asset <ASSET>` — Verify a specific asset by index



## `boundbook extract`

Extract pages from a BBF file

**Usage:** `boundbook extract [OPTIONS] <INPUT>`

###### **Arguments:**

* `<INPUT>` — BBF file to extract from

###### **Options:**

* `-o`, `--output <OUTPUT>` — Output directory for extracted pages

  Default value: `./extracted`
* `--section <SECTION>` — Extract only pages from a specific section
* `--until <UNTIL>` — Stop extraction when reaching a section matching this string
* `--range <RANGE>` — Extract a specific page range (e.g., 1-10 or 5)



## `boundbook from-cbz`

Convert CBZ archive to BBF format

**Usage:** `boundbook from-cbz [OPTIONS] --output <OUTPUT> <INPUT>`

###### **Arguments:**

* `<INPUT>` — Input CBZ file or directory containing CBZ files

###### **Options:**

* `-o`, `--output <OUTPUT>` — Output BBF file
* `-m`, `--meta <METADATA>` — Add metadata (format: Key:Value[:Parent])
* `-k`, `--keep-temp` — Keep temporary files for debugging
* `-d`, `--directory-mode` — Process directory of CBZ files as chapters



## `boundbook read`

Read a BBF file in the terminal

**Usage:** `boundbook read [OPTIONS] <INPUT>`

###### **Arguments:**

* `<INPUT>` — BBF file to read

###### **Options:**

* `-P`, `--prerender` — Pre-render all pages before reading (uses more memory but smoother navigation)
* `-W`, `--max-width <PIXELS>` — Maximum width in pixels (aspect ratio preserved)
* `-H`, `--max-height <PIXELS>` — Maximum height in pixels (aspect ratio preserved)
* `--max-cols <COLS>` — Maximum width in terminal columns (overrides max-width if set)
* `--max-rows <ROWS>` — Maximum height in terminal rows (overrides max-height if set)
* `-f`, `--filter <FILTER>` — Image scaling filter quality

  Default value: `lanczos3`

  Possible values: `nearest`, `triangle`, `catmull-rom`, `gaussian`, `lanczos3`

* `-g`, `--enable-gif-animation` — Enable GIF animation playback

  Default value: `true`
* `--gif-speed <MULTIPLIER>` — GIF animation frame delay multiplier (1.0 = normal speed)

  Default value: `1.0`
* `-l`, `--gif-loop` — Loop GIFs infinitely

  Default value: `true`
* `-i`, `--gif-interpolate <COUNT>` — Number of interpolated frames to generate between each GIF frame (0 = disabled)

  Default value: `0`
* `-m`, `--interpolation-method <INTERPOLATION_METHOD>` — Frame interpolation algorithm

  Default value: `blend`

  Possible values:
  - `blend`:
    Simple linear blending (fastest)
  - `smooth`:
    Weighted blending with ease-in/ease-out
  - `cosine`:
    Cosine interpolation for smoother transitions
  - `cubic`:
    Cubic hermite spline interpolation
  - `perlin`:
    Perlin smoothstep (quintic hermite)
  - `exponential`:
    Exponential ease-in-out
  - `optical-flow-sparse`:
    Optical flow based (Lucas-Kanade sparse)
  - `motion-compensated`:
    Motion-compensated blending (simplified Horn-Schunck)
  - `catmull-rom`:
    Catmull-Rom spline (requires 4 frames, falls back to cubic)

* `--sidebar-width <SIDEBAR_WIDTH>` — Sidebar width in columns

  Default value: `30`
* `--slideshow-delay <SECONDS>` — Slideshow auto-advance delay in seconds

  Default value: `5.0`



## `boundbook complete`

Generate CLI completions

**Usage:** `boundbook complete <SHELL>`

###### **Arguments:**

* `<SHELL>`

  Possible values: `bash`, `elvish`, `fish`, `power-shell`, `zsh`, `nushell`, `clink`, `fig`




<hr/>

<small><i>
    This document was generated automatically by
    <a href="https://crates.io/crates/clap-markdown"><code>clap-markdown</code></a>.
</i></small>
