module completions {

  # BBF CLI
  export extern boundbook [
    --help(-h)                # Print help
    --version(-V)             # Print version
  ]

  # Print help
  export extern "boundbook docs" [
    --help(-h)                # Print help
  ]

  # Create a BBF file from images
  export extern "boundbook create" [
    --output(-o): path        # Output BBF file path
    --order(-O): path         # Page order file (format: filename:index)
    --sections(-S): path      # Sections file (format: Name:Target[:Parent])
    --section(-s): string     # Add section markers (format: Name:Target[:Parent])
    --meta(-m): string        # Add metadata (format: Key:Value[:Parent])
    --alignment(-a): string   # Byte alignment exponent (default: 12 = 4096 bytes)
    --ream-size(-r): string   # Ream size exponent (default: 16 = 65536 bytes)
    --variable-ream-size(-v)  # Enable variable ream size for smaller files
    --auto-detect-sections(-d) # Auto-detect subdirectories with images and create sections from directory names
    --help(-h)                # Print help
    ...inputs: path           # Input files or directories containing images
  ]

  # Display BBF file information
  export extern "boundbook info" [
    --help(-h)                # Print help
    input: path
  ]

  # Verify BBF file integrity
  export extern "boundbook verify" [
    --index-only              # Verify only the index hash (faster)
    --asset: string           # Verify a specific asset by index
    --help(-h)                # Print help
    input: path               # BBF file to verify
  ]

  # Extract pages from a BBF file
  export extern "boundbook extract" [
    --output(-o): path        # Output directory for extracted pages
    --section: string         # Extract only pages from a specific section
    --until: string           # Stop extraction when reaching a section matching this string
    --range: string           # Extract a specific page range (e.g., 1-10 or 5)
    --help(-h)                # Print help
    input: path               # BBF file to extract from
  ]

  # Convert CBZ archive to BBF format
  export extern "boundbook from-cbz" [
    --output(-o): path        # Output BBF file
    --meta(-m): string        # Add metadata (format: Key:Value[:Parent])
    --keep-temp(-k)           # Keep temporary files for debugging
    --directory-mode(-d)      # Process directory of CBZ files as chapters
    --help(-h)                # Print help
    input: path               # Input CBZ file or directory containing CBZ files
  ]

  def "nu-complete boundbook read filter" [] {
    [ "nearest" "triangle" "catmull-rom" "gaussian" "lanczos3" ]
  }

  def "nu-complete boundbook read interpolation_method" [] {
    [ "blend" "smooth" "cosine" "cubic" "perlin" "exponential" "optical-flow-sparse" "motion-compensated" "catmull-rom" ]
  }

  # Read a BBF file in the terminal
  export extern "boundbook read" [
    --prerender(-P)           # Pre-render all pages before reading (uses more memory but smoother navigation)
    --max-width(-W): string   # Maximum width in pixels (aspect ratio preserved)
    --max-height(-H): string  # Maximum height in pixels (aspect ratio preserved)
    --max-cols: string        # Maximum width in terminal columns (overrides max-width if set)
    --max-rows: string        # Maximum height in terminal rows (overrides max-height if set)
    --filter(-f): string@"nu-complete boundbook read filter" # Image scaling filter quality
    --enable-gif-animation(-g) # Enable GIF animation playback
    --gif-speed: string       # GIF animation frame delay multiplier (1.0 = normal speed)
    --gif-loop(-l)            # Loop GIFs infinitely
    --gif-interpolate(-i): string # Number of interpolated frames to generate between each GIF frame (0 = disabled)
    --interpolation-method(-m): string@"nu-complete boundbook read interpolation_method" # Frame interpolation algorithm
    --sidebar-width: string   # Sidebar width in columns
    --slideshow-delay: string # Slideshow auto-advance delay in seconds
    --help(-h)                # Print help (see more with '--help')
    input: path               # BBF file to read
  ]

  def "nu-complete boundbook complete shell" [] {
    [ "bash" "elvish" "fish" "power-shell" "zsh" "nushell" "clink" "fig" ]
  }

  # Generate CLI completions
  export extern "boundbook complete" [
    --help(-h)                # Print help
    shell: string@"nu-complete boundbook complete shell"
  ]

  # Print this message or the help of the given subcommand(s)
  export extern "boundbook help" [
  ]

  # Print help
  export extern "boundbook help docs" [
  ]

  # Create a BBF file from images
  export extern "boundbook help create" [
  ]

  # Display BBF file information
  export extern "boundbook help info" [
  ]

  # Verify BBF file integrity
  export extern "boundbook help verify" [
  ]

  # Extract pages from a BBF file
  export extern "boundbook help extract" [
  ]

  # Convert CBZ archive to BBF format
  export extern "boundbook help from-cbz" [
  ]

  # Read a BBF file in the terminal
  export extern "boundbook help read" [
  ]

  # Generate CLI completions
  export extern "boundbook help complete" [
  ]

  # Print this message or the help of the given subcommand(s)
  export extern "boundbook help help" [
  ]

}

export use completions *
