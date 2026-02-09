
use builtin;
use str;

set edit:completion:arg-completer[boundbook] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'boundbook'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'boundbook'= {
            cand -h 'Print help'
            cand --help 'Print help'
            cand -V 'Print version'
            cand --version 'Print version'
            cand docs 'Print help'
            cand create 'Create a BBF file from images'
            cand info 'Display BBF file information'
            cand verify 'Verify BBF file integrity'
            cand extract 'Extract pages from a BBF file'
            cand from-cbz 'Convert CBZ archive to BBF format'
            cand read 'Read a BBF file in the terminal'
            cand complete 'Generate CLI completions'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'boundbook;docs'= {
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'boundbook;create'= {
            cand -o 'Output BBF file path'
            cand --output 'Output BBF file path'
            cand -O 'Page order file (format: filename:index)'
            cand --order 'Page order file (format: filename:index)'
            cand -S 'Sections file (format: Name:Target[:Parent])'
            cand --sections 'Sections file (format: Name:Target[:Parent])'
            cand -s 'Add section markers (format: Name:Target[:Parent])'
            cand --section 'Add section markers (format: Name:Target[:Parent])'
            cand -m 'Add metadata (format: Key:Value[:Parent])'
            cand --meta 'Add metadata (format: Key:Value[:Parent])'
            cand -a 'Byte alignment exponent (default: 12 = 4096 bytes)'
            cand --alignment 'Byte alignment exponent (default: 12 = 4096 bytes)'
            cand -r 'Ream size exponent (default: 16 = 65536 bytes)'
            cand --ream-size 'Ream size exponent (default: 16 = 65536 bytes)'
            cand -v 'Enable variable ream size for smaller files'
            cand --variable-ream-size 'Enable variable ream size for smaller files'
            cand -d 'Auto-detect subdirectories with images and create sections from directory names'
            cand --auto-detect-sections 'Auto-detect subdirectories with images and create sections from directory names'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'boundbook;info'= {
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'boundbook;verify'= {
            cand --asset 'Verify a specific asset by index'
            cand --index-only 'Verify only the index hash (faster)'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'boundbook;extract'= {
            cand -o 'Output directory for extracted pages'
            cand --output 'Output directory for extracted pages'
            cand --section 'Extract only pages from a specific section'
            cand --until 'Stop extraction when reaching a section matching this string'
            cand --range 'Extract a specific page range (e.g., 1-10 or 5)'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'boundbook;from-cbz'= {
            cand -o 'Output BBF file'
            cand --output 'Output BBF file'
            cand -m 'Add metadata (format: Key:Value[:Parent])'
            cand --meta 'Add metadata (format: Key:Value[:Parent])'
            cand -k 'Keep temporary files for debugging'
            cand --keep-temp 'Keep temporary files for debugging'
            cand -d 'Process directory of CBZ files as chapters'
            cand --directory-mode 'Process directory of CBZ files as chapters'
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'boundbook;read'= {
            cand -W 'Maximum width in pixels (aspect ratio preserved)'
            cand --max-width 'Maximum width in pixels (aspect ratio preserved)'
            cand -H 'Maximum height in pixels (aspect ratio preserved)'
            cand --max-height 'Maximum height in pixels (aspect ratio preserved)'
            cand --max-cols 'Maximum width in terminal columns (overrides max-width if set)'
            cand --max-rows 'Maximum height in terminal rows (overrides max-height if set)'
            cand -f 'Image scaling filter quality'
            cand --filter 'Image scaling filter quality'
            cand --gif-speed 'GIF animation frame delay multiplier (1.0 = normal speed)'
            cand -i 'Number of interpolated frames to generate between each GIF frame (0 = disabled)'
            cand --gif-interpolate 'Number of interpolated frames to generate between each GIF frame (0 = disabled)'
            cand -m 'Frame interpolation algorithm'
            cand --interpolation-method 'Frame interpolation algorithm'
            cand --sidebar-width 'Sidebar width in columns'
            cand --slideshow-delay 'Slideshow auto-advance delay in seconds'
            cand -P 'Pre-render all pages before reading (uses more memory but smoother navigation)'
            cand --prerender 'Pre-render all pages before reading (uses more memory but smoother navigation)'
            cand -g 'Enable GIF animation playback'
            cand --enable-gif-animation 'Enable GIF animation playback'
            cand -l 'Loop GIFs infinitely'
            cand --gif-loop 'Loop GIFs infinitely'
            cand -h 'Print help (see more with ''--help'')'
            cand --help 'Print help (see more with ''--help'')'
        }
        &'boundbook;complete'= {
            cand -h 'Print help'
            cand --help 'Print help'
        }
        &'boundbook;help'= {
            cand docs 'Print help'
            cand create 'Create a BBF file from images'
            cand info 'Display BBF file information'
            cand verify 'Verify BBF file integrity'
            cand extract 'Extract pages from a BBF file'
            cand from-cbz 'Convert CBZ archive to BBF format'
            cand read 'Read a BBF file in the terminal'
            cand complete 'Generate CLI completions'
            cand help 'Print this message or the help of the given subcommand(s)'
        }
        &'boundbook;help;docs'= {
        }
        &'boundbook;help;create'= {
        }
        &'boundbook;help;info'= {
        }
        &'boundbook;help;verify'= {
        }
        &'boundbook;help;extract'= {
        }
        &'boundbook;help;from-cbz'= {
        }
        &'boundbook;help;read'= {
        }
        &'boundbook;help;complete'= {
        }
        &'boundbook;help;help'= {
        }
    ]
    $completions[$command]
}
